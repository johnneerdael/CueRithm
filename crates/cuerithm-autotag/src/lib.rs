//! Automatic tagging adapters. GPL-3.0-only; the independent analysis engine is MIT.
mod asf;
mod config;
mod evidence;
mod review;
pub use review::{review_catalog_url, validate_review_required_fields};
mod catalog_artwork;
pub use catalog_artwork::fetch_catalog_artwork;
mod editing;
pub use editing::*;
mod fields;
mod id3_integrity;
pub use fields::{FieldWriteOutcome, FieldWriteStatus};
mod retry;
pub use evidence::{CandidateProvenance, CandidateSnapshot};
pub use retry::{BackendFailure, ProviderFailureKind};
mod metadata;
mod transfer;
mod write;
pub use cuerithm_cleanup::{FieldChange, Metadata, MetadataField};
pub use metadata::{has_embedded_artwork, read_metadata, read_star_rating};
pub use transfer::{
    TransferDate, TransferPatch, TransferSnapshot, TransferText, read_transfer_metadata,
};
pub use write::{AnalysisTags, StagedTagWrite, TagWritePlan};
mod image;
mod sidecar;
pub use sidecar::StagedSidecar;
mod input;
mod providers;
use anyhow::{Result, ensure};
pub use config::{AnalysisTagSource, AutotagConfig, NoMatchPolicy};
pub use input::TagInput;
pub use onetagger_selected_providers::http::{ChallengeStore, MemoryChallengeStore, SolverConfig};
pub use onetagger_selected_providers::traxsource::TraxsourceRelease as CatalogRelease;
use onetagger_tagger::MatchingUtils;
pub use onetagger_tagger::{
    Album as CatalogAlbum, FrameName, PlatformInfo, SupportedTag, TaggerConfig,
    Track as CatalogTrack, TrackMatch as CatalogMatch,
};
pub use providers::CatalogEndpoints;
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Beatport,
    Traxsource,
}
impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Beatport => "beatport",
            Self::Traxsource => "traxsource",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Matched,
    NoMatch,
    AlreadyTagged,
    Ambiguous,
    Unavailable,
}
#[derive(Debug, Serialize)]
pub struct ProviderFailure {
    pub provider: ProviderId,
    pub message: String,
    pub kind: ProviderFailureKind,
}
#[derive(Debug, Serialize)]
pub struct TagMatchOutcome {
    pub status: MatchStatus,
    pub matches: Vec<CatalogMatch>,
    pub attempted: Vec<ProviderId>,
    pub warnings: Vec<ProviderFailure>,
}
#[derive(Clone)]
pub struct AlbumHint {
    pub provider: ProviderId,
    pub album: Arc<CatalogAlbum>,
}
#[derive(Debug)]
pub struct CatalogLookupError {
    pub outcome: TagMatchOutcome,
}
impl std::fmt::Display for CatalogLookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Catalog lookup failed: {}",
            self.outcome
                .warnings
                .iter()
                .map(|w| format!("{}: {}", w.provider.as_str(), w.message))
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}
impl std::error::Error for CatalogLookupError {}
pub trait ProviderBackend: Send + Sync {
    fn id(&self) -> ProviderId;
    /// Total provider-stage bound, including shared gate and retry waits.
    fn stage_timeout(&self, _config: &AutotagConfig) -> std::time::Duration {
        std::time::Duration::from_secs(95)
    }
    fn search_releases(
        &self,
        _query: &str,
        _page: u32,
        _config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogRelease>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        anyhow::bail!("Release search is unavailable for this provider")
    }
    fn album(&self, _id: &str, _config: &AutotagConfig) -> Result<Option<CatalogAlbum>> {
        Ok(None)
    }
    fn candidates(&self, input: &TagInput, config: &AutotagConfig) -> Result<Vec<CatalogMatch>>;
    fn candidates_cancellable(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        self.candidates(input, config)
    }
    fn album_cancellable(
        &self,
        id: &str,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<CatalogAlbum>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        self.album(id, config)
    }
    fn extend_cancellable(
        &self,
        track: &mut CatalogTrack,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        self.extend(track, config)
    }

    fn extend(&self, _track: &mut CatalogTrack, _config: &AutotagConfig) -> Result<()> {
        Ok(())
    }
}
#[derive(Default)]
struct Gate {
    active: Mutex<usize>,
    changed: Condvar,
}
impl Gate {
    fn acquire(&self, limit: usize, cancel: &Arc<AtomicBool>) -> Result<Permit<'_>> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("Provider gate interrupted"))?;
        while *active >= limit {
            ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
            active = self
                .changed
                .wait_timeout(active, std::time::Duration::from_millis(100))
                .map_err(|_| anyhow::anyhow!("Provider gate interrupted"))?
                .0;
        }
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        *active += 1;
        Ok(Permit(self))
    }
}
struct Permit<'a>(&'a Gate);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        if let Ok(mut n) = self.0.active.lock() {
            *n -= 1;
            self.0.changed.notify_all();
        }
    }
}
struct Backend {
    source: Arc<dyn ProviderBackend>,
    gate: Gate,
    cooldown: retry::Cooldown,
}
#[derive(serde::Serialize)]
pub struct TaggingCatalog {
    pub defaults: AutotagConfig,
    pub providers: Vec<PlatformInfo>,
    pub access: Vec<SourceAccess>,
}
#[derive(Debug, serde::Serialize)]
pub struct SourceAccess {
    pub id: String,
    pub requires_solver: bool,
}
pub fn settings_catalog() -> TaggingCatalog {
    TaggingCatalog {
        defaults: AutotagConfig::default(),
        providers: Tagger::provider_options(),
        access: vec![],
    }
}
pub struct Tagger {
    backends: Vec<Backend>,
}
impl Tagger {
    pub fn new() -> Result<Self> {
        Self::with_endpoints(CatalogEndpoints::default())
    }
    pub fn with_endpoints(endpoints: CatalogEndpoints) -> Result<Self> {
        Self::with_endpoints_and_store(endpoints, Arc::new(MemoryChallengeStore::default()))
    }
    pub fn with_endpoints_and_store(
        endpoints: CatalogEndpoints,
        store: Arc<dyn ChallengeStore>,
    ) -> Result<Self> {
        Self::with_backends(providers::backends(endpoints, store)?)
    }
    pub fn with_backends(backends: Vec<Arc<dyn ProviderBackend>>) -> Result<Self> {
        let mut seen = std::collections::HashSet::new();
        for b in &backends {
            ensure!(seen.insert(b.id()), "Duplicate provider backend");
        }
        Ok(Self {
            backends: backends
                .into_iter()
                .map(|source| Backend {
                    source,
                    gate: Gate::default(),
                    cooldown: retry::Cooldown::default(),
                })
                .collect(),
        })
    }
    pub fn provider_options() -> Vec<PlatformInfo> {
        providers::options()
    }
    pub fn match_candidates(
        input: &TagInput,
        tracks: Vec<CatalogTrack>,
        config: &AutotagConfig,
    ) -> Result<Vec<CatalogMatch>> {
        config.validate()?;
        let info = input.audio_info(config)?;
        let tracks = tracks
            .into_iter()
            .filter(|t| {
                MatchingUtils::match_duration(&info, t, &config.options)
                    && evidence::identity_conflicts(input, t, config).is_empty()
            })
            .collect();
        let mut found = MatchingUtils::match_track(&info, &tracks, &config.options, true);
        sort_candidates(&mut found, config);
        Ok(found)
    }
    pub fn match_track(&self, input: &TagInput, config: &AutotagConfig) -> Result<TagMatchOutcome> {
        self.match_track_cancellable(input, config, &Arc::new(AtomicBool::new(false)))
    }
    pub fn match_track_cancellable(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<TagMatchOutcome> {
        self.match_track_with_album(input, config, cancel, None)
    }
    /// Search an explicit release-results page using the provider's shared request gate.
    pub fn search_releases(
        &self,
        provider: ProviderId,
        query: &str,
        page: u32,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogRelease>> {
        config.validate()?;
        ensure!(!query.trim().is_empty(), "Release query is empty");
        ensure!(page > 0, "Catalog pages start at 1");
        let backend = self
            .backends
            .iter()
            .find(|b| b.source.id() == provider)
            .ok_or_else(|| anyhow::anyhow!("Provider unavailable"))?;
        let limit = if provider == ProviderId::Traxsource {
            1
        } else {
            usize::from(config.options.threads)
        };
        let _permit = backend.gate.acquire(limit, cancel)?;
        backend.source.search_releases(query, page, config, cancel)
    }
    pub fn get_album(
        &self,
        provider: ProviderId,
        id: &str,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<Arc<CatalogAlbum>>> {
        config.validate()?;
        let backend = self
            .backends
            .iter()
            .find(|b| b.source.id() == provider)
            .ok_or_else(|| anyhow::anyhow!("Provider unavailable"))?;
        let limit = if provider == ProviderId::Traxsource {
            1
        } else {
            usize::from(config.options.threads)
        };
        retry::bounded(cancel, backend.source.stage_timeout(config), |cancel| {
            let _permit = backend.gate.acquire(limit, cancel)?;
            backend
                .cooldown
                .run(cancel, backend.source.stage_timeout(config), || {
                    backend.source.album_cancellable(id, config, cancel)
                })
                .map(|album| album.map(Arc::new))
        })
    }
    pub fn match_track_with_album(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
        hint: Option<&AlbumHint>,
    ) -> Result<TagMatchOutcome> {
        config.validate()?;
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        if config.options.skip_tagged && input.audio_info(config)?.tagged.at() {
            return Ok(TagMatchOutcome {
                status: MatchStatus::AlreadyTagged,
                matches: vec![],
                attempted: vec![],
                warnings: vec![],
            });
        }
        let mut out = TagMatchOutcome {
            status: MatchStatus::NoMatch,
            matches: vec![],
            attempted: vec![],
            warnings: vec![],
        };
        let hint = hint.filter(|hint| {
            config.options.album_tagging
                && config
                    .options
                    .platforms
                    .iter()
                    .any(|id| id == hint.provider.as_str())
        });
        let mut album_candidates =
            hint.map(|hint| Self::match_candidates(input, hint.album.tracks.clone(), config));
        let mut platforms = config.options.platforms.iter().collect::<Vec<_>>();
        // A matching selected release takes precedence over single-track search.
        // Preserve configured search order on a miss and for multiplatform writes.
        if !config.options.multiplatform
            && let (Some(hint), Some(Ok(found))) = (hint, &album_candidates)
            && !found.is_empty()
        {
            platforms.sort_by_key(|id| id.as_str() != hint.provider.as_str());
        }
        for id in platforms {
            let b = self
                .backends
                .iter()
                .find(|b| b.source.id().as_str() == id)
                .ok_or_else(|| anyhow::anyhow!("Provider unavailable: {id}"))?;
            let provider = b.source.id();
            out.attempted.push(provider);
            let limit = if provider == ProviderId::Traxsource {
                1
            } else {
                usize::from(config.options.threads)
            };
            let result = retry::bounded(
                cancel,
                b.source.stage_timeout(config),
                |cancel| -> Result<Vec<CatalogMatch>> {
                    let _permit = b.gate.acquire(limit, cancel)?;
                    let mut found = if hint.is_some_and(|hint| hint.provider == provider) {
                        let mut found = album_candidates.take().transpose()?.unwrap_or_default();
                        for item in &mut found {
                            item.reason = onetagger_tagger::MatchReason::Album;
                        }
                        found
                    } else {
                        vec![]
                    };
                    if found.is_empty() {
                        found = b.cooldown.run(cancel, b.source.stage_timeout(config), || {
                            b.source.candidates_cancellable(input, config, cancel)
                        })?;
                    }
                    ensure!(
                        found.iter().all(|m| m.accuracy.is_finite()
                            && (0.0..=1.0).contains(&m.accuracy)
                            && m.track.platform == provider.as_str()),
                        "Invalid catalog match"
                    );
                    sort_candidates(&mut found, config);
                    if found.is_empty() {
                        return Ok(vec![]);
                    }
                    let all = found.clone();
                    found.retain(|m| {
                        evidence::identity_conflicts(input, &m.track, config).is_empty()
                    });
                    if found.is_empty() {
                        return Ok(all
                            .into_iter()
                            .take(evidence::MAX_REVIEW_CANDIDATES + 1)
                            .collect());
                    }
                    let tied_default = config.options.multiple_matches
                        == onetagger_tagger::MultipleMatchesSort::Default
                        && found
                            .get(1)
                            .is_some_and(|next| (found[0].accuracy - next.accuracy).abs() <= 0.01);
                    let recording_conflict = found.iter().skip(1).any(|next| {
                        evidence::recording_conflict(&found[0].track, &next.track, config)
                    });
                    if tied_default || recording_conflict {
                        return Ok(found
                            .into_iter()
                            .take(evidence::MAX_REVIEW_CANDIDATES + 1)
                            .collect());
                    }
                    ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
                    let mut matched = found.remove(0);
                    // Traxsource album tracks already contain the full release metadata.
                    // Re-extending would fetch the track page and that same release again.
                    if provider != ProviderId::Traxsource
                        || matched.reason != onetagger_tagger::MatchReason::Album
                    {
                        b.cooldown
                            .run(cancel, b.source.stage_timeout(config), || {
                                // Do not let a failed partial extension contaminate a retry.
                                let mut track = matched.track.clone();
                                b.source.extend_cancellable(&mut track, config, cancel)?;
                                Ok(track)
                            })
                            .map(|track| matched.track = track)?;
                    }
                    Ok(vec![matched])
                },
            );
            match result {
                Ok(found) if !found.is_empty() => {
                    let ambiguous = found.len() > 1
                        || !evidence::identity_conflicts(input, &found[0].track, config).is_empty();
                    out.matches.extend(found);
                    if ambiguous {
                        out.status = MatchStatus::Ambiguous;
                        break;
                    }
                    if !config.options.multiplatform {
                        break;
                    }
                }
                Ok(_) => {}
                Err(e) => out.warnings.push(ProviderFailure {
                    provider,
                    message: format!("{e:#}").chars().take(2048).collect(),
                    kind: retry::classify(&e),
                }),
            }
        }
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        if !out.warnings.is_empty() && (config.options.multiplatform || out.matches.is_empty()) {
            out.status = MatchStatus::Unavailable;
            return Err(CatalogLookupError { outcome: out }.into());
        }
        if out.matches.iter().enumerate().any(|(i, a)| {
            out.matches
                .iter()
                .skip(i + 1)
                .any(|b| evidence::recording_conflict(&a.track, &b.track, config))
        }) {
            out.status = MatchStatus::Ambiguous;
        }
        if out.status != MatchStatus::Ambiguous && !out.matches.is_empty() {
            out.status = MatchStatus::Matched;
        }
        Ok(out)
    }
}

fn sort_candidates(found: &mut Vec<CatalogMatch>, config: &AutotagConfig) {
    found.sort_by(|a, b| {
        evidence::stable_key(&a.track)
            .cmp(&evidence::stable_key(&b.track))
            .then_with(|| b.accuracy.total_cmp(&a.accuracy))
    });
    found.dedup_by(|a, b| evidence::stable_key(&a.track) == evidence::stable_key(&b.track));
    MatchingUtils::sort_tracks(found, &config.options);
}

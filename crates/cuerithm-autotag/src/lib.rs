//! Automatic tagging adapters. GPL-3.0-only; the independent analysis engine is MIT.
mod asf;
mod config;
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
}
#[derive(Debug, Serialize)]
pub struct ProviderFailure {
    pub provider: ProviderId,
    pub message: String,
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
pub trait ProviderBackend: Send + Sync {
    fn id(&self) -> ProviderId;
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
            .filter(|t| MatchingUtils::match_duration(&info, t, &config.options))
            .collect();
        let mut found = MatchingUtils::match_track(&info, &tracks, &config.options, true);
        MatchingUtils::sort_tracks(&mut found, &config.options);
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
        let _permit = backend.gate.acquire(limit, cancel)?;
        Ok(backend
            .source
            .album_cancellable(id, config, cancel)?
            .map(Arc::new))
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
        for id in &config.options.platforms {
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
            let result = (|| -> Result<Option<CatalogMatch>> {
                let _permit = b.gate.acquire(limit, cancel)?;
                let mut found = if config.options.album_tagging {
                    if let Some(hint) = hint.filter(|h| h.provider == provider) {
                        let mut found =
                            Self::match_candidates(input, hint.album.tracks.clone(), config)?;
                        for item in &mut found {
                            item.reason = onetagger_tagger::MatchReason::Album;
                        }
                        found
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };
                if found.is_empty() {
                    found = b.source.candidates_cancellable(input, config, cancel)?;
                }
                ensure!(
                    found.iter().all(|m| m.accuracy.is_finite()
                        && (0.0..=1.0).contains(&m.accuracy)
                        && m.track.platform == provider.as_str()),
                    "Invalid catalog match"
                );
                MatchingUtils::sort_tracks(&mut found, &config.options);
                if found.is_empty() {
                    return Ok(None);
                }
                ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
                let mut matched = found.remove(0);
                // Traxsource album tracks already contain the full release metadata.
                // Re-extending would fetch the track page and that same release again.
                if provider != ProviderId::Traxsource
                    || matched.reason != onetagger_tagger::MatchReason::Album
                {
                    b.source
                        .extend_cancellable(&mut matched.track, config, cancel)?;
                }
                Ok(Some(matched))
            })();
            match result {
                Ok(Some(m)) => {
                    out.matches.push(m);
                    if !config.options.multiplatform {
                        break;
                    }
                }
                Ok(None) => {}
                Err(e) => out.warnings.push(ProviderFailure {
                    provider,
                    message: format!("{e:#}"),
                }),
            }
        }
        if !out.warnings.is_empty() && (config.options.multiplatform || out.matches.is_empty()) {
            anyhow::bail!(
                "Catalog lookup failed: {}",
                out.warnings
                    .iter()
                    .map(|w| format!("{}: {}", w.provider.as_str(), w.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        if !out.matches.is_empty() {
            out.status = MatchStatus::Matched;
        }
        Ok(out)
    }
}

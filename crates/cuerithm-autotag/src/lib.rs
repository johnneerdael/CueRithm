//! Automatic tagging adapters. GPL-3.0-only; the independent analysis engine is MIT.
mod asf;
mod config;
mod write;
pub use write::{AnalysisTags, StagedTagWrite, TagWritePlan};
mod image;
mod sidecar;
pub use sidecar::StagedSidecar;
mod input;
mod providers;
use anyhow::{Result, ensure};
pub use config::{AnalysisTagSource, AutotagConfig, NoMatchPolicy};
pub use input::TagInput;
use onetagger_tagger::MatchingUtils;
pub use onetagger_tagger::{
    FrameName, PlatformInfo, SupportedTag, TaggerConfig, Track as CatalogTrack,
    TrackMatch as CatalogMatch,
};
pub use providers::CatalogEndpoints;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Condvar, Mutex};
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
pub trait ProviderBackend: Send + Sync {
    fn id(&self) -> ProviderId;
    fn candidates(&self, input: &TagInput, config: &AutotagConfig) -> Result<Vec<CatalogMatch>>;
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
    fn acquire(&self, limit: usize) -> Result<Permit<'_>> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("Provider gate interrupted"))?;
        while *active >= limit {
            active = self
                .changed
                .wait(active)
                .map_err(|_| anyhow::anyhow!("Provider gate interrupted"))?;
        }
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
pub struct Tagger {
    backends: Vec<Backend>,
}
impl Tagger {
    pub fn new() -> Result<Self> {
        Self::with_endpoints(CatalogEndpoints::default())
    }
    pub fn with_endpoints(endpoints: CatalogEndpoints) -> Result<Self> {
        Self::with_backends(providers::backends(endpoints)?)
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
        config.validate()?;
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
                let _permit = b.gate.acquire(limit)?;
                let mut found = b.source.candidates(input, config)?;
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
                let mut matched = found.remove(0);
                b.source.extend(&mut matched.track, config)?;
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

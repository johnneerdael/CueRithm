use crate::{AutotagConfig, CatalogMatch, CatalogTrack, ProviderBackend, ProviderId, TagInput};
use anyhow::{Result, ensure};
use onetagger_selected_providers::{
    beatport::{Beatport, BeatportBuilder, BeatportTokenCache},
    http::{CatalogHttp, ChallengeStore},
    traxsource::{Traxsource, TraxsourceBuilder},
};
use onetagger_tagger::{AutotaggerSource, AutotaggerSourceBuilder, MatchingUtils, PlatformInfo};
use reqwest::blocking::Client;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
#[derive(Clone)]
pub struct CatalogEndpoints {
    web: String,
    api: String,
    auth: String,
    traxsource: String,
    timeout: Duration,
}
impl Default for CatalogEndpoints {
    fn default() -> Self {
        Self {
            web: "https://www.beatport.com".into(),
            api: "https://api.beatport.com".into(),
            auth: "https://account.beatport.com".into(),
            traxsource: "https://www.traxsource.com".into(),
            timeout: Duration::from_secs(30),
        }
    }
}
impl CatalogEndpoints {
    /// Local HTTP fixtures only; production addresses are fixed and never exposed as app settings.
    pub fn for_local_server(url: &str, timeout: Duration) -> Result<Self> {
        let parsed = reqwest::Url::parse(url)?;
        ensure!(
            parsed.scheme() == "http"
                && matches!(parsed.host_str(), Some("127.0.0.1" | "[::1]" | "localhost")),
            "Fixture endpoint must be loopback HTTP"
        );
        ensure!(!timeout.is_zero(), "Invalid request timeout");
        let base = url.trim_end_matches('/').to_owned();
        Ok(Self {
            web: base.clone(),
            api: base.clone(),
            auth: base.clone(),
            traxsource: base,
            timeout,
        })
    }
}
pub(crate) fn options() -> Vec<PlatformInfo> {
    vec![
        BeatportBuilder::new().info(),
        TraxsourceBuilder::new().info(),
    ]
}
pub(crate) fn backends(
    endpoints: CatalogEndpoints,
    store: Arc<dyn ChallengeStore>,
) -> Result<Vec<Arc<dyn ProviderBackend>>> {
    let mut out: Vec<Arc<dyn ProviderBackend>> = Vec::new();
    for id in [ProviderId::Beatport, ProviderId::Traxsource] {
        let agent = if id == ProviderId::Beatport {
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36"
        } else {
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:85.0) Gecko/20100101 Firefox/85.0"
        };
        let client = Client::builder()
            .user_agent(agent)
            .timeout(endpoints.timeout)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        out.push(Arc::new(Native {
            id,
            client,
            endpoints: endpoints.clone(),
            token: Arc::new(BeatportTokenCache::default()),
            store: store.clone(),
        }));
    }
    Ok(out)
}
struct Native {
    id: ProviderId,
    client: Client,
    endpoints: CatalogEndpoints,
    token: Arc<BeatportTokenCache>,
    store: Arc<dyn ChallengeStore>,
}
impl Native {
    fn source(&self, cancel: Arc<AtomicBool>, config: &AutotagConfig) -> Box<dyn AutotaggerSource> {
        match self.id {
            ProviderId::Beatport => Box::new(
                Beatport::with_endpoints(
                    self.token.clone(),
                    self.client.clone(),
                    &self.endpoints.web,
                    &self.endpoints.api,
                    &self.endpoints.auth,
                )
                .with_cancel(cancel),
            ),
            ProviderId::Traxsource => Box::new(
                Traxsource::with_endpoints(self.client.clone(), &self.endpoints.traxsource)
                    .with_http(
                        CatalogHttp::new(self.client.clone(), self.id.as_str(), self.store.clone()),
                        config.solver.clone(),
                        cancel,
                    ),
            ),
        }
    }
}
impl ProviderBackend for Native {
    fn album(&self, id: &str, config: &AutotagConfig) -> Result<Option<crate::CatalogAlbum>> {
        self.album_cancellable(id, config, &Arc::new(AtomicBool::new(false)))
    }
    fn album_cancellable(
        &self,
        id: &str,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<crate::CatalogAlbum>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        self.source(cancel.clone(), config)
            .get_album(id, &config.options)
            .map_err(|e| native_failure(self.id, e))
    }
    fn stage_timeout(&self, config: &AutotagConfig) -> Duration {
        if self.id == ProviderId::Traxsource
            && config
                .solver
                .endpoint
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        {
            // Search, track expansion and release expansion can each need two
            // solver attempts. Solver errors are terminal to the outer retry
            // layer, so these attempts are never multiplied by its retry count.
            let page = 2 * (config.solver.timeout_seconds.min(300) + 10) + 1;
            Duration::from_secs(95 + 3 * page)
        } else {
            Duration::from_secs(95)
        }
    }
    fn id(&self) -> ProviderId {
        self.id
    }
    fn candidates(&self, input: &TagInput, config: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        self.candidates_cancellable(input, config, &Arc::new(AtomicBool::new(false)))
    }
    fn candidates_cancellable(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        let mut info = input.audio_info(config)?;
        if !config.options.match_by_id {
            info.tags.remove("BEATPORT_TRACK_ID");
            info.isrc = None;
        }
        let mut found = self
            .source(cancel.clone(), config)
            .match_track(&info, &config.options)
            .map_err(|e| native_failure(self.id, e))?;
        found.retain(|m| MatchingUtils::match_duration(&info, &m.track, &config.options));
        Ok(found)
    }
    fn extend(&self, track: &mut CatalogTrack, config: &AutotagConfig) -> Result<()> {
        self.extend_cancellable(track, config, &Arc::new(AtomicBool::new(false)))
    }
    fn extend_cancellable(
        &self,
        track: &mut CatalogTrack,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        self.source(cancel.clone(), config)
            .extend_track(track, &config.options)
            .map_err(|e| native_failure(self.id, e))
    }
}

// These exact parser messages belong to our pinned adapter; unknown errors stay unknown.
fn native_failure(provider: ProviderId, error: anyhow::Error) -> anyhow::Error {
    if provider == ProviderId::Traxsource
        && crate::retry::classify(&error) == crate::ProviderFailureKind::Unknown
    {
        let message = error.to_string();
        if message.starts_with("Malformed Traxsource page:")
            || [
                "Missing track title",
                "Missing duration",
                "Invalid track ID",
                "Invalid release ID",
                "Missing track ID",
                "No results!",
            ]
            .contains(&message.as_str())
        {
            return crate::BackendFailure::new(crate::ProviderFailureKind::Parse, message).into();
        }
    }
    error
}

#[cfg(test)]
mod solver_budget_tests {
    use super::*;
    #[test]
    fn solver_budget_covers_all_pages_without_changing_direct_api_deadline() {
        let native = Native {
            id: ProviderId::Traxsource,
            client: Client::new(),
            endpoints: CatalogEndpoints::default(),
            token: Arc::new(BeatportTokenCache::default()),
            store: Arc::new(crate::MemoryChallengeStore::default()),
        };
        let mut config = AutotagConfig::default();
        assert_eq!(native.stage_timeout(&config), Duration::from_secs(95));
        config.solver.endpoint = Some("http://localhost:8191".into());
        assert_eq!(native.stage_timeout(&config), Duration::from_secs(878));
        config.solver.timeout_seconds = 300;
        assert_eq!(native.stage_timeout(&config), Duration::from_secs(1958));
        let beatport = Native {
            id: ProviderId::Beatport,
            ..native
        };
        assert_eq!(beatport.stage_timeout(&config), Duration::from_secs(95));
    }
}

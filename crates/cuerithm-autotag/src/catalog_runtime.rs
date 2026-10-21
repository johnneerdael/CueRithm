//! Shared official-API transport, credential and durable request-start authority.
use anyhow::{Result, ensure};
use cuerithm_credentials::{
    CredentialError, CredentialRef, CredentialService, CredentialStatus,
    ProviderId as CredentialProviderId, Revocation, Secret, Vault, Verification,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

pub const MAX_CATALOG_JSON_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CATALOG_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CATALOG_CACHE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_CATALOG_CACHE_ENTRIES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogService {
    #[serde(rename = "musicbrainz")]
    MusicBrainz,
    CoverArtArchive,
    #[serde(rename = "sonovault")]
    SonoVault,
    Discogs,
    Beatport,
    Traxsource,
    #[serde(rename = "itunes")]
    ITunes,
    BpmSupreme,
    Spotify,
    Bandcamp,
    Deezer,
    #[serde(rename = "acoustid")]
    AcoustId,
    #[serde(rename = "audd")]
    AudD,
    #[serde(rename = "rapidapi")]
    RapidApi,
}
impl CatalogService {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MusicBrainz => "musicbrainz",
            Self::CoverArtArchive => "cover_art_archive",
            Self::SonoVault => "sonovault",
            Self::Discogs => "discogs",
            Self::Beatport => "beatport",
            Self::Traxsource => "traxsource",
            Self::ITunes => "itunes",
            Self::BpmSupreme => "bpm_supreme",
            Self::Spotify => "spotify",
            Self::Bandcamp => "bandcamp",
            Self::Deezer => "deezer",
            Self::AcoustId => "acoustid",
            Self::AudD => "audd",
            Self::RapidApi => "rapidapi",
        }
    }
}
#[derive(Clone)]
pub struct CatalogIdentity {
    user_agent: String,
}
impl CatalogIdentity {
    pub fn new(release: &str) -> Result<Self> {
        ensure!(
            !release.is_empty()
                && release.len() <= 96
                && release
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-_+".contains(&c)),
            "Invalid application release identity"
        );
        Ok(Self {
            user_agent: format!(
                "CueRithm/{release} (https://github.com/johnneerdael/Cuerithm/issues)"
            ),
        })
    }
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
}
#[derive(Debug, Clone)]
pub struct RateClaim {
    pub service: CatalogService,
    pub now_ms: i64,
    pub min_interval_ms: u64,
    pub window_ms: u64,
    pub max_starts: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartDecision {
    Granted,
    WaitUntil(i64),
}
#[derive(Debug, Clone)]
pub struct RateObservation {
    pub manual_hold: Option<bool>,
    pub service: CatalogService,
    pub not_before_ms: Option<i64>,
    pub limit: Option<u32>,
    pub remaining: Option<u32>,
    pub observed_ms: i64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub body: Vec<u8>,
    pub expires_ms: i64,
}
/// Implementations perform short atomic claims; never sleep under their storage lock.
pub trait CatalogState: Send + Sync {
    fn claim_start(&self, claim: &RateClaim) -> Result<StartDecision>;
    fn observe(&self, observation: &RateObservation) -> Result<()>;
    fn cache_get(&self, key: &str, now_ms: i64) -> Result<Option<CachedResponse>>;
    fn cache_put(&self, key: &str, response: &CachedResponse) -> Result<()>;
}
pub trait CatalogCredentials: Send + Sync {
    fn status(&self, provider: CredentialProviderId) -> Result<CredentialStatus>;
    fn save(&self, expected: &CredentialRef, secret: Secret) -> Result<CredentialStatus>;
    fn remove(&self, provider: CredentialProviderId) -> Result<CredentialStatus>;
    fn with_secret(
        &self,
        expected: &CredentialRef,
        operation: &mut dyn FnMut(&str, Revocation) -> Result<()>,
    ) -> Result<()>;
    fn record_verification(
        &self,
        expected: &CredentialRef,
        verification: Verification,
    ) -> Result<CredentialStatus>;
}
impl<V: Vault> CatalogCredentials for CredentialService<V> {
    fn status(&self, p: CredentialProviderId) -> Result<CredentialStatus> {
        Ok(CredentialService::status(self, p)?)
    }
    fn save(&self, r: &CredentialRef, s: Secret) -> Result<CredentialStatus> {
        Ok(CredentialService::save(self, r, s)?)
    }
    fn remove(&self, p: CredentialProviderId) -> Result<CredentialStatus> {
        Ok(CredentialService::remove(self, p)?)
    }
    fn with_secret(
        &self,
        r: &CredentialRef,
        op: &mut dyn FnMut(&str, Revocation) -> Result<()>,
    ) -> Result<()> {
        self.run(r, op)?
    }
    fn record_verification(&self, r: &CredentialRef, v: Verification) -> Result<CredentialStatus> {
        Ok(CredentialService::record_verification(self, r, v)?)
    }
}
/// Secrets are native-only and deliberately neither Debug nor serializable.
pub enum CatalogAuth {
    None,
    Credential(CredentialRef),
    Bearer {
        token: Secret,
        guard: Option<CredentialRef>,
    },
    Session {
        token: Secret,
        guard: CredentialRef,
    },
    Guard(CredentialRef),
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CatalogMethod {
    Get,
    Post,
}
pub struct CatalogHttpRequest {
    pub service: CatalogService,
    pub method: CatalogMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub max_bytes: usize,
    pub timeout_ms: u64,
}
#[derive(Clone, Default)]
pub struct CatalogHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
pub trait CatalogTransport: Send + Sync {
    fn execute(
        &self,
        request: CatalogHttpRequest,
        cancel: &Arc<AtomicBool>,
        revocation: Option<&Revocation>,
    ) -> Result<CatalogHttpResponse>;
}
pub trait CatalogClock: Send + Sync {
    fn now_ms(&self) -> i64;
    fn wait_until(
        &self,
        until_ms: i64,
        cancel: &Arc<AtomicBool>,
        revocation: Option<&Revocation>,
    ) -> Result<()>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogErrorKind {
    Cancelled,
    CredentialMissing,
    CredentialRevoked,
    VaultUnavailable,
    InvalidCredential,
    EmailUnverified,
    Restricted,
    MockCredential,
    RateLimited,
    QuotaExhausted,
    RateOrQuotaLimited,
    Timeout,
    Transport,
    InvalidResponse,
    Malformed,
    TooLarge,
    ForbiddenOrigin,
    CloudflareProtected,
    NotFound,
    Unavailable,
    Busy,
}
#[derive(Debug)]
pub struct CatalogError {
    pub service: CatalogService,
    pub kind: CatalogErrorKind,
    pub status: Option<u16>,
    pub retry_at_ms: Option<i64>,
    failure: crate::BackendFailure,
}
impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.failure.fmt(f)
    }
}
impl std::error::Error for CatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogProbe {
    pub credential: CredentialStatus,
    pub live: bool,
    pub authenticated_account: bool,
    pub kind: Option<CatalogErrorKind>,
    pub retry_at_ms: Option<i64>,
}
#[derive(Clone)]
pub struct CatalogRuntime {
    identity: CatalogIdentity,
    state: Arc<dyn CatalogState>,
    credentials: Arc<dyn CatalogCredentials>,
    transport: Arc<dyn CatalogTransport>,
    clock: Arc<dyn CatalogClock>,
    fixture_origin: Option<String>,
    control: Arc<Control>,
}
#[derive(Default)]
struct Control {
    flights: Mutex<std::collections::BTreeSet<String>>,
    grants: Mutex<std::collections::BTreeMap<CredentialProviderId, CredentialRef>>,
    probes: Mutex<std::collections::BTreeSet<CredentialProviderId>>,
    epochs: Mutex<std::collections::BTreeMap<CredentialProviderId, u64>>,
}
impl CatalogRuntime {
    pub fn new(
        identity: CatalogIdentity,
        state: Arc<dyn CatalogState>,
        credentials: Arc<dyn CatalogCredentials>,
    ) -> Result<Self> {
        Ok(Self {
            identity,
            state,
            credentials,
            transport: Arc::new(transport::HttpTransport),
            clock: Arc::new(SystemClock),
            fixture_origin: None,
            control: Arc::new(Control::default()),
        })
    }
    pub fn development() -> Result<Self> {
        Self::new(
            CatalogIdentity::new("development")?,
            Arc::new(MemoryCatalogState::default()),
            Arc::new(MissingCredentials),
        )
    }
    /// Explicit trusted-Rust fixture helper; no installed configuration exposes origin override.
    pub fn for_local_server(base: &str) -> Result<Self> {
        Self::development()?.with_local_server(base)
    }
    pub fn with_local_server(mut self, base: &str) -> Result<Self> {
        let url = reqwest::Url::parse(base)?;
        ensure!(
            url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "::1"))
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "Fixture origin must be literal loopback HTTP"
        );
        self.fixture_origin = Some(base.trim_end_matches('/').into());
        Ok(self)
    }
    pub fn with_transport(mut self, transport: Arc<dyn CatalogTransport>) -> Self {
        self.transport = transport;
        self
    }
    pub fn with_clock(mut self, clock: Arc<dyn CatalogClock>) -> Self {
        self.clock = clock;
        self
    }
    pub fn identity(&self) -> &CatalogIdentity {
        &self.identity
    }
    pub fn credential_status(&self, p: CredentialProviderId) -> Result<CredentialStatus> {
        self.credentials.status(p)
    }
    pub fn credential_save(&self, r: &CredentialRef, s: Secret) -> Result<CredentialStatus> {
        *self
            .control
            .epochs
            .lock()
            .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
            .entry(r.provider)
            .or_default() += 1;
        self.control
            .grants
            .lock()
            .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
            .remove(&r.provider);
        self.credentials.save(r, s)
    }
    pub fn credential_remove(&self, p: CredentialProviderId) -> Result<CredentialStatus> {
        *self
            .control
            .epochs
            .lock()
            .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
            .entry(p)
            .or_default() += 1;
        self.control
            .grants
            .lock()
            .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
            .remove(&p);
        self.credentials.remove(p)
    }
    pub fn with_credential<R>(
        &self,
        r: &CredentialRef,
        op: impl FnOnce(&str, Revocation) -> Result<R>,
    ) -> Result<R> {
        let mut operation = Some(op);
        let mut result = None;
        self.credentials.with_secret(r, &mut |secret, revocation| {
            let op = operation
                .take()
                .ok_or_else(|| anyhow::anyhow!("Credential callback invoked twice"))?;
            result = Some(op(secret, revocation)?);
            Ok(())
        })?;
        result.ok_or_else(|| anyhow::anyhow!("Credential callback was not invoked"))
    }
    pub fn record_verification(
        &self,
        r: &CredentialRef,
        v: Verification,
    ) -> Result<CredentialStatus> {
        self.credentials.record_verification(r, v)
    }
    pub fn is_authenticated(&self, r: &CredentialRef) -> Result<bool> {
        let status = self.credentials.status(r.provider)?;
        Ok(status.reference == *r
            && status.presence == cuerithm_credentials::Presence::Present
            && status.vault == cuerithm_credentials::VaultState::Ready
            && status.verification == Verification::Valid
            && self
                .control
                .grants
                .lock()
                .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
                .get(&r.provider)
                == Some(r))
    }
    pub fn test_connection(
        &self,
        r: &CredentialRef,
        cancel: &Arc<AtomicBool>,
    ) -> Result<CatalogProbe> {
        let service = match r.provider {
            CredentialProviderId::SonoVault => CatalogService::SonoVault,
            CredentialProviderId::Discogs => CatalogService::Discogs,
            _ => {
                return Err(CatalogError::new(
                    CatalogService::Discogs,
                    CatalogErrorKind::Restricted,
                    None,
                    None,
                )
                .into());
            }
        };
        let status = self.credentials.status(r.provider)?;
        ensure!(status.reference == *r, "Credential generation changed");
        {
            let mut probes = self
                .control
                .probes
                .lock()
                .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?;
            if !probes.insert(r.provider) {
                return Err(CatalogError::new(service, CatalogErrorKind::Busy, None, None).into());
            }
        }
        struct Lease<'a>(&'a Control, CredentialProviderId);
        impl Drop for Lease<'_> {
            fn drop(&mut self) {
                if let Ok(mut p) = self.0.probes.lock() {
                    p.remove(&self.1);
                }
            }
        }
        let _lease = Lease(&self.control, r.provider);
        let epoch = *self
            .control
            .epochs
            .lock()
            .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
            .entry(r.provider)
            .or_default();
        let mock = self.with_credential(r, |secret, _| {
            Ok(service == CatalogService::SonoVault && secret.starts_with("svk_test_"))
        })?;
        if mock {
            return Ok(CatalogProbe {
                credential: status,
                live: false,
                authenticated_account: false,
                kind: Some(CatalogErrorKind::MockCredential),
                retry_at_ms: None,
            });
        }
        let url = if service == CatalogService::Discogs {
            "https://api.discogs.com/oauth/identity"
        } else {
            "https://api.sonovault.now/v1/genres"
        };
        self.state.observe(&RateObservation {
            service,
            manual_hold: Some(false),
            not_before_ms: None,
            limit: None,
            remaining: None,
            observed_ms: self.clock.now_ms(),
        })?;
        let response = self
            .request(
                service,
                url,
                &[],
                Some(CatalogAuth::Credential(r.clone())),
                requests::RequestOptions {
                    method: CatalogMethod::Get,
                    body: vec![],
                    content: None,
                    image: false,
                    cache: false,
                },
                cancel,
            )
            .and_then(|bytes| {
                serde_json::from_slice::<Value>(&bytes).map_err(|_| {
                    CatalogError::new(service, CatalogErrorKind::Malformed, None, None).into()
                })
            });
        let mut epochs = self
            .control
            .epochs
            .lock()
            .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?;
        ensure!(
            *epochs.entry(r.provider).or_default() == epoch,
            "Credential verification superseded"
        );
        match response {
            Ok(value) => {
                let valid = if service == CatalogService::Discogs {
                    value
                        .get("id")
                        .and_then(Value::as_u64)
                        .is_some_and(|id| id > 0)
                        && value
                            .get("username")
                            .and_then(Value::as_str)
                            .is_some_and(|s| !s.is_empty() && s.len() <= 256)
                } else {
                    value.get("genres").and_then(Value::as_array).is_some()
                };
                if !valid {
                    return Err(CatalogError::new(
                        service,
                        CatalogErrorKind::Malformed,
                        None,
                        None,
                    )
                    .into());
                }
                let credential = self
                    .credentials
                    .record_verification(r, Verification::Valid)?;
                if service == CatalogService::Discogs {
                    self.control
                        .grants
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
                        .insert(r.provider, r.clone());
                }
                Ok(CatalogProbe {
                    credential,
                    live: true,
                    authenticated_account: service == CatalogService::Discogs,
                    kind: None,
                    retry_at_ms: None,
                })
            }
            Err(e) => {
                let typed = e.downcast_ref::<CatalogError>();
                let kind = typed.map(|e| e.kind).unwrap_or(CatalogErrorKind::Transport);
                let v = match kind {
                    CatalogErrorKind::InvalidCredential => Verification::Invalid,
                    CatalogErrorKind::EmailUnverified => Verification::EmailUnverified,
                    CatalogErrorKind::Restricted => Verification::Restricted,
                    CatalogErrorKind::RateLimited | CatalogErrorKind::RateOrQuotaLimited => {
                        Verification::RateLimited
                    }
                    CatalogErrorKind::QuotaExhausted => Verification::QuotaExhausted,
                    _ => Verification::Untested,
                };
                let credential = self.credentials.record_verification(r, v)?;
                Ok(CatalogProbe {
                    credential,
                    live: false,
                    authenticated_account: false,
                    kind: Some(kind),
                    retry_at_ms: typed.and_then(|e| e.retry_at_ms),
                })
            }
        }
    }
    pub fn wait_start(&self, service: CatalogService, cancel: &Arc<AtomicBool>) -> Result<()> {
        self.start(service, cancel, None)
    }
    pub fn get_json(
        &self,
        service: CatalogService,
        url: &str,
        query: &[(String, String)],
        auth: impl Into<Option<CatalogAuth>>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Value> {
        let bytes = self.request(
            service,
            url,
            query,
            auth.into(),
            requests::RequestOptions {
                method: CatalogMethod::Get,
                body: vec![],
                content: None,
                image: false,
                cache: true,
            },
            cancel,
        )?;
        serde_json::from_slice(&bytes)
            .map_err(|_| CatalogError::new(service, CatalogErrorKind::Malformed, None, None).into())
    }
    pub fn get_text(
        &self,
        service: CatalogService,
        url: &str,
        query: &[(String, String)],
        auth: impl Into<Option<CatalogAuth>>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<String> {
        let bytes = self.request(
            service,
            url,
            query,
            auth.into(),
            requests::RequestOptions {
                method: CatalogMethod::Get,
                body: vec![],
                content: None,
                image: false,
                cache: true,
            },
            cancel,
        )?;
        String::from_utf8(bytes)
            .map_err(|_| CatalogError::new(service, CatalogErrorKind::Malformed, None, None).into())
    }
    pub fn get_bytes(
        &self,
        service: CatalogService,
        url: &str,
        query: &[(String, String)],
        auth: impl Into<Option<CatalogAuth>>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<u8>> {
        self.request(
            service,
            url,
            query,
            auth.into(),
            requests::RequestOptions {
                method: CatalogMethod::Get,
                body: vec![],
                content: None,
                image: true,
                cache: false,
            },
            cancel,
        )
    }
    pub fn post_json(
        &self,
        service: CatalogService,
        url: &str,
        body: &Value,
        auth: impl Into<Option<CatalogAuth>>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Value> {
        let body = serde_json::to_vec(body)?;
        ensure!(
            body.len() <= MAX_CATALOG_JSON_BYTES,
            "Catalog request too large"
        );
        let bytes = self.request(
            service,
            url,
            &[],
            auth.into(),
            requests::RequestOptions {
                method: CatalogMethod::Post,
                body,
                content: Some("application/json"),
                image: false,
                cache: false,
            },
            cancel,
        )?;
        serde_json::from_slice(&bytes)
            .map_err(|_| CatalogError::new(service, CatalogErrorKind::Malformed, None, None).into())
    }
    pub fn post_form(
        &self,
        service: CatalogService,
        url: &str,
        body: &[(String, String)],
        auth: impl Into<Option<CatalogAuth>>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Value> {
        let mut form = reqwest::Url::parse("https://form.invalid/")?;
        form.query_pairs_mut()
            .extend_pairs(body.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let bytes = self.request(
            service,
            url,
            &[],
            auth.into(),
            requests::RequestOptions {
                method: CatalogMethod::Post,
                body: form.query().unwrap_or("").as_bytes().to_vec(),
                content: Some("application/x-www-form-urlencoded"),
                image: false,
                cache: false,
            },
            cancel,
        )?;
        serde_json::from_slice(&bytes)
            .map_err(|_| CatalogError::new(service, CatalogErrorKind::Malformed, None, None).into())
    }
}
mod requests;
mod transport;
#[derive(Default)]
pub struct MemoryCatalogState {
    inner: Mutex<MemoryState>,
}
#[derive(Default)]
struct MemoryState {
    rates: std::collections::BTreeMap<CatalogService, MemoryRate>,
    cache: std::collections::BTreeMap<String, CachedResponse>,
}
#[derive(Default)]
struct MemoryRate {
    manual_hold: bool,
    manual_observed: i64,
    starts: std::collections::VecDeque<i64>,
    last: Option<i64>,
    defer: i64,
    limit: Option<u32>,
    remaining: Option<u32>,
    observed: i64,
}
impl CatalogState for MemoryCatalogState {
    fn claim_start(&self, c: &RateClaim) -> Result<StartDecision> {
        ensure!(
            c.max_starts > 0 && c.window_ms > 0,
            "Invalid request budget"
        );
        let mut state = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog state interrupted"))?;
        let r = state.rates.entry(c.service).or_default();
        if r.manual_hold {
            return Ok(StartDecision::WaitUntil(i64::MAX));
        }
        let window = c.window_ms.min(i64::MAX as u64) as i64;
        while r
            .starts
            .front()
            .is_some_and(|s| *s <= c.now_ms.saturating_sub(window))
        {
            r.starts.pop_front();
        }
        let recent = r.observed.saturating_add(window) > c.now_ms;
        let limit = if recent {
            r.limit.unwrap_or(c.max_starts).min(c.max_starts)
        } else {
            c.max_starts
        };
        let mut wait = r.defer;
        if let Some(last) = r.last {
            wait = wait.max(last.saturating_add(c.min_interval_ms.min(i64::MAX as u64) as i64));
        }
        if r.starts.len() >= limit as usize {
            wait = wait.max(
                r.starts
                    .front()
                    .copied()
                    .unwrap_or(c.now_ms)
                    .saturating_add(window),
            );
        }
        if recent && r.remaining == Some(0) {
            wait = wait.max(r.observed.saturating_add(window));
        }
        if wait > c.now_ms {
            return Ok(StartDecision::WaitUntil(wait));
        }
        r.starts.push_back(c.now_ms);
        r.last = Some(c.now_ms);
        if recent && let Some(v) = r.remaining.as_mut() {
            *v = v.saturating_sub(1);
        }
        Ok(StartDecision::Granted)
    }
    fn observe(&self, o: &RateObservation) -> Result<()> {
        let mut s = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog state interrupted"))?;
        let r = s.rates.entry(o.service).or_default();
        if o.observed_ms >= r.manual_observed
            && let Some(held) = o.manual_hold
        {
            r.manual_hold = held;
            r.manual_observed = o.observed_ms;
        }
        r.defer = r.defer.max(o.not_before_ms.unwrap_or(0));
        if o.observed_ms >= r.observed && (o.limit.is_some() || o.remaining.is_some()) {
            r.limit = o.limit;
            r.remaining = o.remaining;
            r.observed = o.observed_ms;
        }
        Ok(())
    }
    fn cache_get(&self, k: &str, now: i64) -> Result<Option<CachedResponse>> {
        let mut s = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog state interrupted"))?;
        s.cache.retain(|_, v| v.expires_ms > now);
        Ok(s.cache.get(k).cloned())
    }
    fn cache_put(&self, k: &str, v: &CachedResponse) -> Result<()> {
        ensure!(
            k.len() == 64
                && k.bytes().all(|c| c.is_ascii_hexdigit())
                && v.body.len() <= MAX_CATALOG_JSON_BYTES,
            "Invalid catalog cache entry"
        );
        let mut s = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("Catalog state interrupted"))?;
        s.cache.insert(k.into(), v.clone());
        while s.cache.len() > MAX_CATALOG_CACHE_ENTRIES
            || s.cache.values().map(|v| v.body.len()).sum::<usize>() > MAX_CATALOG_CACHE_BYTES
        {
            if let Some(k) = s
                .cache
                .iter()
                .min_by_key(|(_, v)| v.expires_ms)
                .map(|(k, _)| k.clone())
            {
                s.cache.remove(&k);
            } else {
                break;
            }
        }
        Ok(())
    }
}

struct MissingCredentials;
impl CatalogCredentials for MissingCredentials {
    fn status(&self, _p: CredentialProviderId) -> Result<CredentialStatus> {
        Err(CredentialError::Missing.into())
    }
    fn save(&self, _r: &CredentialRef, _s: Secret) -> Result<CredentialStatus> {
        Err(CredentialError::Missing.into())
    }
    fn remove(&self, _p: CredentialProviderId) -> Result<CredentialStatus> {
        Err(CredentialError::Missing.into())
    }
    fn with_secret(
        &self,
        _r: &CredentialRef,
        _op: &mut dyn FnMut(&str, Revocation) -> Result<()>,
    ) -> Result<()> {
        Err(CredentialError::Missing.into())
    }
    fn record_verification(
        &self,
        _r: &CredentialRef,
        _v: Verification,
    ) -> Result<CredentialStatus> {
        Err(CredentialError::Missing.into())
    }
}
struct SystemClock;
impl CatalogClock for SystemClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64
    }
    fn wait_until(
        &self,
        until: i64,
        cancel: &Arc<AtomicBool>,
        revocation: Option<&Revocation>,
    ) -> Result<()> {
        loop {
            ensure!(
                !cancel.load(Ordering::Relaxed) && !revocation.is_some_and(Revocation::is_revoked),
                "Tagging is stopping"
            );
            let remaining = until.saturating_sub(self.now_ms());
            if remaining <= 0 {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(remaining.min(25) as u64));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;

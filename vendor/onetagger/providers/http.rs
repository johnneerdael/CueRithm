//! Shared, opt-in transport for public catalog GETs. Never forward local credentials.
use anyhow::{Context, Result, bail, ensure};
use reqwest::{StatusCode, Url, blocking::Client, header::HeaderMap};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::Read,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const MAX_PAGE: usize = 8 * 1024 * 1024;
const MAX_ENVELOPE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SolverConfig {
    pub endpoint: Option<String>,
    pub timeout_seconds: u64,
}
impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            timeout_seconds: 120,
        }
    }
}
impl SolverConfig {
    pub fn api_url(&self) -> Result<Option<Url>> {
        let Some(endpoint) = self
            .endpoint
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            return Ok(None);
        };
        let mut url = Url::parse(endpoint).map_err(|_| anyhow!("Invalid solver URL"))?;
        ensure!(
            matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
            "Solver URL must use HTTP or HTTPS"
        );
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "Solver URL must not contain credentials, query parameters or a fragment"
        );
        let path = url.path().trim_end_matches('/');
        if path.ends_with("/v1") {
            url.set_path(&path.to_owned());
        } else {
            url.set_path(&format!("{path}/v1"));
        }
        Ok(Some(url))
    }
    pub fn validate(&self) -> Result<()> {
        self.api_url()?;
        ensure!(
            (1..=300).contains(&self.timeout_seconds),
            "Solver timeout must be between 1 and 300 seconds"
        );
        Ok(())
    }
}

/// A required, unavailable, unsupported or unresolved solver is a provider
/// availability failure. Its retries belong exclusively to this transport.
#[derive(Debug)]
pub struct SolverFailure(pub String);
impl std::fmt::Display for SolverFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for SolverFailure {}

pub trait ChallengeStore: Send + Sync {
    fn requires_solver(&self, source: &str) -> Result<bool>;
    fn mark_requires_solver(&self, source: &str) -> Result<()>;
}
#[derive(Default)]
pub struct MemoryChallengeStore(Mutex<BTreeSet<String>>);
impl ChallengeStore for MemoryChallengeStore {
    fn requires_solver(&self, source: &str) -> Result<bool> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow!("Source access state interrupted"))?
            .contains(source))
    }
    fn mark_requires_solver(&self, source: &str) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| anyhow!("Source access state interrupted"))?
            .insert(source.to_owned());
        Ok(())
    }
}

#[derive(Clone)]
pub struct CatalogHttp {
    client: Client,
    source: String,
    store: Arc<dyn ChallengeStore>,
}
impl CatalogHttp {
    pub fn new(client: Client, source: &str, store: Arc<dyn ChallengeStore>) -> Self {
        Self {
            client,
            source: source.into(),
            store,
        }
    }
    pub fn get_text(
        &self,
        url: &str,
        config: &SolverConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<String> {
        config.validate()?;
        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
        let target = Url::parse(url)?;
        ensure!(
            matches!(target.scheme(), "http" | "https")
                && target.username().is_empty()
                && target.password().is_none(),
            "Invalid catalog URL"
        );
        if !self.store.requires_solver(&self.source)? {
            let response = self
                .client
                .get(target.clone())
                .send()
                .map_err(|e| e.without_url())?;
            let status = response.status();
            let failure = response
                .error_for_status_ref()
                .err()
                .map(|e| e.without_url());
            let headers = response.headers().clone();
            // An explicit mitigation header is sufficient evidence. Do not wait
            // for a challenge body that may be slow, oversized or interrupted.
            if is_challenge(&headers, "") {
                drop(response);
            } else {
                ensure!(
                    response
                        .content_length()
                        .is_none_or(|n| n <= MAX_PAGE as u64),
                    "Catalog response exceeds size limit"
                );
                let mut bytes = Vec::new();
                response.take(MAX_PAGE as u64 + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() <= MAX_PAGE,
                    "Catalog response exceeds size limit"
                );
                let body = String::from_utf8(bytes)?;
                if !is_challenge(&headers, &body) {
                    ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
                    if let Some(error) = failure {
                        return Err(error.into());
                    }
                    ensure!(status.is_success(), "Catalog HTTP {status}");
                    return Ok(body);
                }
            }
            self.store.mark_requires_solver(&self.source)?;
        }
        let endpoint = config.api_url()?.ok_or_else(|| SolverFailure("Source requires a FlareSolverr-compatible solver; configure it in Settings → Automatic tagging".into()))?;
        // No cookies or browser identities are imported into the native client. The
        // solver fetches every page for a marked source and owns clearance reuse.
        for attempt in 0..2 {
            ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
            let result = solve(&endpoint, &target, config.timeout_seconds, cancel);
            match result {
                Err(error)
                    if attempt == 0 && error.downcast_ref::<TransientSolverFailure>().is_some() =>
                {
                    for _ in 0..5 {
                        ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
                result => {
                    return result.map_err(|error| {
                        if cancel.load(Ordering::Relaxed) {
                            error
                        } else {
                            SolverFailure(format!("{error:#}")).into()
                        }
                    });
                }
            }
        }
        unreachable!()
    }
}

/// Cloudflare's explicit mitigation header, or an interstitial plus its challenge
/// machinery. Merely using Cloudflare, mentioning it, or returning 403 is not enough.
pub fn is_challenge(headers: &HeaderMap, body: &str) -> bool {
    if headers
        .get("cf-mitigated")
        .is_some_and(|h| h == "challenge")
    {
        return true;
    }
    let body = body.to_ascii_lowercase();
    let machinery = body.contains("/cdn-cgi/challenge-platform/")
        || body.contains("_cf_chl_opt")
        || body.contains("cf-chl-");
    let interstitial = body.contains("<title>just a moment")
        || body.contains("<title>attention required!")
        || body.contains("checking your browser")
        || body.contains("enable javascript and cookies to continue");
    machinery && interstitial
}

#[derive(Debug)]
struct TransientSolverFailure;
impl std::fmt::Display for TransientSolverFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Solver timed out or is temporarily unavailable; retry the lookup")
    }
}
impl std::error::Error for TransientSolverFailure {}
#[derive(Deserialize)]
struct Envelope {
    status: String,
    #[serde(default)]
    message: String,
    solution: Option<Solution>,
}
#[derive(Deserialize)]
struct Solution {
    #[serde(default)]
    url: String,
    #[serde(default)]
    status: u16,
    #[serde(default)]
    response: String,
    #[serde(default)]
    headers: std::collections::HashMap<String, String>,
}

fn solve(endpoint: &Url, target: &Url, timeout: u64, cancel: &Arc<AtomicBool>) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let fetch = async {
            let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(timeout + 10)).build()?;
            let mut response = client.post(endpoint.clone()).json(&serde_json::json!({"cmd":"request.get", "url":target.as_str(), "maxTimeout":timeout * 1000})).send().await.map_err(|e| if e.is_timeout() || e.is_connect() || e.is_request() || e.is_body() { anyhow::Error::new(TransientSolverFailure) } else { anyhow!("Could not contact solver: {}", e.without_url()) })?;
            let status = response.status();
            ensure!(response.content_length().is_none_or(|n| n <= MAX_ENVELOPE as u64), "Solver response exceeds size limit");
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| if e.is_timeout() || e.is_connect() || e.is_request() || e.is_body() || e.is_decode() { anyhow::Error::new(TransientSolverFailure) } else { anyhow!("Could not read solver response") })? {
                ensure!(bytes.len() + chunk.len() <= MAX_ENVELOPE, "Solver response exceeds size limit"); bytes.extend_from_slice(&chunk);
            }
            let envelope = serde_json::from_slice::<Envelope>(&bytes);
            if matches!(status, StatusCode::TOO_MANY_REQUESTS | StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT) { return Err(TransientSolverFailure.into()); }
            let envelope = envelope.context("Invalid FlareSolverr-compatible response")?;
            if envelope.status != "ok" {
                let message = envelope.message.to_ascii_lowercase();
                if ["timeout", "timed out", "deadline", "pool saturated", "pool exhausted"].iter().any(|s| message.contains(s)) { return Err(TransientSolverFailure.into()); }
                // Do not echo untrusted server messages, which may include URLs or cookies.
                bail!("Solver could not retrieve the catalog page (HTTP {status}); check the solver service");
            }
            ensure!(status.is_success(), "Solver HTTP {status}");
            let solution = envelope.solution.context("Solver response is missing a solution")?;
            let final_url = Url::parse(&solution.url).context("Solver returned an invalid final URL")?;
            ensure!(target.origin() == final_url.origin(), "Solver redirected outside the catalog origin");
            let target_status = StatusCode::from_u16(solution.status)?;
            ensure!(target_status.is_success(), "Solved catalog HTTP {target_status}");
            ensure!(!solution.response.is_empty() && solution.response.len() <= MAX_PAGE, "Solver returned empty or oversized catalog content");
            let mut headers = HeaderMap::new();
            for (key, value) in solution.headers { if key.eq_ignore_ascii_case("cf-mitigated") { headers.insert("cf-mitigated", value.parse()?); } }
            ensure!(!is_challenge(&headers, &solution.response), "Solver returned an unresolved Cloudflare challenge");
            Ok(solution.response)
        };
        let cancelled = async { loop { if cancel.load(Ordering::Relaxed) { break; } tokio::time::sleep(Duration::from_millis(50)).await; } };
        tokio::select! { result = fetch => result, _ = cancelled => bail!("Tagging is stopping") }
    })
}

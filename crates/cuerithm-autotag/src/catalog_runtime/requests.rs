use super::*;
use sha2::{Digest, Sha256};
pub(super) struct RequestOptions {
    pub method: CatalogMethod,
    pub body: Vec<u8>,
    pub content: Option<&'static str>,
    pub image: bool,
    pub cache: bool,
}
impl CatalogError {
    pub(crate) fn new(
        service: CatalogService,
        kind: CatalogErrorKind,
        status: Option<u16>,
        retry_at_ms: Option<i64>,
    ) -> Self {
        use crate::ProviderFailureKind as P;
        let failure = crate::BackendFailure::new(
            match kind {
                CatalogErrorKind::RateLimited => P::RateLimited,
                CatalogErrorKind::Timeout => P::Timeout,
                CatalogErrorKind::Transport => P::Transport,
                CatalogErrorKind::CredentialMissing
                | CatalogErrorKind::CredentialRevoked
                | CatalogErrorKind::InvalidCredential
                | CatalogErrorKind::EmailUnverified
                | CatalogErrorKind::Restricted => P::Authentication,
                CatalogErrorKind::Malformed => P::Parse,
                _ => P::Unavailable,
            },
            format!("{}: {:?}", service.as_str(), kind),
        );
        Self {
            service,
            kind,
            status,
            retry_at_ms,
            failure,
        }
    }
}
fn check(service: CatalogService, cancel: &Arc<AtomicBool>, r: Option<&Revocation>) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CatalogError::new(service, CatalogErrorKind::Cancelled, None, None).into());
    }
    if r.is_some_and(Revocation::is_revoked) {
        return Err(
            CatalogError::new(service, CatalogErrorKind::CredentialRevoked, None, None).into(),
        );
    }
    Ok(())
}
fn suffix(host: &str, root: &str) -> bool {
    host == root
        || host
            .strip_suffix(root)
            .is_some_and(|prefix| prefix.ends_with('.'))
}
impl CatalogRuntime {
    pub(super) fn allowed(
        &self,
        service: CatalogService,
        raw: &str,
        image: bool,
    ) -> Result<reqwest::Url> {
        let u = reqwest::Url::parse(raw).map_err(|_| {
            CatalogError::new(service, CatalogErrorKind::ForbiddenOrigin, None, None)
        })?;
        let h = u.host_str().unwrap_or("");
        let fixture = self
            .fixture_origin
            .as_ref()
            .is_some_and(|f| raw.starts_with(&format!("{f}/")));
        let allowed = if image {
            match service {
                CatalogService::Beatport => suffix(h, "beatport.com"),
                CatalogService::Traxsource => suffix(h, "traxsource.com"),
                CatalogService::CoverArtArchive => h == "coverartarchive.org",
                CatalogService::Discogs => matches!(h, "img.discogs.com" | "i.discogs.com"),
                CatalogService::ITunes => suffix(h, "mzstatic.com"),
                CatalogService::Deezer => suffix(h, "dzcdn.net"),
                CatalogService::Bandcamp => h == "f4.bcbits.com",
                CatalogService::Spotify => h == "i.scdn.co",
                _ => false,
            }
        } else {
            match service {
                CatalogService::MusicBrainz => h == "musicbrainz.org",
                CatalogService::CoverArtArchive => h == "coverartarchive.org",
                CatalogService::SonoVault => h == "api.sonovault.now",
                CatalogService::Discogs => h == "api.discogs.com",
                CatalogService::Beatport => matches!(h, "api.beatport.com" | "www.beatport.com"),
                CatalogService::Traxsource => h == "www.traxsource.com",
                CatalogService::ITunes => h == "itunes.apple.com",
                CatalogService::BpmSupreme => matches!(
                    h,
                    "api.bpmsupreme.com"
                        | "api.download.bpmsupreme.com"
                        | "api.bpmmusic.io"
                        | "app.bpmsupreme.com"
                ),
                CatalogService::Spotify => matches!(h, "api.spotify.com" | "accounts.spotify.com"),
                CatalogService::Bandcamp => suffix(h, "bandcamp.com"),
                CatalogService::Deezer => h == "api.deezer.com",
                CatalogService::AcoustId => h == "api.acoustid.org",
                CatalogService::AudD => h == "api.audd.io",
                CatalogService::RapidApi => false,
            }
        };
        if (!fixture
            && (!allowed || u.scheme() != "https" || u.port_or_known_default() != Some(443)))
            || !u.username().is_empty()
            || u.password().is_some()
            || u.fragment().is_some()
        {
            return Err(
                CatalogError::new(service, CatalogErrorKind::ForbiddenOrigin, None, None).into(),
            );
        }
        if service == CatalogService::CoverArtArchive && !fixture {
            let parts = u
                .path()
                .split('/')
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>();
            ensure!(
                parts.len() >= 2 && parts[0] == "release" && crate::Mbid::new(parts[1]).is_ok(),
                "Invalid selected-release artwork request"
            );
        }
        Ok(u)
    }
    pub(super) fn start(
        &self,
        service: CatalogService,
        cancel: &Arc<AtomicBool>,
        revocation: Option<&Revocation>,
    ) -> Result<()> {
        let begin = self.clock.now_ms();
        loop {
            check(service, cancel, revocation)?;
            if self.fixture_origin.is_some() {
                return Ok(());
            }
            let (interval, max) = match service {
                CatalogService::SonoVault | CatalogService::ITunes => (3000, 20),
                CatalogService::Discogs => {
                    let valid = self
                        .control
                        .grants
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
                        .get(&CredentialProviderId::Discogs)
                        .cloned();
                    let grant = valid.is_some_and(|r| {
                        self.credentials
                            .status(CredentialProviderId::Discogs)
                            .is_ok_and(|s| {
                                s.reference == r
                                    && s.presence == cuerithm_credentials::Presence::Present
                                    && s.vault == cuerithm_credentials::VaultState::Ready
                                    && s.verification == Verification::Valid
                            })
                    });
                    (1000, if grant { 60 } else { 25 })
                }
                _ => (1000, 60),
            };
            match self.state.claim_start(&RateClaim {
                service,
                now_ms: self.clock.now_ms(),
                min_interval_ms: interval,
                window_ms: 60000,
                max_starts: max,
            })? {
                StartDecision::Granted => return Ok(()),
                StartDecision::WaitUntil(until) => {
                    if until == i64::MAX {
                        return Err(CatalogError::new(
                            service,
                            CatalogErrorKind::RateOrQuotaLimited,
                            None,
                            None,
                        )
                        .into());
                    }
                    if until.saturating_sub(begin) > 20000 {
                        return Err(CatalogError::new(
                            service,
                            CatalogErrorKind::RateLimited,
                            None,
                            Some(until),
                        )
                        .into());
                    }
                    self.clock
                        .wait_until(until, cancel, revocation)
                        .map_err(|_| {
                            CatalogError::new(
                                service,
                                if cancel.load(Ordering::Relaxed) {
                                    CatalogErrorKind::Cancelled
                                } else {
                                    CatalogErrorKind::CredentialRevoked
                                },
                                None,
                                None,
                            )
                        })?;
                }
            }
        }
    }
    pub(super) fn request(
        &self,
        service: CatalogService,
        raw: &str,
        query: &[(String, String)],
        auth: Option<CatalogAuth>,
        options: RequestOptions,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<u8>> {
        let RequestOptions {
            method,
            body,
            content,
            image,
            cache,
        } = options;
        ensure!(
            body.len() <= MAX_CATALOG_JSON_BYTES,
            "Catalog request too large"
        );
        check(service, cancel, None)?;
        let mut url = self.allowed(service, raw, image)?;
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        }
        let mut headers = vec![("User-Agent".into(), self.identity.user_agent().into())];
        if let Some(content) = content {
            headers.push(("Content-Type".into(), content.into()));
        }
        if service == CatalogService::Traxsource && image {
            headers.push(("Referer".into(), "https://www.traxsource.com/".into()));
        }
        if service == CatalogService::Deezer && !image {
            headers.push(("Accept-Language".into(), "en-US".into()));
            headers.push(("Content-Language".into(), "en-US".into()));
        }
        let cache_allowed = matches!(auth.as_ref(), None | Some(CatalogAuth::None));
        let execute = |headers: Vec<(String, String)>,
                       revocation: Option<Revocation>,
                       reference: Option<&CredentialRef>|
         -> Result<Vec<u8>> {
            check(service, cancel, revocation.as_ref())?;
            let mut digest = Sha256::new();
            digest.update(service.as_str());
            digest.update(url.as_str());
            if let Some(r) = reference {
                digest.update(serde_json::to_vec(r)?);
            }
            let key = format!("{:x}", digest.finalize());
            let can_cache = cache
                && cache_allowed
                && method == CatalogMethod::Get
                && !image
                && reference.is_none();
            if can_cache && let Some(cached) = self.state.cache_get(&key, self.clock.now_ms())? {
                check(service, cancel, revocation.as_ref())?;
                return Ok(cached.body);
            }
            struct Flight<'a>(&'a Control, Option<String>);
            impl Drop for Flight<'_> {
                fn drop(&mut self) {
                    if let Some(key) = &self.1
                        && let Ok(mut flights) = self.0.flights.lock()
                    {
                        flights.remove(key);
                    }
                }
            }
            let _flight = if can_cache {
                loop {
                    check(service, cancel, revocation.as_ref())?;
                    {
                        let mut flights = self.control.flights.lock().map_err(|_| {
                            anyhow::anyhow!("Catalog request authority interrupted")
                        })?;
                        if !flights.contains(&key) {
                            ensure!(flights.len() < 64, "Catalog request concurrency limit");
                            flights.insert(key.clone());
                            break Flight(&self.control, Some(key.clone()));
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    if let Some(cached) = self.state.cache_get(&key, self.clock.now_ms())? {
                        check(service, cancel, revocation.as_ref())?;
                        return Ok(cached.body);
                    }
                }
            } else {
                Flight(&self.control, None)
            };
            if can_cache && let Some(cached) = self.state.cache_get(&key, self.clock.now_ms())? {
                check(service, cancel, revocation.as_ref())?;
                return Ok(cached.body);
            }
            let mut actual = url.clone();
            if let Some(base) = &self.fixture_origin {
                let origin = reqwest::Url::parse(base)?;
                actual
                    .set_scheme(origin.scheme())
                    .map_err(|_| anyhow::anyhow!("Invalid fixture"))?;
                actual.set_host(origin.host_str())?;
                actual
                    .set_port(origin.port())
                    .map_err(|_| anyhow::anyhow!("Invalid fixture"))?;
            }
            let mbid = if service == CatalogService::CoverArtArchive {
                url.path().split('/').nth(2).map(str::to_owned)
            } else {
                None
            };
            for hop in 0..=3 {
                self.start(service, cancel, revocation.as_ref())?;
                check(service, cancel, revocation.as_ref())?;
                let response = self
                    .transport
                    .execute(
                        CatalogHttpRequest {
                            service,
                            method,
                            url: actual.to_string(),
                            headers: headers.clone(),
                            body: body.clone(),
                            max_bytes: if image {
                                MAX_CATALOG_IMAGE_BYTES
                            } else {
                                MAX_CATALOG_JSON_BYTES
                            },
                            timeout_ms: 20000,
                        },
                        cancel,
                        revocation.as_ref(),
                    )
                    .map_err(|e| {
                        if e.downcast_ref::<CatalogError>().is_some() {
                            e
                        } else {
                            CatalogError::new(
                                service,
                                if cancel.load(Ordering::Relaxed) {
                                    CatalogErrorKind::Cancelled
                                } else if revocation.as_ref().is_some_and(Revocation::is_revoked) {
                                    CatalogErrorKind::CredentialRevoked
                                } else if e
                                    .downcast_ref::<reqwest::Error>()
                                    .is_some_and(|e| e.is_timeout())
                                {
                                    CatalogErrorKind::Timeout
                                } else {
                                    CatalogErrorKind::Transport
                                },
                                None,
                                None,
                            )
                            .into()
                        }
                    })?;
                check(service, cancel, revocation.as_ref())?;
                let header = |name: &str| {
                    response
                        .headers
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(name))
                        .map(|(_, v)| v.as_str())
                };
                let now = self.clock.now_ms();
                let retry = header("Retry-After").and_then(|v| {
                    v.parse::<u64>()
                        .ok()
                        .map(|s| now.saturating_add(s.min(i64::MAX as u64 / 1000) as i64 * 1000))
                        .or_else(|| {
                            chrono::DateTime::parse_from_rfc2822(v)
                                .ok()
                                .map(|d| d.timestamp_millis())
                        })
                });
                let reset = header("RateLimit-Reset")
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|s| now.saturating_add(s.min(i64::MAX as u64 / 1000) as i64 * 1000));
                self.state.observe(&RateObservation {
                    manual_hold: if service == CatalogService::SonoVault
                        && response.status == 429
                        && retry.or(reset).is_none_or(|until| until <= now)
                    {
                        Some(true)
                    } else {
                        None
                    },
                    service,
                    not_before_ms: retry.or(if response.status == 429 { reset } else { None }),
                    limit: header(if service == CatalogService::Discogs {
                        "X-Discogs-Ratelimit"
                    } else {
                        "RateLimit-Limit"
                    })
                    .and_then(|s| s.parse().ok()),
                    remaining: header(if service == CatalogService::Discogs {
                        "X-Discogs-Ratelimit-Remaining"
                    } else {
                        "RateLimit-Remaining"
                    })
                    .and_then(|s| s.parse().ok()),
                    observed_ms: now,
                })?;
                if service == CatalogService::Bandcamp {
                    let challenge = header("cf-mitigated")
                        .is_some_and(|v| v.trim().eq_ignore_ascii_case("challenge"))
                        || (header("server")
                            .is_some_and(|v| v.to_ascii_lowercase().contains("cloudflare"))
                            && std::str::from_utf8(&response.body).is_ok_and(|body| {
                                body.contains("__cf_chl_opt")
                                    || body.contains("/cdn-cgi/challenge-platform/")
                                    || body.contains("cf_chl_")
                            }));
                    if challenge {
                        return Err(CatalogError::new(
                            service,
                            CatalogErrorKind::CloudflareProtected,
                            Some(response.status),
                            None,
                        )
                        .into());
                    }
                }
                if matches!(response.status, 301 | 302 | 303 | 307 | 308)
                    && image
                    && matches!(
                        service,
                        CatalogService::Beatport | CatalogService::Traxsource
                    )
                    && cache_allowed
                {
                    let target = header("Location")
                        .and_then(|location| actual.join(location).ok())
                        .ok_or_else(|| {
                            CatalogError::new(
                                service,
                                CatalogErrorKind::ForbiddenOrigin,
                                Some(response.status),
                                None,
                            )
                        })?;
                    if hop == 3
                        || target.origin() != url.origin()
                        || self.allowed(service, target.as_str(), true).is_err()
                    {
                        return Err(CatalogError::new(
                            service,
                            CatalogErrorKind::ForbiddenOrigin,
                            Some(response.status),
                            None,
                        )
                        .into());
                    }
                    actual = target;
                    continue;
                }
                if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
                    if service != CatalogService::CoverArtArchive
                        || hop == 3
                        || !matches!(auth_marker(reference), 0)
                    {
                        return Err(CatalogError::new(
                            service,
                            CatalogErrorKind::ForbiddenOrigin,
                            Some(response.status),
                            None,
                        )
                        .into());
                    }
                    let location = header("Location").ok_or_else(|| {
                        CatalogError::new(
                            service,
                            CatalogErrorKind::InvalidResponse,
                            Some(response.status),
                            None,
                        )
                    })?;
                    let target = actual.join(location).map_err(|_| {
                        CatalogError::new(service, CatalogErrorKind::ForbiddenOrigin, None, None)
                    })?;
                    let bound = mbid.as_ref().is_some_and(|id| {
                        target.path().starts_with(&format!("/download/mbid-{id}/"))
                    });
                    if target.scheme() != "https"
                        || !target.username().is_empty()
                        || target.password().is_some()
                        || target.fragment().is_some()
                        || target.port_or_known_default() != Some(443)
                        || !target.host_str().is_some_and(|h| suffix(h, "archive.org"))
                        || !bound
                    {
                        return Err(CatalogError::new(
                            service,
                            CatalogErrorKind::ForbiddenOrigin,
                            Some(response.status),
                            None,
                        )
                        .into());
                    }
                    actual = target;
                    continue;
                }
                if !(200..300).contains(&response.status) {
                    let kind = match response.status {
                        401 => CatalogErrorKind::InvalidCredential,
                        403 if service == CatalogService::SonoVault
                            && serde_json::from_slice::<Value>(&response.body)
                                .ok()
                                .and_then(|v| {
                                    v.get("error")
                                        .and_then(Value::as_str)
                                        .map(|s| s.eq_ignore_ascii_case("Email not verified"))
                                })
                                .unwrap_or(false) =>
                        {
                            CatalogErrorKind::EmailUnverified
                        }
                        403 => CatalogErrorKind::Restricted,
                        404 => CatalogErrorKind::NotFound,
                        429 if service == CatalogService::SonoVault => {
                            CatalogErrorKind::RateOrQuotaLimited
                        }
                        429 => CatalogErrorKind::RateLimited,
                        500..=599 => CatalogErrorKind::Transport,
                        _ => CatalogErrorKind::InvalidResponse,
                    };
                    if response.status == 401
                        && let Some(r) = reference
                    {
                        self.control
                            .grants
                            .lock()
                            .map_err(|_| anyhow::anyhow!("Credential authority interrupted"))?
                            .remove(&r.provider);
                        let _ = self
                            .credentials
                            .record_verification(r, Verification::Invalid);
                    }
                    if service == CatalogService::SonoVault
                        && let Some(r) = reference
                    {
                        let verification = match kind {
                            CatalogErrorKind::InvalidCredential => Some(Verification::Invalid),
                            CatalogErrorKind::EmailUnverified => {
                                Some(Verification::EmailUnverified)
                            }
                            CatalogErrorKind::Restricted => Some(Verification::Restricted),
                            CatalogErrorKind::RateOrQuotaLimited
                            | CatalogErrorKind::RateLimited => Some(Verification::RateLimited),
                            CatalogErrorKind::QuotaExhausted => Some(Verification::QuotaExhausted),
                            _ => None,
                        };
                        if let Some(v) = verification {
                            let _ = self.credentials.record_verification(r, v);
                        }
                    }
                    return Err(CatalogError::new(
                        service,
                        kind,
                        Some(response.status),
                        retry.or(reset),
                    )
                    .into());
                }
                if response.body.len()
                    > if image {
                        MAX_CATALOG_IMAGE_BYTES
                    } else {
                        MAX_CATALOG_JSON_BYTES
                    }
                {
                    return Err(
                        CatalogError::new(service, CatalogErrorKind::TooLarge, None, None).into(),
                    );
                }
                if can_cache {
                    self.state.cache_put(
                        &key,
                        &CachedResponse {
                            body: response.body.clone(),
                            expires_ms: now.saturating_add(900000),
                        },
                    )?;
                }
                return Ok(response.body);
            }
            unreachable!()
        };
        match auth.unwrap_or(CatalogAuth::None) {
            CatalogAuth::None => execute(headers, None, None),
            CatalogAuth::Credential(r) => {
                ensure!(
                    matches!(
                        (service, r.provider),
                        (CatalogService::SonoVault, CredentialProviderId::SonoVault)
                            | (CatalogService::Discogs, CredentialProviderId::Discogs)
                    ),
                    "Credential belongs to another provider"
                );
                ensure!(
                    !image
                        && (self.fixture_origin.is_some()
                            || matches!(
                                (service, url.host_str()),
                                (CatalogService::SonoVault, Some("api.sonovault.now"))
                                    | (CatalogService::Discogs, Some("api.discogs.com"))
                            )),
                    "API credentials require their exact API origin"
                );
                self.with_credential(&r, |secret, revoke| {
                    headers.push((
                        if service == CatalogService::SonoVault {
                            "x-api-key"
                        } else {
                            "Authorization"
                        }
                        .into(),
                        if service == CatalogService::SonoVault {
                            secret.into()
                        } else {
                            format!("Discogs token={secret}")
                        },
                    ));
                    execute(headers, Some(revoke), Some(&r))
                })
            }
            CatalogAuth::Guard(r) => {
                ensure!(
                    !image
                        && matches!(
                            (service, r.provider),
                            (CatalogService::SonoVault, CredentialProviderId::SonoVault)
                                | (CatalogService::Discogs, CredentialProviderId::Discogs)
                                | (CatalogService::BpmSupreme, CredentialProviderId::BpmSupreme)
                                | (CatalogService::Spotify, CredentialProviderId::Spotify)
                                | (CatalogService::AcoustId, CredentialProviderId::AcoustId)
                                | (CatalogService::AudD, CredentialProviderId::AudD)
                                | (CatalogService::RapidApi, CredentialProviderId::RapidApi)
                        ),
                    "Credential guard belongs to another provider"
                );
                self.with_credential(&r, |_, revoke| execute(headers, Some(revoke), Some(&r)))
            }
            CatalogAuth::Bearer { token, guard } => {
                ensure!(
                    matches!(service, CatalogService::Spotify),
                    "Bearer authentication forbidden for service"
                );
                ensure!(
                    !image
                        && (self.fixture_origin.is_some()
                            || matches!(
                                (service, url.host_str()),
                                (CatalogService::Spotify, Some("api.spotify.com"))
                            )),
                    "Bearer credentials require their exact API origin"
                );
                headers.push(("Authorization".into(), format!("Bearer {}", token.expose())));
                if let Some(r) = guard {
                    ensure!(
                        service == CatalogService::Spotify
                            && r.provider == CredentialProviderId::Spotify,
                        "Credential belongs to another provider"
                    );
                    self.with_credential(&r, |_, revoke| execute(headers, Some(revoke), Some(&r)))
                } else {
                    execute(headers, None, None)
                }
            }
            CatalogAuth::Session { token, guard } => {
                ensure!(
                    service == CatalogService::BpmSupreme
                        && guard.provider == CredentialProviderId::BpmSupreme,
                    "Session belongs to another provider"
                );
                ensure!(
                    !image
                        && (self.fixture_origin.is_some()
                            || url.host_str() == Some("api.download.bpmsupreme.com")),
                    "BPM session requires its exact download API origin"
                );
                ensure!(
                    token.expose().bytes().all(|b| b == 0x21
                        || (0x23..=0x2b).contains(&b)
                        || (0x2d..=0x3a).contains(&b)
                        || (0x3c..=0x5b).contains(&b)
                        || (0x5d..=0x7e).contains(&b)),
                    "Invalid BPM session cookie token"
                );
                headers.push(("Cookie".into(), format!("bpm_session={}", token.expose())));
                self.with_credential(&guard, |_, revoke| {
                    execute(headers, Some(revoke), Some(&guard))
                })
            }
        }
    }
}
fn auth_marker(r: Option<&CredentialRef>) -> u8 {
    u8::from(r.is_some())
}

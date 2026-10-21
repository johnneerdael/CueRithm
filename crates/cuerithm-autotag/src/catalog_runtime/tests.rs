use super::*;
#[test]
fn request_start_ledger_survives_runtime_and_enforces_full_window() {
    let state = MemoryCatalogState::default();
    let claim = RateClaim {
        service: CatalogService::Discogs,
        now_ms: 1000,
        min_interval_ms: 0,
        window_ms: 60000,
        max_starts: 25,
    };
    for _ in 0..25 {
        assert_eq!(state.claim_start(&claim).unwrap(), StartDecision::Granted);
    }
    assert_eq!(
        state.claim_start(&claim).unwrap(),
        StartDecision::WaitUntil(61000)
    );
    state
        .observe(&RateObservation {
            manual_hold: None,
            service: CatalogService::Discogs,
            not_before_ms: Some(90000),
            limit: None,
            remaining: None,
            observed_ms: 1000,
        })
        .unwrap();
    assert_eq!(
        state
            .claim_start(&RateClaim {
                now_ms: 61000,
                ..claim
            })
            .unwrap(),
        StartDecision::WaitUntil(90000)
    );
}
struct Response;
impl CatalogTransport for Response {
    fn execute(
        &self,
        r: CatalogHttpRequest,
        _: &Arc<AtomicBool>,
        _: Option<&Revocation>,
    ) -> Result<CatalogHttpResponse> {
        assert!(r.headers.iter().any(|(k, v)| k == "User-Agent"
            && v == "CueRithm/development (https://github.com/johnneerdael/Cuerithm/issues)"));
        if r.service == CatalogService::Deezer {
            for name in ["Accept-Language", "Content-Language"] {
                assert!(r.headers.iter().any(|(k, v)| k == name && v == "en-US"));
            }
        }
        Ok(CatalogHttpResponse {
            status: 200,
            body: b"{\"recordings\":[]}".to_vec(),
            headers: vec![],
        })
    }
}
#[test]
fn official_json_get_has_injected_identity_and_strict_origins() {
    let r = CatalogRuntime::development()
        .unwrap()
        .with_transport(Arc::new(Response));
    let c = Arc::new(AtomicBool::new(false));
    assert!(
        r.get_json(
            CatalogService::MusicBrainz,
            "https://evil.invalid/ws/2/recording",
            &[],
            None,
            &c
        )
        .is_err()
    );
    assert_eq!(
        r.get_json(
            CatalogService::MusicBrainz,
            "https://musicbrainz.org/ws/2/recording",
            &[],
            None,
            &c
        )
        .unwrap()["recordings"],
        serde_json::json!([])
    );
    r.get_json(
        CatalogService::Deezer,
        "https://api.deezer.com/search/track",
        &[],
        None,
        &c,
    )
    .unwrap();
}
#[test]
fn runtime_cancellation_precedes_transport() {
    let r = CatalogRuntime::development()
        .unwrap()
        .with_transport(Arc::new(Response));
    let c = Arc::new(AtomicBool::new(true));
    let e = r
        .get_json(
            CatalogService::MusicBrainz,
            "https://musicbrainz.org/ws/2/recording",
            &[],
            None,
            &c,
        )
        .unwrap_err();
    assert_eq!(
        e.downcast_ref::<CatalogError>().unwrap().kind,
        CatalogErrorKind::Cancelled
    );
}

#[derive(Default, Clone)]
pub(crate) struct VaultFixture(
    Arc<Mutex<std::collections::BTreeMap<CredentialProviderId, String>>>,
);
impl Vault for VaultFixture {
    fn contains(
        &self,
        p: CredentialProviderId,
    ) -> std::result::Result<bool, cuerithm_credentials::VaultError> {
        Ok(self.0.lock().unwrap().contains_key(&p))
    }
    fn load(
        &self,
        p: CredentialProviderId,
    ) -> std::result::Result<Option<Secret>, cuerithm_credentials::VaultError> {
        Ok(self.0.lock().unwrap().get(&p).map(|s| {
            if matches!(
                p,
                CredentialProviderId::BpmSupreme | CredentialProviderId::Spotify
            ) {
                Secret::structured(s.clone()).unwrap()
            } else {
                Secret::new(s.clone()).unwrap()
            }
        }))
    }
    fn store(
        &self,
        p: CredentialProviderId,
        s: &Secret,
    ) -> std::result::Result<(), cuerithm_credentials::VaultError> {
        self.0.lock().unwrap().insert(p, s.expose().into());
        Ok(())
    }
    fn remove(
        &self,
        p: CredentialProviderId,
    ) -> std::result::Result<(), cuerithm_credentials::VaultError> {
        self.0.lock().unwrap().remove(&p);
        Ok(())
    }
}
struct ProbeResponse;
impl CatalogTransport for ProbeResponse {
    fn execute(
        &self,
        r: CatalogHttpRequest,
        _: &Arc<AtomicBool>,
        _: Option<&Revocation>,
    ) -> Result<CatalogHttpResponse> {
        assert!(r.url.ends_with("/oauth/identity"));
        assert!(
            r.headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v == "Discogs token=fixture-personal")
        );
        Ok(CatalogHttpResponse {
            status: 200,
            body: br#"{"id":42,"username":"fixture-account"}"#.to_vec(),
            headers: vec![],
        })
    }
}
fn credential_runtime() -> (tempfile::TempDir, CatalogRuntime, CredentialRef) {
    let temp = tempfile::tempdir().unwrap();
    let store = cuerithm_credentials::state::FileStateStore::open(temp.path()).unwrap();
    let service = CredentialService::new(VaultFixture::default(), Arc::new(store)).unwrap();
    let old = service.status(CredentialProviderId::Discogs).unwrap();
    let saved = service
        .save(
            &old.reference,
            Secret::new("fixture-personal".into()).unwrap(),
        )
        .unwrap();
    let runtime = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(service),
    )
    .unwrap()
    .with_transport(Arc::new(ProbeResponse));
    (temp, runtime, saved.reference)
}
#[test]
fn discogs_sixty_requires_successful_current_account_probe() {
    let (_temp, r, reference) = credential_runtime();
    let result = r
        .test_connection(&reference, &Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert!(result.live && result.authenticated_account);
    assert_eq!(result.credential.verification, Verification::Valid);
    r.credential_remove(CredentialProviderId::Discogs).unwrap();
    assert!(
        r.test_connection(&reference, &Arc::new(AtomicBool::new(false)))
            .is_err()
    );
}

#[test]
fn mock_sonovault_key_never_claims_live_readiness() {
    let temp = tempfile::tempdir().unwrap();
    let service = CredentialService::new(
        VaultFixture::default(),
        Arc::new(cuerithm_credentials::state::FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let old = service.status(CredentialProviderId::SonoVault).unwrap();
    let saved = service
        .save(
            &old.reference,
            Secret::new("svk_test_fixture".into()).unwrap(),
        )
        .unwrap();
    let r = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(service),
    )
    .unwrap()
    .with_transport(Arc::new(ProbeResponse));
    let probe = r
        .test_connection(&saved.reference, &Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert!(!probe.live && !probe.authenticated_account);
    assert_eq!(probe.kind, Some(CatalogErrorKind::MockCredential));
}
#[test]
fn cache_coalesces_duplicate_public_gets_without_spending_another_start() {
    use std::sync::atomic::AtomicUsize;
    struct Counting(AtomicUsize);
    impl CatalogTransport for Counting {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(40));
            Ok(CatalogHttpResponse {
                status: 200,
                body: b"{}".to_vec(),
                headers: vec![],
            })
        }
    }
    let transport = Arc::new(Counting(AtomicUsize::new(0)));
    let r = CatalogRuntime::development()
        .unwrap()
        .with_transport(transport.clone());
    std::thread::scope(|s| {
        for _ in 0..4 {
            let r = r.clone();
            s.spawn(move || {
                r.get_json(
                    CatalogService::MusicBrainz,
                    "https://musicbrainz.org/ws/2/recording",
                    &[],
                    None,
                    &Arc::new(AtomicBool::new(false)),
                )
                .unwrap();
            });
        }
    });
    assert_eq!(transport.0.load(Ordering::SeqCst), 1);
}

#[test]
fn actual_http_body_wait_is_cancelled_without_waiting_for_timeout() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let cancel = Arc::new(AtomicBool::new(false));
    std::thread::scope(|scope| {
        let c = cancel.clone();
        scope.spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut input = [0u8; 4096];
            assert!(socket.read(&mut input).unwrap() > 0);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\n{")
                .unwrap();
            c.store(true, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(100));
        });
        let runtime = CatalogRuntime::for_local_server(&base).unwrap();
        let error = runtime
            .get_json(
                CatalogService::MusicBrainz,
                "https://musicbrainz.org/ws/2/recording",
                &[],
                None,
                &cancel,
            )
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<CatalogError>().unwrap().kind,
            CatalogErrorKind::Cancelled
        );
    });
}
#[test]
fn remove_during_probe_revokes_without_waiting_for_network() {
    struct Slow(Arc<std::sync::Barrier>);
    impl CatalogTransport for Slow {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            revocation: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            self.0.wait();
            while !revocation.unwrap().is_revoked() {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            anyhow::bail!("Fixture revoked")
        }
    }
    let (_temp, r, reference) = credential_runtime();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let r = r.with_transport(Arc::new(Slow(barrier.clone())));
    std::thread::scope(|scope| {
        let copy = r.clone();
        let refcopy = reference.clone();
        let worker =
            scope.spawn(move || copy.test_connection(&refcopy, &Arc::new(AtomicBool::new(false))));
        barrier.wait();
        r.credential_remove(CredentialProviderId::Discogs).unwrap();
        assert!(worker.join().unwrap().is_err());
    });
    assert!(!r.is_authenticated(&reference).unwrap());
}
#[test]
fn authenticated_redirect_is_not_forwarded_or_cached() {
    struct Redirect;
    impl CatalogTransport for Redirect {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            Ok(CatalogHttpResponse {
                status: 302,
                headers: vec![("Location".into(), "https://evil.invalid/collect".into())],
                body: vec![],
            })
        }
    }
    let (_temp, r, reference) = credential_runtime();
    let error = r
        .with_transport(Arc::new(Redirect))
        .get_json(
            CatalogService::Discogs,
            "https://api.discogs.com/releases/42",
            &[],
            CatalogAuth::Credential(reference),
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<CatalogError>().unwrap().kind,
        CatalogErrorKind::ForbiddenOrigin
    );
    assert!(!error.to_string().contains("fixture-personal"));
}

#[test]
fn discogs_personal_token_never_reaches_an_image_origin() {
    struct Bytes;
    impl CatalogTransport for Bytes {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            Ok(CatalogHttpResponse {
                status: 200,
                body: vec![1],
                headers: vec![],
            })
        }
    }
    let (_temp, r, reference) = credential_runtime();
    let e = r.with_transport(Arc::new(Bytes)).get_bytes(
        CatalogService::Discogs,
        "https://img.discogs.com/cover.jpg",
        &[],
        CatalogAuth::Credential(reference),
        &Arc::new(AtomicBool::new(false)),
    );
    assert!(
        e.is_err(),
        "API credential must not be sent to the separately approved image CDN"
    );
}

pub(crate) fn provider_input() -> crate::TagInput {
    crate::TagInput {
        path: "Credit - Signal (Dub).flac".into(),
        source_sha256: "1".repeat(64),
        duration_seconds: 300.,
        media: cuerithm_media::MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("artist".into(), "Credit".into()),
                ("title".into(), "Signal (Dub)".into()),
            ]
            .into(),
        },
    }
}

#[test]
fn documented_sonovault_email_error_is_actionable_and_sanitized() {
    struct Email;
    impl CatalogTransport for Email {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            Ok(CatalogHttpResponse {
                status: 403,
                headers: vec![],
                body: br#"{"error":"Email not verified"}"#.to_vec(),
            })
        }
    }
    let r = CatalogRuntime::development()
        .unwrap()
        .with_transport(Arc::new(Email));
    let error = r
        .get_json(
            CatalogService::SonoVault,
            "https://api.sonovault.now/v1/genres",
            &[],
            None,
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<CatalogError>().unwrap().kind,
        CatalogErrorKind::EmailUnverified
    );
    assert!(!error.to_string().contains("Email not verified"));
}

#[test]
fn bpm_session_uses_named_cookie_and_rejects_delimiter_injection() {
    struct Cookie;
    impl CatalogTransport for Cookie {
        fn execute(
            &self,
            r: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            assert!(
                r.headers
                    .iter()
                    .any(|(k, v)| k == "Cookie" && v == "bpm_session=fixture-session")
            );
            Ok(CatalogHttpResponse {
                status: 200,
                body: b"{}".to_vec(),
                headers: vec![],
            })
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let service = CredentialService::new(
        VaultFixture::default(),
        Arc::new(cuerithm_credentials::state::FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let old = service.status(CredentialProviderId::BpmSupreme).unwrap();
    let saved = service
        .save(
            &old.reference,
            Secret::structured(
                r#"{"email":"fixture@example.invalid","password":"two words"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
    let r = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(service),
    )
    .unwrap()
    .with_transport(Arc::new(Cookie));
    let cancel = Arc::new(AtomicBool::new(false));
    r.get_json(
        CatalogService::BpmSupreme,
        "https://api.download.bpmsupreme.com/v1/albums",
        &[],
        CatalogAuth::Session {
            token: Secret::new("fixture-session".into()).unwrap(),
            guard: saved.reference.clone(),
        },
        &cancel,
    )
    .unwrap();
    assert!(
        r.get_json(
            CatalogService::BpmSupreme,
            "https://api.download.bpmsupreme.com/v1/albums",
            &[],
            CatalogAuth::Session {
                token: Secret::new("x;admin=true".into()).unwrap(),
                guard: saved.reference
            },
            &cancel
        )
        .is_err()
    );
}

#[test]
fn ambiguous_sonovault_quota_requires_explicit_probe_before_more_metadata() {
    use std::sync::atomic::{AtomicI64, AtomicUsize};
    struct Clock(AtomicI64);
    impl CatalogClock for Clock {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
        fn wait_until(
            &self,
            until: i64,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<()> {
            self.0.store(until, Ordering::SeqCst);
            Ok(())
        }
    }
    struct Quota(AtomicUsize);
    impl CatalogTransport for Quota {
        fn execute(
            &self,
            r: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            let n = self.0.fetch_add(1, Ordering::SeqCst);
            Ok(if n == 0 {
                CatalogHttpResponse {
                    status: 429,
                    headers: vec![],
                    body: br#"{"error":"Allowance unavailable"}"#.to_vec(),
                }
            } else {
                CatalogHttpResponse {
                    status: 200,
                    headers: vec![],
                    body: if r.url.ends_with("/genres") {
                        br#"{"genres":[]}"#.to_vec()
                    } else {
                        br#"{"results":[]}"#.to_vec()
                    },
                }
            })
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let service = CredentialService::new(
        VaultFixture::default(),
        Arc::new(cuerithm_credentials::state::FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let old = service.status(CredentialProviderId::SonoVault).unwrap();
    let saved = service
        .save(
            &old.reference,
            Secret::new("svk_live_fixture".into()).unwrap(),
        )
        .unwrap();
    let transport = Arc::new(Quota(AtomicUsize::new(0)));
    let r = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(service),
    )
    .unwrap()
    .with_transport(transport.clone())
    .with_clock(Arc::new(Clock(AtomicI64::new(1000))));
    let cancel = Arc::new(AtomicBool::new(false));
    let get = || {
        r.get_json(
            CatalogService::SonoVault,
            "https://api.sonovault.now/v1/tracks/search",
            &[],
            CatalogAuth::Credential(saved.reference.clone()),
            &cancel,
        )
    };
    assert!(get().is_err());
    let error = get().unwrap_err();
    assert_eq!(
        error.downcast_ref::<CatalogError>().unwrap().kind,
        CatalogErrorKind::RateOrQuotaLimited
    );
    assert_eq!(
        transport.0.load(Ordering::SeqCst),
        1,
        "Ambiguous quota must not be retried once per track"
    );
    assert!(r.test_connection(&saved.reference, &cancel).unwrap().live);
    assert!(get().is_ok());
    assert_eq!(transport.0.load(Ordering::SeqCst), 3);
}

#[test]
fn explicit_bandcamp_challenge_is_distinct_from_generic_forbidden() {
    struct Forbidden(bool);
    impl CatalogTransport for Forbidden {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            Ok(CatalogHttpResponse {
                status: 403,
                headers: if self.0 {
                    vec![("cf-mitigated".into(), "challenge".into())]
                } else {
                    vec![]
                },
                body: b"Forbidden".to_vec(),
            })
        }
    }
    for (challenge, expected) in [
        (true, CatalogErrorKind::CloudflareProtected),
        (false, CatalogErrorKind::Restricted),
    ] {
        let r = CatalogRuntime::development()
            .unwrap()
            .with_transport(Arc::new(Forbidden(challenge)));
        let error = r
            .post_json(
                CatalogService::Bandcamp,
                "https://bandcamp.com/api/fuzzysearch/1/autocomplete_elastic",
                &serde_json::json!({"search_text":"Fixture"}),
                None,
                &Arc::new(AtomicBool::new(false)),
            )
            .unwrap_err();
        assert_eq!(error.downcast_ref::<CatalogError>().unwrap().kind, expected);
    }
}

#[test]
fn production_legacy_artwork_with_runtime_keeps_origins_referer_and_no_credentials() {
    struct FastClock(std::sync::atomic::AtomicI64);
    impl CatalogClock for FastClock {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
        fn wait_until(
            &self,
            until: i64,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<()> {
            self.0.store(until, Ordering::SeqCst);
            Ok(())
        }
    }
    impl Default for FastClock {
        fn default() -> Self {
            Self(std::sync::atomic::AtomicI64::new(1000))
        }
    }
    struct Covers(Vec<u8>);
    impl CatalogTransport for Covers {
        fn execute(
            &self,
            r: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            assert!(
                !r.headers
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("Authorization")
                        || k.eq_ignore_ascii_case("Cookie")
                        || k.eq_ignore_ascii_case("x-api-key"))
            );
            let referer = r
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("Referer"))
                .map(|(_, v)| v.as_str());
            if r.service == CatalogService::Traxsource {
                assert_eq!(referer, Some("https://www.traxsource.com/"));
            } else {
                assert_eq!(r.service, CatalogService::Beatport);
                assert!(referer.is_none());
            }
            if r.url.ends_with("start.png") {
                Ok(CatalogHttpResponse {
                    status: 302,
                    headers: vec![("Location".into(), "/final.png".into())],
                    body: vec![],
                })
            } else {
                assert!(r.url.ends_with("/final.png"));
                Ok(CatalogHttpResponse {
                    status: 200,
                    headers: vec![],
                    body: self.0.clone(),
                })
            }
        }
    }
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::new(2, 2))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let expected = png.into_inner();
    let runtime = CatalogRuntime::development()
        .unwrap()
        .with_transport(Arc::new(Covers(expected.clone())))
        .with_clock(Arc::new(FastClock::default()));
    for (provider, url) in [
        (
            crate::ProviderId::Beatport,
            "https://geo-media.beatport.com/start.png",
        ),
        (
            crate::ProviderId::Traxsource,
            "https://geo-static.traxsource.com/start.png",
        ),
    ] {
        let bytes = crate::fetch_catalog_artwork_with_runtime(
            &runtime,
            provider,
            url,
            &cuerithm_media::CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(bytes, expected);
    }
}

struct VolumoProbeResponse(u16);
impl CatalogTransport for VolumoProbeResponse {
    fn execute(
        &self,
        r: CatalogHttpRequest,
        _: &Arc<AtomicBool>,
        _: Option<&Revocation>,
    ) -> Result<CatalogHttpResponse> {
        assert_eq!(r.service, CatalogService::Volumo);
        assert!(r.url.ends_with("/api/pub/track_by_isrc/GBKQU2599555"));
        assert!(r.method == CatalogMethod::Get);
        assert!(
            r.headers
                .iter()
                .any(|(k, v)| k == "Authorization" && v == "Bearer fixture-personal")
        );
        Ok(CatalogHttpResponse {
            status: self.0,
            headers: vec![],
            body: if self.0 == 200 {
                br#"{"isrc":"GBKQU2599555","url":"https://volumo.com/track/5644962-dump-original-mix","status":"live","embed_html":"unused","embed_src":"https://volumo.com/player/track/5644962"}"#.to_vec()
            } else {
                vec![]
            },
        })
    }
}
fn volumo_runtime(
    transport: Arc<dyn CatalogTransport>,
) -> (tempfile::TempDir, CatalogRuntime, CredentialRef) {
    let temp = tempfile::tempdir().unwrap();
    let service = CredentialService::new(
        VaultFixture::default(),
        Arc::new(cuerithm_credentials::state::FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let before = service.status(CredentialProviderId::Volumo).unwrap();
    let saved = service
        .save(
            &before.reference,
            Secret::new("fixture-personal".into()).unwrap(),
        )
        .unwrap();
    let r = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(service),
    )
    .unwrap()
    .with_transport(transport);
    (temp, r, saved.reference)
}
#[test]
fn volumo_connection_uses_only_the_documented_bearer_metadata_probe() {
    let (_temp, r, reference) = volumo_runtime(Arc::new(VolumoProbeResponse(200)));
    let probe = r
        .test_connection(&reference, &Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert!(probe.live);
    assert!(!probe.authenticated_account);
    assert_eq!(probe.credential.verification, Verification::Valid);
}
#[test]
fn volumo_missing_and_gone_probe_resources_do_not_invalidate_a_key() {
    for (status, kind) in [
        (404, CatalogErrorKind::NotFound),
        (410, CatalogErrorKind::Gone),
    ] {
        let (_temp, r, reference) = volumo_runtime(Arc::new(VolumoProbeResponse(status)));
        let probe = r
            .test_connection(&reference, &Arc::new(AtomicBool::new(false)))
            .unwrap();
        assert!(!probe.live);
        assert_eq!(probe.credential.verification, Verification::Untested);
        assert_eq!(probe.kind, Some(kind));
    }
}
#[test]
fn volumo_credentials_and_unknown_routes_never_reach_public_transport() {
    struct Never;
    impl CatalogTransport for Never {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            panic!("Unapproved Volumo request reached transport")
        }
    }
    let (_temp, r, reference) = volumo_runtime(Arc::new(Never));
    let cancel = Arc::new(AtomicBool::new(false));
    for url in [
        "https://volumo.com/track/5644962-dump-original-mix",
        "https://volumo.com/img/640/image.jpg",
        "https://volumo.com/api/pub/identify/audio",
        "https://volumo.com/api/private/account",
    ] {
        assert!(
            r.get_json(
                CatalogService::Volumo,
                url,
                &[],
                CatalogAuth::Credential(reference.clone()),
                &cancel
            )
            .is_err()
        );
    }
    assert!(
        r.get_json(
            CatalogService::Volumo,
            "https://volumo.com/api/pub/track_by_isrc/GBKQU2599555",
            &[],
            None,
            &cancel
        )
        .is_err()
    );
}

#[test]
fn request_guard_rechecks_consent_after_pacing_before_transport() {
    use std::sync::atomic::{AtomicI64, AtomicUsize};
    struct RevokeDuringWait {
        now: AtomicI64,
        allowed: Arc<AtomicBool>,
    }
    impl CatalogClock for RevokeDuringWait {
        fn now_ms(&self) -> i64 {
            self.now.load(Ordering::SeqCst)
        }
        fn wait_until(
            &self,
            until: i64,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<()> {
            self.allowed.store(false, Ordering::SeqCst);
            self.now.store(until, Ordering::SeqCst);
            Ok(())
        }
    }
    struct CountRequests(AtomicUsize);
    impl CatalogTransport for CountRequests {
        fn execute(
            &self,
            _: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(CatalogHttpResponse {
                status: 200,
                body: br#"{"status":"ok","results":[]}"#.to_vec(),
                headers: vec![],
            })
        }
    }
    let allowed = Arc::new(AtomicBool::new(true));
    let calls = Arc::new(CountRequests(AtomicUsize::new(0)));
    let runtime = CatalogRuntime::development()
        .unwrap()
        .with_transport(calls.clone())
        .with_clock(Arc::new(RevokeDuringWait {
            now: AtomicI64::new(1000),
            allowed: allowed.clone(),
        }));
    let cancel = Arc::new(AtomicBool::new(false));
    let guard: CatalogRequestGuard = Arc::new(move || {
        ensure!(allowed.load(Ordering::SeqCst), "Consent revoked");
        Ok(())
    });
    let lookup = || {
        runtime.post_form(
            CatalogService::AcoustId,
            "https://api.acoustid.org/v2/lookup",
            &[("fingerprint".into(), "synthetic".into())],
            CatalogAuth::RequestGuard(guard.clone()),
            &cancel,
        )
    };
    lookup().unwrap();
    let error = lookup().unwrap_err();
    assert!(error.to_string().contains("Consent revoked"));
    assert_eq!(
        calls.0.load(Ordering::SeqCst),
        1,
        "revocation during the second request's pacing must prevent its transport call"
    );
}

use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
struct Server {
    url: String,
    seen: Arc<Mutex<Vec<(String, String)>>>,
    stop: Arc<AtomicBool>,
    accepted: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(handler: impl Fn(&str, &str) -> (u16, String) + Send + 'static) -> Self {
        Self::with_accepted_nonblocking(false, handler)
    }
    fn with_accepted_nonblocking(
        inherited_nonblocking: bool,
        handler: impl Fn(&str, &str) -> (u16, String) + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let log = seen.clone();
        let done = stop.clone();
        let accepted = Arc::new(AtomicBool::new(false));
        let accepted_signal = accepted.clone();
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !done.load(Ordering::Relaxed) && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        // Emulate BSD accept inheritance on every test platform.
                        if inherited_nonblocking {
                            socket.set_nonblocking(true).unwrap();
                        }
                        socket.set_nonblocking(false).unwrap();
                        accepted_signal.store(true, Ordering::Release);
                        socket
                            .set_read_timeout(Some(Duration::from_millis(500)))
                            .unwrap();
                        let mut bytes = Vec::new();
                        let mut block = [0; 4096];
                        let mut needed = None;
                        while needed.is_none_or(|n| bytes.len() < n) {
                            let n = match socket.read(&mut block) {
                                Ok(0) | Err(_) => break,
                                Ok(n) => n,
                            };
                            bytes.extend_from_slice(&block[..n]);
                            if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                                let h = String::from_utf8_lossy(&bytes[..end]);
                                let len = h
                                    .lines()
                                    .find_map(|line| {
                                        line.to_ascii_lowercase()
                                            .strip_prefix("content-length:")
                                            .and_then(|s| s.trim().parse::<usize>().ok())
                                    })
                                    .unwrap_or(0);
                                needed = Some(end + 4 + len);
                            }
                        }
                        // Connection probes may close without sending HTTP. Never
                        // dispatch them as catalog responses or pollute request proofs.
                        if needed.is_none_or(|length| bytes.len() < length) {
                            continue;
                        }
                        // Never retain or print authentication bodies/headers, including public client constants.
                        let line = String::from_utf8_lossy(&bytes)
                            .lines()
                            .next()
                            .unwrap_or("")
                            .to_owned();
                        let mut parts = line.split_whitespace();
                        let method = parts.next().unwrap_or("").to_owned();
                        let path = parts.next().unwrap_or("").to_owned();
                        log.lock().unwrap().push((method.clone(), path.clone()));
                        let (status, body) = handler(&method, &path);
                        let response = format!(
                            "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = socket.write_all(response.as_bytes());
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            url,
            seen,
            stop,
            accepted,
            thread: Some(handle),
        }
    }
}
#[test]
fn fixture_accepts_a_delayed_request_after_nonblocking_listener_accept() {
    let server = Server::with_accepted_nonblocking(true, |method, path| {
        assert_eq!((method, path), ("GET", "/delayed"));
        (200, "delayed request received".into())
    });
    let mut client =
        std::net::TcpStream::connect(server.url.trim_start_matches("http://")).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !server.accepted.load(Ordering::Acquire) {
        assert!(
            Instant::now() < deadline,
            "fixture did not accept the client"
        );
        thread::yield_now();
    }
    // Longer than the listener's 2ms polling interval, within the read timeout.
    thread::sleep(Duration::from_millis(25));
    client
        .write_all(b"GET /delayed HTTP/1.1\r\nHost: fixture\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    assert!(response.ends_with("delayed request received"));
    assert_eq!(
        server.seen.lock().unwrap().as_slice(),
        &[("GET".into(), "/delayed".into())]
    );
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
fn input() -> TagInput {
    TagInput {
        path: "fixture.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 240.,
        media: cuerithm_media::MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("artist".into(), "Artist".into()),
                ("title".into(), "Signal".into()),
            ]
            .into(),
        },
    }
}
fn config(id: ProviderId) -> AutotagConfig {
    let mut c = AutotagConfig::default();
    c.options.platforms = vec![id.as_str().into()];
    c.options.parse_filename = false;
    c.options.match_duration = false;
    c
}
fn port(id: ProviderId, server: &Server) -> CatalogPort {
    CatalogPort::new(
        id,
        Arc::new(CatalogRuntime::for_local_server(&server.url).unwrap()),
        Arc::new(crate::MemoryChallengeStore::default()),
    )
    .unwrap()
    .with_local_server(&server.url)
    .unwrap()
}
fn deezer_track() -> Value {
    json!({"id":1,"title_short":"Signal","title":"Signal","contributors":[{"name":"Artist"},{"name":"Guest"}],"artist":{"name":"Artist"},"isrc":"USAAA2100001","album":{"id":2,"title":"Release"},"duration":240})
}
#[test]
fn fixture_page_origins_are_literal_loopback_only() {
    let runtime = Arc::new(CatalogRuntime::development().unwrap());
    for url in [
        "http://localhost:9999",
        "https://127.0.0.1:9999",
        "http://user:pass@127.0.0.1:9999",
        "http://127.0.0.1:9999/path",
        "http://127.0.0.1:9999?secret=fixture",
    ] {
        assert!(
            CatalogPort::new(
                ProviderId::Bandcamp,
                runtime.clone(),
                Arc::new(crate::MemoryChallengeStore::default())
            )
            .unwrap()
            .with_local_server(url)
            .is_err()
        );
    }
}
#[test]
fn beatport_token_start_gate_runs_only_for_actual_refresh() {
    use onetagger_selected_providers::beatport::{Beatport, BeatportTokenCache};
    let server = Server::new(|_, _| {
        (
            200,
            json!({"access_token":"fixture-anonymous-token","expires_in":3600}).to_string(),
        )
    });
    let starts = Arc::new(AtomicUsize::new(0));
    let counted = starts.clone();
    let source = Beatport::with_endpoints(
        Arc::new(BeatportTokenCache::default()),
        reqwest::blocking::Client::new(),
        &server.url,
        &server.url,
        &server.url,
    )
    .with_start_gate(Arc::new(move |_| {
        counted.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }));
    source.update_token().unwrap();
    source.update_token().unwrap();
    assert_eq!(
        starts.load(Ordering::Relaxed),
        1,
        "cached tokens must not spend request starts; physical refresh must"
    );
    assert_eq!(server.seen.lock().unwrap().len(), 1);
}
#[test]
fn deezer_selected_album_cannot_change_under_the_same_recording() {
    let server = Server::new(|_, path| {
        if path.starts_with("/search/track?") {
            (200, json!({"data":[deezer_track()]}).to_string())
        } else if path == "/track/1" {
            (200, deezer_track().to_string())
        } else {
            (
                200,
                json!({"id":999,"title":"Another album","label":"Wrong","nb_tracks":4}).to_string(),
            )
        }
    });
    let port = port(ProviderId::Deezer, &server);
    let mut selected = port
        .candidates(&input(), &config(ProviderId::Deezer))
        .unwrap()
        .remove(0)
        .track;
    let error = port
        .extend_cancellable(
            &mut selected,
            &config(ProviderId::Deezer),
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("album identity"),
        "expected identity refusal, got {error:#}; requests={:?}",
        server.seen.lock().unwrap()
    );
}

#[test]
fn deezer_isrc_lookup_uses_documented_direct_path_then_preserves_full_credits() {
    let server = Server::new(|_, path| {
        if path == "/track/isrc:USAAA2100001" || path == "/track/1" {
            (200, deezer_track().to_string())
        } else if path == "/album/2" {
            (200,json!({"id":2,"title":"Release","label":"Label","nb_tracks":4,"contributors":[{"name":"Album Artist"}]}).to_string())
        } else {
            (200, json!({"data":[]}).to_string())
        }
    });
    let port = port(ProviderId::Deezer, &server);
    let mut input = input();
    input
        .media
        .tags
        .insert("isrc".into(), "USAAA2100001".into());
    let mut cfg = config(ProviderId::Deezer);
    cfg.options.match_by_id = true;
    let found = port.candidates(&input, &cfg).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].reason, onetagger_tagger::MatchReason::ISRC);
    let mut selected = found[0].track.clone();
    port.extend_cancellable(&mut selected, &cfg, &Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert_eq!(selected.artists, vec!["Artist", "Guest"]);
    assert_eq!(selected.album_artists, vec!["Album Artist"]);
    assert_eq!(selected.label.as_deref(), Some("Label"));
    assert!(server.seen.lock().unwrap()[0].1.starts_with("/track/isrc:"));
}
#[test]
fn bandcamp_resolves_credit_before_rejecting_a_label_uploader() {
    let server = Server::new(|method, path| {
        if method == "POST" && path == "/api/bcsearch_public_api/1/autocomplete_elastic" {
            (200,json!({"auto":{"results":[{"id":7,"name":"Signal","band_name":"Label Uploader","album_id":8,"item_url_path":"https://label.bandcamp.com/track/signal"}]}}).to_string())
        } else {
            (200,r#"<script type="application/ld+json">{"@type":"MusicRecording","name":"Signal","url":"https://label.bandcamp.com/track/signal","byArtist":{"name":"Artist"},"duration":"PT4M","datePublished":"12 Mar 2026 00:00:00 GMT"}</script>"#.into())
        }
    });
    // A speculative connection with no request must not consume a response or
    // become the first observed catalog request.
    drop(std::net::TcpStream::connect(server.url.trim_start_matches("http://")).unwrap());
    let port = port(ProviderId::Bandcamp, &server);
    let tagger = crate::Tagger::with_backends(vec![Arc::new(port)]).unwrap();
    let found = tagger
        .match_track(&input(), &config(ProviderId::Bandcamp))
        .unwrap()
        .matches;
    assert_eq!(
        found.len(),
        1,
        "storefront uploader is not authoritative artist credit"
    );
    assert_eq!(found[0].track.artists, vec!["Artist"]);
    assert_eq!(
        found[0].track.release_date,
        chrono::NaiveDate::from_ymd_opt(2026, 3, 12)
    );
    let seen = server.seen.lock().unwrap();
    assert_eq!(
        seen[0],
        (
            "POST".into(),
            "/api/bcsearch_public_api/1/autocomplete_elastic".into()
        )
    );
    assert!(!seen.iter().any(|(_, path)| path.starts_with("/search?")));
}
#[test]
fn bandcamp_expansion_never_replaces_a_selected_credited_artist() {
    let server = Server::new(|_, _| {
        (200,r#"<script type="application/ld+json">{"@type":"MusicRecording","name":"Signal","byArtist":{"name":"Unrelated Artist"}}</script>"#.into())
    });
    let port = port(ProviderId::Bandcamp, &server);
    let mut selected = CatalogTrack {
        platform: "bandcamp".into(),
        title: "Signal".into(),
        artists: vec!["Artist".into()],
        url: "https://label.bandcamp.com/track/signal".into(),
        ..Default::default()
    };
    assert!(
        port.extend_cancellable(
            &mut selected,
            &config(ProviderId::Bandcamp),
            &Arc::new(AtomicBool::new(false))
        )
        .is_err()
    );
}
#[test]
fn itunes_candidate_to_extension_retains_music_credit_release_and_position() {
    let server = Server::new(|_, _| {
        (200,json!({"results":[{"kind":"podcast"},{"kind":"song","trackId":1,"trackName":"Signal","artistName":"Artist","collectionId":2,"collectionName":"Release","trackNumber":3,"discNumber":2,"trackCount":8,"trackTimeMillis":240000}]}).to_string())
    });
    let port = port(ProviderId::Itunes, &server);
    let tagger = crate::Tagger::with_backends(vec![Arc::new(port)]).unwrap();
    let found = tagger
        .match_track(&input(), &config(ProviderId::Itunes))
        .unwrap()
        .matches;
    assert_eq!(found.len(), 1);
    let selected = found[0].track.clone();
    assert_eq!(selected.release_id.as_deref(), Some("2"));
    assert_eq!(selected.disc_number, Some(2));
    assert_eq!(selected.track_number.unwrap().to_string(), "3");
    let requests = server.seen.lock().unwrap();
    assert_eq!(requests.len(), 1, "fixture request sequence: {requests:?}");
}

#[test]
fn cancelled_beatport_auth_start_wait_does_not_poison_another_cache_owner() {
    use onetagger_selected_providers::beatport::{Beatport, BeatportTokenCache};
    let server = Server::new(|_, _| {
        (
            200,
            json!({"access_token":"fixture-anonymous-token","expires_in":3600}).to_string(),
        )
    });
    let cache = Arc::new(BeatportTokenCache::default());
    let cancel = Arc::new(AtomicBool::new(false));
    let first = Beatport::with_endpoints(
        cache.clone(),
        reqwest::blocking::Client::new(),
        &server.url,
        &server.url,
        &server.url,
    )
    .with_cancel(cancel.clone())
    .with_start_gate(Arc::new(move |_| {
        cancel.store(true, Ordering::Relaxed);
        anyhow::bail!("fixture cancelled")
    }));
    assert!(first.update_token().is_err());
    let next = Beatport::with_endpoints(
        cache,
        reqwest::blocking::Client::new(),
        &server.url,
        &server.url,
        &server.url,
    );
    assert!(
        next.update_token().is_ok(),
        "an unrelated worker must not inherit another cancellation"
    );
    let requests = server.seen.lock().unwrap();
    assert_eq!(requests.len(), 1, "fixture request sequence: {requests:?}");
}

#[test]
fn bpm_login_uses_a_stable_uuid_shaped_device_identity() {
    use crate::catalog_runtime::{
        CatalogHttpRequest, CatalogHttpResponse, CatalogIdentity, CatalogRuntime, CatalogTransport,
        MemoryCatalogState,
    };
    use cuerithm_credentials::{
        CredentialService, ProviderId as CredentialProvider, Revocation, Secret,
        state::FileStateStore,
    };
    struct Login(String);
    impl CatalogTransport for Login {
        fn execute(
            &self,
            r: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&Revocation>,
        ) -> Result<CatalogHttpResponse> {
            let body: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
            let id = body["device"]["device_uuid"].as_str().unwrap();
            assert_eq!(id, self.0);
            assert_eq!(id.len(), 32);
            assert!(id.bytes().all(|c| c.is_ascii_hexdigit()));
            Ok(CatalogHttpResponse {
                status: 200,
                headers: vec![],
                body: br#"{"data":{"session_token":"fixture-session"}}"#.to_vec(),
            })
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let service = CredentialService::new(
        crate::catalog_runtime::tests::VaultFixture::default(),
        Arc::new(FileStateStore::open(temp.path()).unwrap()),
    )
    .unwrap();
    let before = service.status(CredentialProvider::BpmSupreme).unwrap();
    let saved = service
        .save(
            &before.reference,
            Secret::structured(
                r#"{"email":"fixture@example.invalid","password":"two words"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
    let expected = saved.reference.generation.simple().to_string();
    let runtime = CatalogRuntime::new(
        CatalogIdentity::new("test").unwrap(),
        Arc::new(MemoryCatalogState::default()),
        Arc::new(service),
    )
    .unwrap()
    .with_transport(Arc::new(Login(expected)));
    super::test_pool_account(
        &runtime,
        &saved.reference,
        &Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
}

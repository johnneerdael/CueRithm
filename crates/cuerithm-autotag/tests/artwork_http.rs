use cuerithm_autotag::{
    AlbumHint, AnalysisTags, AutotagConfig, CatalogEndpoints, CatalogMatch, CatalogTrack,
    MatchStatus, ProviderId, SupportedTag, TagInput, TagMatchOutcome, TagWritePlan, Tagger,
};
use cuerithm_media::MediaRuntime;
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::{Duration, Instant},
};

struct Response {
    path: &'static str,
    referer: Option<&'static str>,
    body: Vec<u8>,
    redirect: bool,
}

fn fixture(
    responses: impl FnOnce(&str) -> Vec<Response>,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let responses = responses(&url);
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut requests = Vec::new();
        for response in responses {
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "Artwork fixture timed out");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("Artwork fixture: {error}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = [0; 8192];
            let count = socket.read(&mut bytes).unwrap();
            let request = String::from_utf8_lossy(&bytes[..count]).into_owned();
            let referer = request.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("referer").then(|| value.trim())
            });
            let allowed = request.starts_with(&format!("GET {} HTTP/1.1", response.path))
                && referer == response.referer;
            requests.push(request);
            let status = if !allowed {
                "403 Forbidden"
            } else if response.redirect {
                "302 Found"
            } else {
                "200 OK"
            };
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nLocation: /cover.png\r\nConnection: close\r\n\r\n",
                        response.body.len()
                    )
                    .as_bytes(),
                )
                .unwrap();
            socket.write_all(&response.body).unwrap();
            if !allowed {
                break;
            }
        }
        requests
    });
    (url, server)
}

#[test]
fn cached_traxsource_album_downloads_artwork_with_referer_and_no_catalog_refetch() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let source = root.join("crates/cuerithm-media/tests/fixtures/metadata.flac");
    let audio = runtime.decode(&source, cache.path()).unwrap();
    let image = std::fs::read(root.join("apps/desktop/src-tauri/icons/32x32.png")).unwrap();
    let (url, server) = fixture(|url| {
        let release = format!(
            r#"<div class="ttl-cont"><h1 class="title">Fixture Release</h1><h1 class="artists"><a>Fixture Artist</a></h1><div class="cat-rdate">FIX001 | 2026-01-02</div><div class="t-image"><img src="{url}/cover"></div><div id="trkListCont"><div class="trklist"><div class="trk-row play-trk"><div class="tnum">1</div><div class="title"><a href="/track/101/signal">Signal</a><span class="duration">5:00</span></div><div class="artists"><a>Fixture Artist</a></div></div></div></div></div>"#
        );
        vec![
            Response {
                path: "/title/456/",
                referer: None,
                body: release.into_bytes(),
                redirect: false,
            },
            Response {
                path: "/cover",
                referer: Some("https://www.traxsource.com/"),
                body: vec![],
                redirect: true,
            },
            Response {
                path: "/cover.png",
                referer: Some("https://www.traxsource.com/"),
                body: image.clone(),
                redirect: false,
            },
        ]
    });
    let tagger = Tagger::with_endpoints(
        CatalogEndpoints::for_local_server(&url, Duration::from_secs(3)).unwrap(),
    )
    .unwrap();
    let mut config = AutotagConfig::default();
    assert_eq!(config.options.platforms, ["beatport", "traxsource"]);
    config.options.album_tagging = true;
    config.options.tags = vec![SupportedTag::AlbumArt];
    config.options.overwrite = true;
    let cancel = Arc::new(AtomicBool::new(false));
    let hint = AlbumHint {
        provider: ProviderId::Traxsource,
        album: tagger
            .get_album(ProviderId::Traxsource, "456", &config, &cancel)
            .unwrap()
            .unwrap(),
    };
    let mut input = TagInput {
        path: source,
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    input.media.tags.insert("title".into(), "Signal".into());
    input
        .media
        .tags
        .insert("artist".into(), "Fixture Artist".into());
    let outcome = tagger
        .match_track_with_album(&input, &config, &cancel, Some(&hint))
        .unwrap();
    assert_eq!(
        outcome.matches[0].reason,
        onetagger_tagger::MatchReason::Album
    );
    assert_eq!(outcome.matches[0].track, hint.album.tracks[0]);
    let staged = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default())
        .unwrap()
        .stage_prepared(&runtime, &audio, cache.path());
    let requests = server.join().unwrap();
    let staged = staged.unwrap();
    assert_eq!(
        requests.len(),
        3,
        "One release lookup and two artwork requests including redirect"
    );
    assert_eq!(runtime.artwork(&staged.path).unwrap().unwrap().bytes, image);
    assert_eq!(
        cuerithm_media::file_sha256(&audio.source_path).unwrap(),
        audio.source_sha256
    );
}

#[test]
fn selected_beatport_artwork_does_not_receive_traxsource_referer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let cache = tempfile::tempdir().unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(cache.path())
    .unwrap();
    let source = cache.path().join("pulses.flac");
    let audio = runtime.decode(&source, cache.path()).unwrap();
    assert!(runtime.artwork(&source).unwrap().is_none());
    let image = std::fs::read(root.join("apps/desktop/src-tauri/icons/32x32.png")).unwrap();
    for overwrite in [false, true] {
        let (url, server) = fixture(|_| {
            vec![Response {
                path: "/cover",
                referer: None,
                body: image.clone(),
                redirect: false,
            }]
        });
        let mut matches = vec![
            CatalogMatch::new(
                1.,
                CatalogTrack {
                    platform: "beatport".into(),
                    art: Some(format!("{url}/cover")),
                    ..Default::default()
                },
            ),
            CatalogMatch::new(
                1.,
                CatalogTrack {
                    platform: "traxsource".into(),
                    art: Some(format!("{url}/wrong-cover")),
                    ..Default::default()
                },
            ),
        ];
        if overwrite {
            matches.reverse();
        }
        let outcome = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches,
            attempted: vec![],
            warnings: vec![],
        };
        let input = TagInput {
            path: source.clone(),
            source_sha256: audio.source_sha256.clone(),
            media: audio.info.clone(),
            duration_seconds: audio.clock.duration_seconds(),
        };
        let mut config = AutotagConfig::default();
        config.options.tags = vec![SupportedTag::AlbumArt];
        config.options.overwrite = overwrite;
        let staged = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default())
            .unwrap()
            .stage_prepared(&runtime, &audio, cache.path());
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(
            runtime
                .artwork(&staged.unwrap().path)
                .unwrap()
                .unwrap()
                .bytes,
            image
        );
    }
}

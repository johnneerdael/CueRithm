use cuerithm_autotag::{AlbumHint, AutotagConfig, CatalogEndpoints, ProviderId, TagInput, Tagger};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Duration,
};

// Invented metadata and markup; never copied catalog responses.
fn row(id: u32, number: u32, title: &str) -> String {
    format!(
        r#"<div class="trk-row play-trk ptk-{id}"><div class="tnum">{number}</div><div class="title"><a href="/track/{id}/signal">{title}</a><span class="version">Extended Mix <span class="duration">5:00</span></span></div><div class="artists"><a>Fixture Artist</a></div><div class="genre"><a>House</a></div></div>"#
    )
}
fn album() -> String {
    format!(
        r#"<div class="ttl-cont"><h1 class="title">Fixture Release</h1><h1 class="artists"><a>Fixture Artist</a></h1><div class="cat-rdate">FIX001 | 2026-01-02</div><a class="com-label">Fixture Label</a><div class="t-image"><img src="https://example.test/art.jpg"></div><div id="trkListCont"><div class="trklist">{}{}</div></div></div><div class="recommended">{}</div>"#,
        row(101, 1, "Signal"),
        row(102, 2, "Afterglow"),
        row(999, 1, "Unrelated")
    )
}
fn fixture(body: String) -> (Tagger, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut bytes = [0; 8192];
        let n = socket.read(&mut bytes).unwrap();
        let request = String::from_utf8_lossy(&bytes[..n])
            .lines()
            .next()
            .unwrap()
            .to_string();
        socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .unwrap();
        request
    });
    let tagger = Tagger::with_endpoints(
        CatalogEndpoints::for_local_server(&url, Duration::from_secs(3)).unwrap(),
    )
    .unwrap();
    (tagger, server)
}
fn config() -> AutotagConfig {
    let mut c = AutotagConfig::default();
    c.options.platforms = vec!["traxsource".into()];
    c
}
#[test]
fn full_release_preserves_order_metadata_and_excludes_recommendations() {
    let (tagger, server) = fixture(album());
    let release = tagger
        .get_album(
            ProviderId::Traxsource,
            "456",
            &config(),
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
        .expect("Traxsource must return its complete release");
    assert!(server.join().unwrap().starts_with("GET /title/456/"));
    assert_eq!(release.id, "456");
    assert_eq!(release.name, "Fixture Release");
    assert_eq!(release.tracks.len(), 2);
    for (i, track) in release.tracks.iter().enumerate() {
        assert_eq!(track.track_id, Some((101 + i).to_string()));
        assert_eq!(track.release_id.as_deref(), Some("456"));
        assert_eq!(
            track.track_number.as_ref().unwrap().to_string(),
            (i + 1).to_string()
        );
        assert_eq!(track.track_total, Some(2));
        assert_eq!(track.album.as_deref(), Some("Fixture Release"));
        assert_eq!(track.version.as_deref(), Some("Extended Mix"));
        assert_eq!(track.catalog_number.as_deref(), Some("FIX001"));
        assert_eq!(track.release_date.unwrap().to_string(), "2026-01-02");
        assert_eq!(track.label.as_deref(), Some("Fixture Label"));
    }
}

#[test]
fn cached_release_matches_each_track_without_reopening_catalog_pages() {
    let (tagger, server) = fixture(album());
    let mut config = config();
    config.options.album_tagging = true;
    config.options.match_duration = true;
    config.options.strictness = 1.;
    config.options.tags = Tagger::provider_options()
        .into_iter()
        .find(|provider| provider.id == "traxsource")
        .unwrap()
        .supported_tags;
    let cancel = Arc::new(AtomicBool::new(false));
    let album = tagger
        .get_album(ProviderId::Traxsource, "456", &config, &cancel)
        .unwrap()
        .unwrap();
    assert!(server.join().unwrap().starts_with("GET /title/456/"));
    // The fixture listener is closed: any redundant track/release request now fails.
    let hint = AlbumHint {
        provider: ProviderId::Traxsource,
        album,
    };
    for track in &hint.album.tracks {
        let input = TagInput {
            path: "fixture.flac".into(),
            source_sha256: "0".repeat(64),
            duration_seconds: track.duration.as_secs_f64(),
            media: cuerithm_media::MediaInfo {
                codec: "flac".into(),
                sample_rate: 44100,
                channels: 2,
                bits_per_sample: Some(16),
                properties: Default::default(),
                tags: [
                    ("title".into(), track.full_title()),
                    ("artist".into(), track.artists[0].clone()),
                ]
                .into(),
            },
        };
        let result = tagger
            .match_track_with_album(&input, &config, &cancel, Some(&hint))
            .unwrap();
        assert_eq!(result.matches.len(), 1);
        assert_eq!(
            result.matches[0].reason,
            onetagger_tagger::MatchReason::Album
        );
        assert_eq!(&result.matches[0].track, track);
        assert_eq!(
            result.matches[0].track.art.as_deref(),
            Some("https://example.test/art.jpg")
        );
        assert!(result.warnings.is_empty());
    }
}
#[test]
fn empty_malformed_and_challenged_releases_are_errors() {
    for body in [
        "<div class='ttl-cont'><h1 class='title'>Empty</h1></div>",
        "<h1>Broken</h1>",
        "<title>Just a moment...</title>",
    ] {
        let (tagger, server) = fixture(body.into());
        assert!(
            tagger
                .get_album(
                    ProviderId::Traxsource,
                    "456",
                    &config(),
                    &Arc::new(AtomicBool::new(false))
                )
                .is_err()
        );
        server.join().unwrap();
    }
}

#[test]
fn release_search_preserves_ids_and_requests_explicit_page() {
    let html = r#"<?xml version="1.0"?><root><data><![CDATA[<div id="searchResCont"><div class="grid-item play-ttl"><div class="grid-image"><img src="https://example.test/thumb.jpg"></div><a class="com-title" href="/title/456/fixture">Fixture Release</a><a class="com-label">Fixture Label</a></div></div>]]></data></root>"#;
    let (tagger, server) = fixture(html.into());
    let releases = tagger
        .search_releases(
            ProviderId::Traxsource,
            "Fixture & Artist",
            2,
            &config(),
            &Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    let request = server.join().unwrap();
    assert!(request.starts_with("GET /scripts/search.php/titles?"));
    assert!(request.contains("term=Fixture+%26+Artist"));
    assert!(request.contains("page=2"));
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].id, "456");
    assert_eq!(releases[0].title, "Fixture Release");
}

#[test]
fn extension_uses_tracks_own_release_and_preserves_original_position() {
    use onetagger_selected_providers::traxsource::Traxsource;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for (route, body) in [("/track/102/afterglow", "<div class='ttl-info ellip'><a href='/title/456/fixture'>Fixture Release</a></div>".to_owned()), ("/title/456/", album())] {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut bytes = [0;8192];
            let n = socket.read(&mut bytes).unwrap();
            assert!(String::from_utf8_lossy(&bytes[..n]).starts_with(&format!("GET {route}")));
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
        }
    });
    let source = Traxsource::with_endpoints(
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap(),
        &url,
    );
    let mut track = cuerithm_autotag::CatalogTrack {
        track_id: Some("102".into()),
        url: format!("{url}/track/102/afterglow"),
        title: "Afterglow".into(),
        ..Default::default()
    };
    source
        .extend_track_traxsource(&mut track, true, false)
        .unwrap();
    server.join().unwrap();
    assert_eq!(track.release_id.as_deref(), Some("456"));
    assert_eq!(track.track_number.unwrap().to_string(), "2");
    assert_eq!(track.track_total, Some(2));
    assert_eq!(track.title, "Afterglow");
}

#[test]
fn release_search_distinguishes_empty_results_from_bad_markup() {
    for (body, empty) in [
        (
            "<div id='searchResCont'><h2>No results found for Fixture</h2></div>",
            true,
        ),
        ("<div id='searchResCont'><h2>Search</h2></div>", false),
        (
            "<div id='searchResCont'><div class='grid-item play-ttl'></div></div>",
            false,
        ),
        ("<root><data>broken", false),
    ] {
        let (tagger, server) = fixture(body.into());
        let result = tagger.search_releases(
            ProviderId::Traxsource,
            "Fixture",
            1,
            &config(),
            &Arc::new(AtomicBool::new(false)),
        );
        server.join().unwrap();
        if empty {
            assert!(result.unwrap().is_empty());
        } else {
            assert!(result.is_err());
        }
    }
}
#[test]
fn cancellation_prevents_release_network_requests() {
    let tagger = Tagger::with_endpoints(
        CatalogEndpoints::for_local_server("http://127.0.0.1:1", Duration::from_millis(20))
            .unwrap(),
    )
    .unwrap();
    let cancel = Arc::new(AtomicBool::new(true));
    for result in [
        tagger
            .get_album(ProviderId::Traxsource, "456", &config(), &cancel)
            .map(|_| ()),
        tagger
            .search_releases(ProviderId::Traxsource, "Fixture", 1, &config(), &cancel)
            .map(|_| ()),
    ] {
        assert!(result.unwrap_err().to_string().contains("stopping"));
    }
}

#[test]
fn release_rejects_partial_tracklist_when_header_lists_more_tracks() {
    let body = album().replace(
        "<h1 class=\"title\">",
        "<a class='com-play-all' data-tracks='101,102,103'></a><h1 class=\"title\">",
    );
    let (tagger, server) = fixture(body);
    let result = tagger.get_album(
        ProviderId::Traxsource,
        "456",
        &config(),
        &Arc::new(AtomicBool::new(false)),
    );
    server.join().unwrap();
    assert!(result.is_err());
}

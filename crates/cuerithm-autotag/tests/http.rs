use cuerithm_autotag::{AutotagConfig, CatalogEndpoints, MatchStatus, TagInput, Tagger};
use cuerithm_media::MediaInfo;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    thread,
    time::Duration,
};
fn input() -> TagInput {
    TagInput {
        path: PathBuf::from("fixture.flac"),
        source_sha256: "1".repeat(64),
        duration_seconds: 300.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: BTreeMap::new(),
            tags: [
                ("title".into(), "Signal".into()),
                ("artist".into(), "Fixture Artist".into()),
            ]
            .into(),
        },
    }
}
fn fixture(status: u16, body: &'static str, delay: Duration) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buffer = [0; 8192];
        let _ = stream.read(&mut buffer);
        thread::sleep(delay);
        let message = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(message.as_bytes());
    });
    (addr, handle)
}
#[test]
fn http_failure_is_not_reported_as_no_match() {
    for provider in ["beatport", "traxsource"] {
        let (url, server) = fixture(403, "blocked", Duration::ZERO);
        let t = Tagger::with_endpoints(
            CatalogEndpoints::for_local_server(&url, Duration::from_secs(2)).unwrap(),
        )
        .unwrap();
        let mut config = AutotagConfig::default();
        config.options.platforms = vec![provider.into()];
        assert!(
            t.match_track(&input(), &config)
                .unwrap_err()
                .to_string()
                .contains("403")
        );
        server.join().unwrap();
    }
}
#[test]
fn malformed_rows_and_empty_results_are_distinct() {
    for (html, no_match) in [
        ("<div id='searchTrackList'></div>", true),
        (
            "<div id='searchTrackList'><div class='trk-row'></div></div>",
            false,
        ),
    ] {
        let (url, server) = fixture(200, html, Duration::ZERO);
        let t = Tagger::with_endpoints(
            CatalogEndpoints::for_local_server(&url, Duration::from_secs(2)).unwrap(),
        )
        .unwrap();
        let mut config = AutotagConfig::default();
        config.options.platforms = vec!["traxsource".into()];
        let result = t.match_track(&input(), &config);
        if no_match {
            assert_eq!(result.unwrap().status, MatchStatus::NoMatch);
        } else {
            assert!(result.is_err());
        }
        server.join().unwrap();
    }
}
#[test]
fn requests_time_out_and_fixture_overrides_cannot_target_other_hosts() {
    assert!(
        CatalogEndpoints::for_local_server("http://example.com", Duration::from_secs(1)).is_err()
    );
    let (url, server) = fixture(200, "", Duration::from_millis(250));
    let t = Tagger::with_endpoints(
        CatalogEndpoints::for_local_server(&url, Duration::from_millis(40)).unwrap(),
    )
    .unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["traxsource".into()];
    assert!(t.match_track(&input(), &config).is_err());
    server.join().unwrap();
}
#[test]
fn beatport_search_uses_catalog_api_instead_of_webpage_markup() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for (method, path, body) in [
            (
                "POST",
                "/o/token/",
                r#"{"access_token":"fixture-token","expires_in":3600}"#,
            ),
            ("GET", "/v4/catalog/search/", r#"{"tracks":[]}"#),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0; 8192];
            let n = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..n]);
            let first = request.lines().next().unwrap();
            assert!(
                first.starts_with(&format!("{method} {path}")),
                "unexpected request route"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    let tagger = Tagger::with_endpoints(
        CatalogEndpoints::for_local_server(&url, Duration::from_secs(2)).unwrap(),
    )
    .unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    assert_eq!(
        tagger.match_track(&input(), &config).unwrap().status,
        MatchStatus::NoMatch
    );
    server.join().unwrap();
}
#[test]
fn traxsource_valid_search_and_detail_populate_metadata() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let search = r#"<div id="searchTrackList"><div class="trk-row"><div class="title"><a href="/track/123/signal">Signal</a><span>5:00</span></div><div class="artists"><a>Fixture Artist</a></div><div class="label">Fixture Label</div><div class="key-bpm">Cmin<span>120</span></div><div class="genre">House</div><div class="r-date">2020-01-01</div></div></div>"#;
    let detail =
        r#"<div class="ttl-info ellip"><a href="/title/456/release">Fixture Album</a></div>"#;
    let server = thread::spawn(move || {
        for (path, body) in [("/search/tracks?", search), ("/track/123/signal", detail)] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0; 8192];
            let n = stream.read(&mut buffer).unwrap();
            assert!(String::from_utf8_lossy(&buffer[..n]).starts_with(&format!("GET {path}")));
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .unwrap();
        }
    });
    let tagger = Tagger::with_endpoints(
        CatalogEndpoints::for_local_server(&url, Duration::from_secs(2)).unwrap(),
    )
    .unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["traxsource".into()];
    let result = tagger.match_track(&input(), &config).unwrap();
    assert_eq!(
        result.matches[0].track.label.as_deref(),
        Some("Fixture Label")
    );
    assert_eq!(
        result.matches[0].track.album.as_deref(),
        Some("Fixture Album")
    );
    assert_eq!(result.matches[0].track.bpm, Some(120));
    assert_eq!(result.matches[0].track.genres, vec!["House"]);
    server.join().unwrap();
}
#[test]
fn expired_token_is_refreshed_without_reacquiring_a_held_lock() {
    use onetagger_selected_providers::beatport::{Beatport, BeatportOAuth};
    use std::sync::{Arc, Mutex};
    let (url, server) = fixture(
        200,
        r#"{"access_token":"fresh-fixture-token","expires_in":3600}"#,
        Duration::ZERO,
    );
    let token = Arc::new(Mutex::new(Some(BeatportOAuth {
        access_token: "expired-fixture-token".into(),
        expires_in: 0,
    })));
    let source = Beatport::with_endpoints(
        token,
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        &url,
        &url,
        &url,
    );
    assert_eq!(source.update_token().unwrap(), "fresh-fixture-token");
    server.join().unwrap();
}

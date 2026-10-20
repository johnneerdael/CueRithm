#![cfg(feature = "autotag")]
use cuerithm_autotag::ChallengeStore;
use cuerithm_library::Library;

#[test]
fn detected_challenge_is_persisted_by_the_real_tagger_and_prevents_direct_access_after_reopen() {
    use cuerithm_autotag::{AutotagConfig, CatalogEndpoints, TagInput, Tagger};
    use cuerithm_media::MediaInfo;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        time::Duration,
    };
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.sqlite");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; 8192];
        assert!(socket.read(&mut request).unwrap() > 0);
        socket.write_all(b"HTTP/1.1 403 Forbidden\r\ncf-mitigated: challenge\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let input = TagInput {
        path: "public-fixture.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 300.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("artist".into(), "Fixture Artist".into()),
                ("title".into(), "Fixture Track".into()),
            ]
            .into(),
        },
    };
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["traxsource".into()];
    for attempt in 0..2 {
        let library = Library::open(&path).unwrap();
        let tagger = Tagger::with_endpoints_and_store(
            CatalogEndpoints::for_local_server(&url, Duration::from_secs(1)).unwrap(),
            Arc::new(library.clone()),
        )
        .unwrap();
        let error = tagger.match_track(&input, &config).unwrap_err();
        assert!(
            error.to_string().contains("solver"),
            "attempt {attempt}: {error:#}"
        );
        assert!(library.requires_solver("traxsource").unwrap());
    }
    server.join().unwrap();
}

#[test]
fn detected_source_requirement_survives_reopen_and_can_be_reset() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("library.sqlite");
    {
        let lib = Library::open(&path).unwrap();
        assert!(!lib.requires_solver("traxsource").unwrap());
        lib.mark_requires_solver("traxsource").unwrap();
        lib.mark_requires_solver("traxsource").unwrap();
    }
    let lib = Library::open(&path).unwrap();
    assert!(lib.requires_solver("traxsource").unwrap());
    assert!(!lib.requires_solver("beatport").unwrap());
    let catalog = lib.tagging_catalog().unwrap();
    assert!(
        catalog
            .access
            .iter()
            .any(|a| a.id == "traxsource" && a.requires_solver)
    );
    lib.reset_catalog_access().unwrap();
    assert!(!lib.requires_solver("traxsource").unwrap());
}

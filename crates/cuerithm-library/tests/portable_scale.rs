//! Authored metadata only. Run explicitly; no user music or private backup input.
use cuerithm_library::{backup::*, *};
use std::collections::BTreeMap;

#[test]
#[ignore = "100,000-track acceptance workload; run explicitly and measure the test process RSS"]
fn hundred_thousand_tracks_and_ten_thousand_playlists_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("source/library.sqlite");
    let library = Library::open(&db).unwrap();
    let started = std::time::Instant::now();
    for offset in (0..100_000).step_by(250) {
        let batch: Vec<_> = (offset..offset + 250)
            .map(|i| {
                let id = format!("scale-track-{i:06}");
                PortableTrack {
                    id: id.clone(),
                    locator: PortableLocator {
                        id: format!("scale-locator-{i:06}"),
                        flavor: PathFlavor::WindowsDrive,
                        original: EncodedPath::utf8(format!(
                            "D:/Music/Album{:04}/Track{i:06}.flac",
                            i / 10
                        )),
                        source_root: Some(EncodedPath::utf8("D:/Music")),
                        components: Some(vec![
                            EncodedPath::utf8(format!("Album{:04}", i / 10)),
                            EncodedPath::utf8(format!("Track{i:06}.flac")),
                        ]),
                    },
                    source_sha256: None,
                    title: format!("Authored track {i}"),
                    artist: "Scale fixture".into(),
                    album: format!("Album {}", i / 10),
                    genre: "House".into(),
                    metadata: BTreeMap::new(),
                    key: Some("8A".into()),
                    bpm: Some(128.),
                    energy: Some(7),
                    duration: Some(300.),
                    archived: i % 100 == 0,
                    origin: "cuerithm".into(),
                    analysis_status: AnalysisStatus::NotAnalyzed,
                    musical: Some(AcceptedMusicalData {
                        id: format!("scale-musical-{i:06}"),
                        source: "cuerithm".into(),
                        cues: (0..8)
                            .map(|cue| AcceptedCue {
                                id: format!("{id}-cue-{cue}"),
                                kind: CueKind::Cue,
                                start_seconds: cue as f64 * 32.,
                                end_seconds: None,
                                slot: Some(cue),
                                memory: false,
                                name: Some(format!("Cue {}", cue + 1)),
                                color: Some("blue".into()),
                                active_loop: false,
                                source_data: BTreeMap::new(),
                            })
                            .collect(),
                        tempo_markers: vec![],
                    }),
                }
            })
            .collect();
        library.import_portable_tracks(&batch).unwrap();
    }
    let mut connection = rusqlite::Connection::open(&db).unwrap();
    let transaction = connection.transaction().unwrap();
    for i in 0..10_000 {
        transaction.execute("INSERT INTO playlists(id,kind,name,position,last_edit_id) VALUES(?1,'playlist',?2,?3,'authored-scale-fixture')",rusqlite::params![format!("scale-playlist-{i:05}"),format!("Authored playlist {i}"),i]).unwrap();
    }
    for i in 0..100_000 {
        transaction
            .execute(
                "INSERT INTO playlist_members VALUES(?1,?2,?3)",
                rusqlite::params![
                    format!("scale-playlist-{:05}", i / 10),
                    format!("scale-track-{i:06}"),
                    i % 10
                ],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    drop(connection);
    println!("fixture_seconds={:.3}", started.elapsed().as_secs_f64());
    let limits = ArchiveLimits::default();
    let metrics =
        std::cell::RefCell::new((std::time::Instant::now(), std::time::Duration::ZERO, 0u64));
    let control = |_phase: &str, _count: u64| -> anyhow::Result<()> {
        let mut m = metrics.borrow_mut();
        let gap = m.0.elapsed();
        if gap.as_secs_f64() > 0.5 {
            println!(
                "control_gap_seconds={:.3} next_phase={} count={}",
                gap.as_secs_f64(),
                _phase,
                _count
            );
        }
        m.1 = m.1.max(gap);
        m.0 = std::time::Instant::now();
        m.2 += 1;
        Ok(())
    };
    let snapshot = library
        .portable_snapshot_spooled(&limits, &control)
        .unwrap();
    assert_eq!(snapshot.count("tracks").unwrap(), 100_000);
    assert_eq!(snapshot.count("playlists").unwrap(), 10_000);
    let output = temp.path().join("large.cuerithm");
    write_portable_spooled(&output, &snapshot, &limits, &control).unwrap();
    drop(snapshot);
    let loaded = read_portable_spooled(&output, &limits, &control).unwrap();
    assert_eq!(loaded.count("tracks").unwrap(), 100_000);
    assert_eq!(loaded.count("playlist_members").unwrap(), 100_000);
    let destination = Library::open(&temp.path().join("restored/library.sqlite")).unwrap();
    destination
        .restore_spooled(&loaded, &limits, &control)
        .unwrap();
    assert_eq!(destination.counts().unwrap().tracks, 99_000);
    assert_eq!(destination.counts().unwrap().archive, 1_000);
    assert_eq!(
        destination
            .accepted_musical_data("scale-track-099999")
            .unwrap()
            .unwrap()
            .cues
            .len(),
        8
    );
    assert!(destination.jobs().unwrap().is_empty());
    assert!(
        destination
            .local_track_binding("scale-track-099999")
            .unwrap()
            .is_none()
    );
    println!(
        "archive_bytes={} total_seconds={:.3} read_control_events={} max_read_control_gap_seconds={:.3}",
        std::fs::metadata(output).unwrap().len(),
        started.elapsed().as_secs_f64(),
        metrics.borrow().2,
        metrics.borrow().1.as_secs_f64()
    );
}

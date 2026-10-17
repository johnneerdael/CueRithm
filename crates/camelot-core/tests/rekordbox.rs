use camelot_core::{
    Cue, Mode, MusicalKey,
    rekordbox::{self, ExportTrack, VerifiedBeatGrid},
};
use std::path::Path;

fn track<'a>(path: &'a Path, cues: &'a [Cue]) -> ExportTrack<'a> {
    ExportTrack {
        id: 7,
        path,
        title: "Öcean & <Light> \"live\"\npart 2",
        artist: "O'Connor",
        duration_seconds: 123.75,
        key: Some(MusicalKey::new(5, Mode::Minor).unwrap()),
        bpm: Some(124.99996948242188),
        cues,
        verified_grid: None,
    }
}

#[test]
fn exports_parseable_playlist_with_precise_hotcues_and_local_file_uri() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Öcean #1 & 100%.flac");
    let cues = [
        Cue {
            time_seconds: 30.920190775915323,
            name: "Energy 6 & <intro>".into(),
        },
        Cue {
            time_seconds: -0.006639,
            name: "Start".into(),
        },
    ];
    let exported = rekordbox::render(&[track(&path, &cues)], "Practice & mix").unwrap();
    let doc = roxmltree::Document::parse(&exported.xml).unwrap();
    let collection = doc
        .descendants()
        .find(|n| n.has_tag_name("COLLECTION"))
        .unwrap();
    assert_eq!(collection.attribute("Entries"), Some("1"));
    let t = collection
        .children()
        .find(|n| n.has_tag_name("TRACK"))
        .unwrap();
    assert_eq!(
        t.attribute("Name"),
        Some("Öcean & <Light> \"live\"\npart 2")
    );
    assert_eq!(t.attribute("Artist"), Some("O'Connor"));
    assert_eq!(t.attribute("Tonality"), Some("4A"));
    assert_eq!(t.attribute("TotalTime"), Some("123"));
    let location = t.attribute("Location").unwrap();
    assert!(location.starts_with("file://localhost/"));
    assert!(location.contains("%C3%96cean%20%231%20&%20100%25.flac"));
    assert_eq!(
        url::Url::parse(location).unwrap().to_file_path().unwrap(),
        path
    );
    let marks: Vec<_> = t
        .children()
        .filter(|n| n.has_tag_name("POSITION_MARK"))
        .collect();
    assert_eq!(marks.len(), 2);
    assert_eq!(marks[0].attribute("Start"), Some("0"));
    assert_eq!(marks[0].attribute("Num"), Some("0"));
    assert_eq!(marks[1].attribute("Type"), Some("0"));
    assert_eq!(marks[1].attribute("Num"), Some("1"));
    assert_eq!(marks[1].attribute("Name"), Some("Energy 6 & <intro>"));
    assert_eq!(
        marks[1].attribute("Start").unwrap().parse::<f64>().unwrap(),
        cues[0].time_seconds
    );
    assert_eq!(exported.clamped_negative_cues, 1);
    assert_eq!(cues[1].time_seconds, -0.006639);
    assert!(!doc.descendants().any(|n| n.has_tag_name("TEMPO")));
    let playlist = doc
        .descendants()
        .find(|n| n.has_tag_name("NODE") && n.attribute("Type") == Some("1"))
        .unwrap();
    assert_eq!(playlist.attribute("Name"), Some("Practice & mix"));
    assert_eq!(playlist.attribute("Entries"), Some("1"));
    assert_eq!(
        playlist
            .children()
            .find(|n| n.has_tag_name("TRACK"))
            .unwrap()
            .attribute("Key"),
        Some("7")
    );
}

#[test]
fn emits_a_beatgrid_only_when_caller_supplies_verified_meter_and_phase() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("track.wav");
    let mut t = track(&path, &[]);
    t.verified_grid = Some(VerifiedBeatGrid {
        start_seconds: 0.2001832759134947,
        bpm: 125.0,
    });
    let exported = rekordbox::render(&[t], "Grid").unwrap();
    let doc = roxmltree::Document::parse(&exported.xml).unwrap();
    let grid = doc.descendants().find(|n| n.has_tag_name("TEMPO")).unwrap();
    assert_eq!(grid.attribute("Inizio"), Some("0.2001832759134947"));
    assert_eq!(grid.attribute("Bpm"), Some("125"));
    assert_eq!(grid.attribute("Metro"), Some("4/4"));
    assert_eq!(grid.attribute("Battito"), Some("1"));
}

#[test]
fn rejects_invalid_times_numbers_xml_characters_and_more_than_eight_slots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("track.flac");
    for time in [f64::NAN, f64::INFINITY, -0.011, 123.75, 124.0] {
        let cues = [Cue {
            time_seconds: time,
            name: "cue".into(),
        }];
        assert!(rekordbox::render(&[track(&path, &cues)], "P").is_err());
    }
    let cues = vec![
        Cue {
            time_seconds: 1.0,
            name: "cue".into()
        };
        9
    ];
    assert!(rekordbox::render(&[track(&path, &cues)], "P").is_err());
    let mut t = track(&path, &[]);
    t.duration_seconds = f64::NAN;
    assert!(rekordbox::render(&[t], "P").is_err());
    let mut t = track(&path, &[]);
    t.bpm = Some(f64::INFINITY);
    assert!(rekordbox::render(&[t], "P").is_err());
    assert!(rekordbox::render(&[track(&path, &[])], "bad\u{0}name").is_err());
    let mut t = track(&path, &[]);
    t.verified_grid = Some(VerifiedBeatGrid {
        start_seconds: -1.0,
        bpm: 125.0,
    });
    assert!(rekordbox::render(&[t], "P").is_err());
}

#[test]
fn rejects_relative_paths_duplicate_ids_and_duplicate_locations() {
    assert!(rekordbox::render(&[track(Path::new("relative.wav"), &[])], "P").is_err());
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    assert!(rekordbox::render(&[track(&a, &[]), track(&b, &[])], "P").is_err());
    let mut t = track(&a, &[]);
    t.id = 8;
    assert!(rekordbox::render(&[track(&a, &[]), t], "P").is_err());
}

#[test]
fn unknown_estimates_stay_absent_and_eight_cues_keep_separate_slots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("track.flac");
    let cues: Vec<_> = (0..8)
        .map(|i| Cue {
            time_seconds: i as f64,
            name: format!("Cue {i}"),
        })
        .collect();
    let mut t = track(&path, &cues);
    t.bpm = None;
    t.key = None;
    let exported = rekordbox::render(&[t], "P").unwrap();
    let doc = roxmltree::Document::parse(&exported.xml).unwrap();
    let track = doc
        .descendants()
        .find(|n| n.has_tag_name("TRACK") && n.attribute("TrackID").is_some())
        .unwrap();
    assert_eq!(track.attribute("Tonality"), None);
    assert_eq!(track.attribute("AverageBpm"), None);
    assert_eq!(
        track
            .children()
            .filter(|n| n.has_tag_name("POSITION_MARK"))
            .map(|n| n.attribute("Num").unwrap())
            .collect::<Vec<_>>(),
        ["0", "1", "2", "3", "4", "5", "6", "7"]
    );
}

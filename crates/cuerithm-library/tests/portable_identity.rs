use cuerithm_library::*;
use std::collections::BTreeMap;
fn imported() -> PortableTrack {
    PortableTrack {
        id: "track-one".into(),
        locator: PortableLocator {
            id: "locator-one".into(),
            flavor: PathFlavor::WindowsDrive,
            original: EncodedPath::utf8("D:\\Music\\track.wav"),
            source_root: Some(EncodedPath::utf8("D:\\Music")),
            components: Some(vec![EncodedPath::utf8("track.wav")]),
        },
        source_sha256: None,
        title: "Imported".into(),
        artist: "Artist".into(),
        album: String::new(),
        genre: String::new(),
        metadata: BTreeMap::new(),
        key: Some("8A".into()),
        bpm: Some(128.),
        energy: Some(7),
        duration: Some(200.),
        archived: false,
        origin: "lexicon".into(),
        analysis_status: AnalysisStatus::Imported,
        musical: Some(AcceptedMusicalData {
            id: "revision-one".into(),
            source: "lexicon".into(),
            cues: vec![AcceptedCue {
                id: "cue-one".into(),
                kind: CueKind::Cue,
                start_seconds: -0.25,
                end_seconds: None,
                slot: Some(0),
                memory: false,
                name: Some("Lead in".into()),
                color: None,
                active_loop: false,
                source_data: BTreeMap::new(),
            }],
            tempo_markers: vec![
                TempoMarker {
                    id: "tempo-one".into(),
                    start_seconds: -0.25,
                    bpm: 128.,
                    beat_number: None,
                    beats_per_bar: Some(4),
                    source_data: BTreeMap::new(),
                },
                TempoMarker {
                    id: "tempo-two".into(),
                    start_seconds: 64.,
                    bpm: 130.,
                    beat_number: Some(1),
                    beats_per_bar: Some(4),
                    source_data: BTreeMap::new(),
                },
            ],
        }),
    }
}
#[cfg(feature = "autotag")]
#[test]
fn unbound_file_actions_explain_relocation_without_mutating_imported_data() {
    let temp = tempfile::tempdir().unwrap();
    let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
    let track = imported();
    library
        .import_portable_tracks(std::slice::from_ref(&track))
        .unwrap();
    let runtime = cuerithm_media::MediaRuntime::from_bundle(
        &std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tools/media-runtime/bin")
            }),
    )
    .unwrap();
    let editor = MetadataEditService::new(library.clone(), runtime.clone());
    assert!(library.track_file_missing(&track.id).unwrap());
    let preparation = PreparationService::start(library.clone(), runtime).unwrap();
    let error = preparation.reprepare(track.id.clone()).unwrap_err();
    preparation.shutdown();
    assert!(error.to_string().contains("relocate it first"), "{error:#}");
    for error in [
        editor
            .snapshot(std::slice::from_ref(&track.id))
            .unwrap_err(),
        library.track_match_review(&track.id).unwrap_err(),
        library.manual_publication_snapshot(&track.id).unwrap_err(),
    ] {
        assert!(
            error
                .to_string()
                .contains("Local file is not linked; relocate"),
            "{error:#}"
        );
    }
    assert!(library.jobs().unwrap().is_empty());
    assert_eq!(library.portable_tracks().unwrap()[0].locator, track.locator);
    assert_eq!(
        library
            .accepted_musical_data(&track.id)
            .unwrap()
            .unwrap()
            .cues[0]
            .start_seconds,
        -0.25
    );
    let audio = temp.path().join("bound.wav");
    std::fs::write(&audio, b"authored source").unwrap();
    library
        .bind_portable_track(&track.id, &audio, None)
        .unwrap();
    assert!(!library.track_file_missing(&track.id).unwrap());
    for error in [
        editor
            .snapshot(std::slice::from_ref(&track.id))
            .unwrap_err(),
        library.track_match_review(&track.id).unwrap_err(),
        library.manual_publication_snapshot(&track.id).unwrap_err(),
    ] {
        assert!(
            error
                .to_string()
                .contains("File identity is unknown; analyze"),
            "{error:#}"
        );
    }
    std::fs::remove_file(audio).unwrap();
    assert!(library.track_file_missing(&track.id).unwrap());
}
#[cfg(unix)]
#[test]
fn restore_binding_rejects_a_parent_swapped_after_root_review() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let library = Library::open(&base.join("library.sqlite")).unwrap();
    let track = imported();
    library
        .import_portable_tracks(std::slice::from_ref(&track))
        .unwrap();
    let root = base.join("music");
    let outside = base.join("outside");
    std::fs::create_dir_all(root.join("Album")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let reviewed = root.join("Album/track.wav");
    std::fs::write(&reviewed, b"authored fixture").unwrap();
    std::fs::write(outside.join("track.wav"), b"authored fixture").unwrap();
    let hash = cuerithm_media::file_sha256(&reviewed).unwrap();
    std::fs::rename(root.join("Album"), root.join("saved")).unwrap();
    symlink(&outside, root.join("Album")).unwrap();
    assert!(
        library
            .bind_portable_track_in_root(&track.id, &reviewed, &root, &hash, &Default::default())
            .is_err()
    );
    assert!(
        library
            .bind_portable_track_in_root(
                &track.id,
                &outside.join("track.wav"),
                &root,
                &hash,
                &Default::default()
            )
            .is_err()
    );
    assert!(library.local_track_binding(&track.id).unwrap().is_none());
}
#[test]
fn posix_filename_atoms_remain_unresolved_when_windows_cannot_represent_them() {
    for atom in [
        "A\\B.wav",
        "A:B.wav",
        "CON.flac",
        "LPT9.wav",
        "trailing.wav.",
        "trailing ",
        "../escape",
        "has?mark.mp3",
    ] {
        assert!(
            !EncodedPath::utf8(atom)
                .windows_filename_compatible()
                .unwrap(),
            "{atom}"
        );
    }
    for atom in ["normal.wav", "A_B.flac", "雪.flac", "COM0.wav", "LPT10.wav"] {
        assert!(
            EncodedPath::utf8(atom)
                .windows_filename_compatible()
                .unwrap(),
            "{atom}"
        );
    }
}
#[test]
fn unresolved_ready_entries_keep_identity_and_musical_data_without_jobs() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
    let track = imported();
    lib.import_portable_tracks(std::slice::from_ref(&track))
        .unwrap();
    let page = lib.tracks(TrackQuery::default()).unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].bpm, Some(128.));
    assert_eq!(lib.counts().unwrap().incoming, 0);
    assert!(lib.track_path(&track.id).is_err());
    assert!(lib.source_hash(&track.id).is_err());
    assert!(lib.analysis(&track.id).unwrap().is_none());
    CurationService::new(lib.clone(), None)
        .snapshot(std::slice::from_ref(&track.id))
        .unwrap();
    let m = lib.accepted_musical_data(&track.id).unwrap().unwrap();
    assert_eq!(m.cues[0].start_seconds, -0.25);
    assert_eq!(m.tempo_at(70.).unwrap().bpm, 130.);
    assert_eq!(m.tempo_at(0.).unwrap().beat_number, None);
    assert_eq!(lib.portable_tracks().unwrap()[0].locator, track.locator);
    let conn = rusqlite::Connection::open(d.path().join("library.sqlite")).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
}
#[test]
fn malformed_or_conflicting_source_records_roll_back_the_batch() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let good = imported();
    let mut bad = good.clone();
    bad.id = "second".into();
    bad.locator.components = Some(vec![EncodedPath::utf8("..")]);
    assert!(lib.import_portable_tracks(&[good, bad]).is_err());
    assert_eq!(lib.counts().unwrap().tracks, 0);
}
#[test]
fn opaque_components_validate_decoded_separators_and_preserve_surrogates() {
    let mut locator = imported().locator;
    locator.components = Some(vec![EncodedPath {
        encoding: PathEncoding::Base64Utf16le,
        data: "ANg=".into(),
    }]);
    locator.validate().unwrap();
    let json = serde_json::to_string(&locator).unwrap();
    assert_eq!(
        serde_json::from_str::<PortableLocator>(&json).unwrap(),
        locator
    );
    locator.components = Some(vec![EncodedPath {
        encoding: PathEncoding::Base64Utf16le,
        data: "LwA=".into(),
    }]);
    assert!(locator.validate().is_err());
}
#[test]
fn binding_and_watcher_preserve_ready_import_without_preparation_identity() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
    let track = imported();
    lib.import_portable_tracks(std::slice::from_ref(&track))
        .unwrap();
    let path = d.path().join("track.wav");
    std::fs::write(&path, b"bound synthetic source").unwrap();
    lib.bind_portable_track(&track.id, &path, None).unwrap();
    assert_eq!(
        lib.track_path(&track.id).unwrap(),
        path.canonicalize().unwrap()
    );
    assert_eq!(lib.portable_tracks().unwrap()[0].locator, track.locator);
    assert!(
        lib.enqueue_discovered(
            std::slice::from_ref(&path),
            lib.settings().unwrap().revision
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(lib.counts().unwrap().incoming, 0);
    assert!(lib.analysis(&track.id).unwrap().is_none());
    assert!(lib.accepted_musical_data(&track.id).unwrap().is_some());
}
#[test]
fn taxonomy_ids_allow_same_display_names_without_collapsing_entities() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
    let conn = rusqlite::Connection::open(d.path().join("library.sqlite")).unwrap();
    conn.execute_batch("INSERT INTO custom_tag_categories(id,name) VALUES('group-a','Mood'),('group-b','Mood');INSERT INTO custom_tags(id,category_id,name) VALUES('a','group-a','Warm'),('b','group-a','Warm');").unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM custom_tags", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    drop(lib);
}
#[test]
fn conflicting_tempo_anchors_require_review_and_invalid_loops_stay_inert() {
    let mut musical = imported().musical.unwrap();
    musical.tempo_markers[1].start_seconds = musical.tempo_markers[0].start_seconds;
    assert!(musical.validate().is_err());
    musical.tempo_markers.pop();
    musical.cues[0].kind = CueKind::Loop;
    musical.cues[0].end_seconds = Some(-1.);
    musical.validate().unwrap();
}
#[test]
fn unresolved_imported_scalars_and_markers_participate_in_smart_rules() {
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    lib.import_portable_tracks(&[imported()]).unwrap();
    for (field, operator, value) in [
        ("bpm", Operator::NumberEquals, RuleValue::Number(128.)),
        ("energy", Operator::NumberEquals, RuleValue::Number(7.)),
        (
            "cues",
            Operator::CuepointAnyLabelIncludes,
            RuleValue::Text("Lead".into()),
        ),
    ] {
        let preview = lib
            .preview_smart_list(
                SmartDefinition {
                    match_all: true,
                    rules: vec![SmartRule {
                        id: "rule".into(),
                        field: field.into(),
                        operator,
                        value,
                        or: false,
                    }],
                },
                SmartPreviewScope::default(),
                1_700_000_000_000,
            )
            .unwrap();
        assert_eq!(preview.track_ids, vec!["track-one"], "{field}");
    }
}
#[test]
fn migration_failure_rolls_back_nullable_schema_and_all_new_tables() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("library.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    let mut migrations =
        std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| {
                p.file_name().unwrap().to_string_lossy()[..3]
                    .parse::<u32>()
                    .unwrap()
                    <= 28
            })
            .collect::<Vec<_>>();
    migrations.sort();
    for migration in migrations {
        conn.execute_batch(&std::fs::read_to_string(migration).unwrap())
            .unwrap();
    }
    conn.execute("INSERT INTO tracks(id,path,title,source_hash,created,updated) VALUES('old',?1,'Original',?2,1,1)",rusqlite::params![b"unix:/music/original.wav".as_slice(),"a".repeat(64)]).unwrap();
    conn.execute_batch("CREATE TRIGGER injected_portable_failure BEFORE UPDATE OF source_locator_json ON tracks BEGIN SELECT RAISE(ABORT,'injected portable migration failure');END;").unwrap();
    drop(conn);
    assert!(Library::open(&path).is_err());
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        28
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('tracks') WHERE name='path' AND \"notnull\"=1",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name='accepted_musical_revisions'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn existing_native_blobs_produce_portable_roots_without_losing_path_units() {
    let posix =
        PortableLocator::from_native_path_bytes("posix", b"unix:/Music/odd\\name\xff.wav").unwrap();
    assert_eq!(posix.flavor, PathFlavor::OpaquePosix);
    assert_eq!(
        posix.components.as_ref().unwrap()[0].bytes().unwrap(),
        b"odd\\name\xff.wav"
    );
    let mut windows = b"windows:".to_vec();
    for u in "C:\\Music\\track.wav".encode_utf16() {
        windows.extend(u.to_le_bytes());
    }
    let locator = PortableLocator::from_native_path_bytes("windows", &windows).unwrap();
    assert_eq!(locator.flavor, PathFlavor::WindowsDrive);
    assert!(locator.source_root.is_some());
    assert_eq!(locator.components.unwrap().len(), 1);
}
#[test]
fn stale_measured_data_is_hidden_but_saved_publication_remains_inspectable() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
    let mut track = imported();
    track.musical = None;
    lib.import_portable_tracks(&[track]).unwrap();
    let publication = serde_json::json!({"grid":{"bpm":128.0,"startTime":0.25},"cues":[{"name":"Saved Drop","startTime":32.25,"type":"1","position":0,"color":"red"}],"implementation_sha256":"saved","inputs_sha256":"saved"});
    let conn = rusqlite::Connection::open(d.path().join("library.sqlite")).unwrap();
    conn.execute(
        "UPDATE tracks SET analysis_json=?1,analysis_status='stale' WHERE id='track-one'",
        [serde_json::json!({"publication":publication}).to_string()],
    )
    .unwrap();
    assert!(lib.analysis("track-one").unwrap().is_none());
    let saved = lib.accepted_musical_data("track-one").unwrap().unwrap();
    assert_eq!(saved.cues[0].start_seconds, 32.25);
    assert_eq!(saved.tempo_markers[0].start_seconds, 0.25);
    assert_eq!(saved.tempo_markers[0].beat_number, None);
    assert!(lib.dj_sync_library(DjSyncScope::Full).is_err());
    assert!(
        conn.query_row(
            "SELECT analysis_json IS NOT NULL FROM tracks WHERE id='track-one'",
            [],
            |r| r.get::<_, bool>(0)
        )
        .unwrap()
    );
}
#[test]
fn exclusive_library_operation_rejects_clone_writers_and_releases_on_error() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
    let path = d.path().join("source.wav");
    std::fs::write(&path, b"synthetic").unwrap();
    let revision = lib.settings().unwrap().revision;
    let _failure = (|| -> anyhow::Result<()> {
        let _lease = lib.exclusive_operation()?;
        assert_eq!(lib.counts()?.tracks, 0);
        lib.import_portable_tracks(&[imported()])?;
        let clone = lib.clone();
        let p = path.clone();
        let r = revision.clone();
        assert!(
            std::thread::spawn(move || clone.enqueue(&[p], r))
                .join()
                .unwrap()
                .is_err()
        );
        anyhow::bail!("injected operation failure")
    })();
    let clone = lib.clone();
    assert!(
        std::thread::spawn(move || clone.enqueue(&[path], revision))
            .join()
            .unwrap()
            .is_ok()
    );
    assert!(lib.exclusive_operation().err().unwrap().is::<LibraryBusy>());
    assert!(lib.claim_next().unwrap().is_some());
    assert!(lib.exclusive_operation().err().unwrap().is::<LibraryBusy>());
}
#[test]
fn watcher_resumes_after_exclusive_operation_busy_period() {
    use std::time::{Duration, Instant};
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
    let runtime = cuerithm_media::MediaRuntime::from_bundle(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin"),
    )
    .unwrap();
    let service = PreparationService::start_with_workers(lib.clone(), runtime, 1).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let gate = loop {
        match lib.exclusive_operation() {
            Ok(g) => break g,
            Err(e) if e.is::<LibraryBusy>() && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => panic!("{e}"),
        }
    };
    service.wake();
    std::thread::sleep(Duration::from_millis(450));
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![d.path().join("offline-folder")];
    lib.save_settings(settings).unwrap();
    drop(gate);
    service.wake();
    let deadline = Instant::now() + Duration::from_secs(3);
    while service.status().watch_errors.is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        service
            .status()
            .watch_errors
            .iter()
            .any(|s| s.contains("offline-folder")),
        "Watcher must remain alive and resume reading settings after the operation"
    );
    assert!(service.status().last_error.is_none());
    service.shutdown();
}

fn windows_native_blob(units: &[u16]) -> Vec<u8> {
    let mut blob = b"windows:".to_vec();
    blob.extend(units.iter().flat_map(|u| u.to_le_bytes()));
    blob
}
fn decoded_utf16(path: &EncodedPath) -> Vec<u16> {
    path.bytes()
        .unwrap()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect()
}
#[test]
fn canonical_windows_verbatim_paths_keep_original_and_export_safe_logical_roots() {
    for (native, expected_flavor, expected_root) in [
        (
            r"\\?\C:\Music\track.wav",
            PathFlavor::WindowsDrive,
            "C:/Music",
        ),
        (
            r"\\?\UNC\server\share\Music\track.wav",
            PathFlavor::WindowsUnc,
            "//server/share/Music",
        ),
    ] {
        let units = native.encode_utf16().collect::<Vec<_>>();
        let locator =
            PortableLocator::from_native_path_bytes("canonical", &windows_native_blob(&units))
                .unwrap();
        assert_eq!(locator.flavor, expected_flavor);
        assert_eq!(decoded_utf16(&locator.original), units);
        assert_eq!(
            String::from_utf16(&decoded_utf16(locator.source_root.as_ref().unwrap())).unwrap(),
            expected_root
        );
        assert_eq!(
            String::from_utf16(&decoded_utf16(&locator.components.as_ref().unwrap()[0])).unwrap(),
            "track.wav"
        );
        let d = tempfile::tempdir().unwrap();
        let lib = Library::open(&d.path().join("source.sqlite")).unwrap();
        let mut track = imported();
        track.locator = locator.clone();
        lib.import_portable_tracks(&[track]).unwrap();
        let archive = lib.portable_snapshot().unwrap();
        assert_eq!(archive.records["roots"].len(), 1);
        let encoded: EncodedPath =
            serde_json::from_value(archive.records["roots"][0].value["source_root"].clone())
                .unwrap();
        assert_eq!(
            String::from_utf16(&decoded_utf16(&encoded)).unwrap(),
            expected_root
        );
        let roots = lib.relocation_roots().unwrap();
        assert!(roots.iter().any(|r| r.source_root == expected_root));
        let target = tempfile::tempdir().unwrap();
        let restored = Library::open(&target.path().join("restored.sqlite")).unwrap();
        restored.restore_logical_archive(&archive).unwrap();
        assert_eq!(
            restored.source_locator("track-one").unwrap().unwrap(),
            locator
        );
    }
}
#[test]
fn canonical_windows_opaque_units_are_lossless_and_other_devices_stay_unresolved() {
    let mut units = r"\\?\C:\Music\odd".encode_utf16().collect::<Vec<_>>();
    units.push(0xd800);
    units.extend(".wav".encode_utf16());
    let locator =
        PortableLocator::from_native_path_bytes("opaque", &windows_native_blob(&units)).unwrap();
    assert_eq!(locator.flavor, PathFlavor::OpaqueWindows);
    assert_eq!(decoded_utf16(&locator.original), units);
    assert_eq!(
        decoded_utf16(&locator.components.unwrap()[0]),
        [
            "odd".encode_utf16().collect::<Vec<_>>(),
            vec![0xd800],
            ".wav".encode_utf16().collect()
        ]
        .concat()
    );
    let opaque =
        PortableLocator::from_native_path_bytes("opaque", &windows_native_blob(&units)).unwrap();
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("opaque.sqlite")).unwrap();
    let mut track = imported();
    track.locator = opaque.clone();
    lib.import_portable_tracks(&[track]).unwrap();
    let archive = lib.portable_snapshot().unwrap();
    let target = tempfile::tempdir().unwrap();
    let restored = Library::open(&target.path().join("restored.sqlite")).unwrap();
    restored.restore_logical_archive(&archive).unwrap();
    assert_eq!(
        restored.source_locator("track-one").unwrap().unwrap(),
        opaque
    );
    // Foreign verbatim text alone is inert; only a vetted native decomposition
    // supplies portable root/components.
    let mut foreign = PortableLocator::from_native_path_bytes(
        "foreign",
        &windows_native_blob(&r"\\?\C:\Music\track.wav".encode_utf16().collect::<Vec<_>>()),
    )
    .unwrap();
    foreign.source_root = None;
    foreign.components = None;
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("foreign.sqlite")).unwrap();
    let mut track = imported();
    track.locator = foreign;
    lib.import_portable_tracks(&[track]).unwrap();
    assert!(lib.portable_snapshot().unwrap().records["roots"].is_empty());
    assert!(lib.relocation_roots().unwrap().is_empty());
    for device in [
        r"\\.\PhysicalDrive0",
        r"\\?\GLOBALROOT\Device\HarddiskVolume1\track.wav",
        r"\\?\C:relative.wav",
    ] {
        let units = device.encode_utf16().collect::<Vec<_>>();
        let locator =
            PortableLocator::from_native_path_bytes("device", &windows_native_blob(&units))
                .unwrap();
        assert!(locator.source_root.is_none());
        assert!(locator.components.is_none());
        assert_eq!(decoded_utf16(&locator.original), units);
        let d = tempfile::tempdir().unwrap();
        let lib = Library::open(&d.path().join("db")).unwrap();
        let mut track = imported();
        track.locator = locator;
        lib.import_portable_tracks(&[track]).unwrap();
        let archive = lib.portable_snapshot().unwrap();
        assert!(archive.records["roots"].is_empty());
        assert!(archive.records["locators"][0].value["root_id"].is_null());
        assert!(lib.relocation_roots().unwrap().is_empty());
    }
}
#[cfg(windows)]
#[test]
fn actual_windows_canonical_binding_roundtrips_without_native_path_changes() {
    use std::os::windows::ffi::OsStrExt;
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("canonical.wav");
    std::fs::write(&file, b"synthetic").unwrap();
    let canonical = file.canonicalize().unwrap();
    let units = canonical.as_os_str().encode_wide().collect::<Vec<_>>();
    let locator =
        PortableLocator::from_native_path_bytes("actual-canonical", &windows_native_blob(&units))
            .unwrap();
    assert!(locator.source_root.is_some());
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut track = imported();
    track.locator = locator.clone();
    lib.import_portable_tracks(&[track]).unwrap();
    lib.bind_portable_track("track-one", &file, None).unwrap();
    assert_eq!(lib.track_path("track-one").unwrap(), canonical);
    let archive = lib.portable_snapshot().unwrap();
    let target = tempfile::tempdir().unwrap();
    let restored = Library::open(&target.path().join("db")).unwrap();
    restored.restore_logical_archive(&archive).unwrap();
    assert_eq!(
        restored.source_locator("track-one").unwrap().unwrap(),
        locator
    );
}

#[test]
fn native_nested_library_exports_one_mapping_root_and_full_relative_components() {
    use cuerithm_library::backup::ArchiveLimits;
    for configured in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let music = d.path().join("Music");
        std::fs::create_dir_all(&music).unwrap();
        let music = music.canonicalize().unwrap();
        let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
        if configured {
            let mut settings = lib.settings().unwrap().settings;
            settings.watch_folders = vec![music.clone()];
            lib.save_settings(settings).unwrap();
        }
        for (index, relative) in ["House/A.wav", "Other/B.wav"].iter().enumerate() {
            let file = music.join(relative);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, b"synthetic").unwrap();
            let mut track = imported();
            track.id = format!("native-{index}");
            track.origin = "cuerithm".into();
            track.musical = None;
            track.locator =
                PortableLocator::from_local_path(format!("native-locator-{index}"), &file);
            lib.import_portable_tracks(&[track]).unwrap();
            lib.bind_portable_track(&format!("native-{index}"), &file, None)
                .unwrap();
        }
        // A later machine binding must not replace the original source tree.
        let moved = d.path().join("CurrentBinding/A.wav");
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::write(&moved, b"synthetic").unwrap();
        lib.bind_portable_track("native-0", &moved, None).unwrap();
        let archive = lib.portable_snapshot().unwrap();
        assert_eq!(archive.records["roots"].len(), 1);
        #[cfg(unix)]
        let mut root_native = b"unix:".to_vec();
        #[cfg(windows)]
        let mut root_native = b"windows:".to_vec();
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            root_native.extend(music.as_os_str().as_bytes());
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            root_native.extend(music.as_os_str().encode_wide().flat_map(u16::to_le_bytes));
        }
        let expected = PortableLocator::from_native_path_bytes("root", &root_native)
            .unwrap()
            .original;
        let root: EncodedPath =
            serde_json::from_value(archive.records["roots"][0].value["source_root"].clone())
                .unwrap();
        #[cfg(unix)]
        assert_eq!(root.bytes().unwrap(), expected.bytes().unwrap());
        #[cfg(windows)]
        {
            let text = String::from_utf16(&decoded_utf16(&root)).unwrap();
            assert!(text.replace('/', "\\").ends_with("\\Music"));
            let _ = expected;
        }
        for locator in &archive.records["locators"] {
            assert_eq!(locator.value["components"].as_array().unwrap().len(), 2);
        }
        let limits = ArchiveLimits::default();
        let spool = lib
            .portable_snapshot_spooled(&limits, &|_, _| Ok(()))
            .unwrap();
        assert_eq!(spool.count("roots").unwrap(), 1);
        let target = tempfile::tempdir().unwrap();
        let restored = Library::open(&target.path().join("restored.sqlite")).unwrap();
        restored
            .restore_spooled(&spool, &limits, &|_, _| Ok(()))
            .unwrap();
        let destination = target.path().join("Relocated");
        for (index, relative) in ["House/A.wav", "Other/B.wav"].iter().enumerate() {
            let file = destination.join(relative);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, b"synthetic").unwrap();
            let locator = restored
                .source_locator(&format!("native-{index}"))
                .unwrap()
                .unwrap();
            assert_eq!(locator.components.as_ref().unwrap().len(), 2);
            let selected_root = destination.canonicalize().unwrap();
            let mut mapped = selected_root.clone();
            for component in locator.components.as_ref().unwrap() {
                let text = if component.encoding == PathEncoding::Base64Utf16le {
                    String::from_utf16(&decoded_utf16(component)).unwrap()
                } else {
                    String::from_utf8(component.bytes().unwrap()).unwrap()
                };
                mapped.push(text);
            }
            assert_eq!(mapped, file.canonicalize().unwrap());
            restored
                .bind_portable_track_in_root(
                    &format!("native-{index}"),
                    &mapped,
                    &selected_root,
                    &cuerithm_media::file_sha256(&mapped).unwrap(),
                    &Default::default(),
                )
                .unwrap();
            assert_eq!(
                restored.track_path(&format!("native-{index}")).unwrap(),
                mapped
            );
        }
        // Root projection affects only the archive, never persisted native provenance.
        assert_eq!(
            lib.source_locator("native-0")
                .unwrap()
                .unwrap()
                .components
                .unwrap()
                .len(),
            1
        );
    }
}
#[test]
fn configured_music_root_is_used_even_when_all_tracks_share_a_deeper_parent() {
    for watch_only in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let music = d.path().join("Music");
        let album = music.join("House/Album");
        std::fs::create_dir_all(&album).unwrap();
        let music = music.canonicalize().unwrap();
        let lib = Library::open(&d.path().join("app/db")).unwrap();
        if watch_only {
            let mut settings = lib.settings().unwrap().settings;
            settings.watch_folders = vec![music.clone()];
            lib.save_settings(settings).unwrap();
        } else {
            lib.register_organization_root(music.clone()).unwrap();
        }
        let file = music.join("House/Album/A.wav");
        std::fs::write(&file, b"synthetic").unwrap();
        let mut track = imported();
        track.origin = "cuerithm".into();
        track.musical = None;
        track.locator = PortableLocator::from_local_path("configured-locator", &file);
        lib.import_portable_tracks(&[track]).unwrap();
        lib.bind_portable_track("track-one", &file, None).unwrap();
        let archive = lib.portable_snapshot().unwrap();
        assert_eq!(archive.records["roots"].len(), 1);
        assert_eq!(
            archive.records["locators"][0].value["components"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}
#[test]
fn native_root_aggregation_is_shared_across_spooled_pages() {
    use cuerithm_library::backup::ArchiveLimits;
    let d = tempfile::tempdir().unwrap();
    let music = d.path().join("Music");
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut tracks = vec![];
    let mut files = vec![];
    for index in 0..130 {
        let file = music.join(format!("Album-{index}/A.wav"));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"synthetic").unwrap();
        let file = file.canonicalize().unwrap();
        let mut track = imported();
        track.id = format!("native-page-{index:03}");
        track.origin = "cuerithm".into();
        track.musical = None;
        track.locator = PortableLocator::from_local_path(format!("page-locator-{index}"), &file);
        files.push((track.id.clone(), file));
        tracks.push(track);
    }
    lib.import_portable_tracks(&tracks).unwrap();
    for (id, file) in files {
        lib.bind_portable_track(&id, &file, None).unwrap();
    }
    let spool = lib
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    assert_eq!(spool.count("tracks").unwrap(), 130);
    assert_eq!(spool.count("roots").unwrap(), 1);
    spool
        .for_each("locators", &mut |locator| {
            assert_eq!(locator.value["components"].as_array().unwrap().len(), 2);
            Ok(())
        })
        .unwrap();
}
#[test]
fn aggregate_windows_roots_preserve_opaque_code_units_and_originals() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("db");
    let lib = Library::open(&path).unwrap();
    for (index, parent) in ["House", "Other"].iter().enumerate() {
        let mut units = format!(r"\\?\C:\Music\{parent}\A")
            .encode_utf16()
            .collect::<Vec<_>>();
        units.push(0xd800);
        units.extend(".wav".encode_utf16());
        let native = windows_native_blob(&units);
        let mut track = imported();
        track.id = format!("opaque-native-{index}");
        track.origin = "cuerithm".into();
        track.musical = None;
        track.locator =
            PortableLocator::from_native_path_bytes(format!("opaque-locator-{index}"), &native)
                .unwrap();
        lib.import_portable_tracks(&[track]).unwrap();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE tracks SET path=?1 WHERE id=?2",
                rusqlite::params![native, format!("opaque-native-{index}")],
            )
            .unwrap();
    }
    let archive = lib.portable_snapshot().unwrap();
    assert_eq!(archive.records["roots"].len(), 1);
    let root: EncodedPath =
        serde_json::from_value(archive.records["roots"][0].value["source_root"].clone()).unwrap();
    assert_eq!(
        String::from_utf16(&decoded_utf16(&root)).unwrap(),
        "C:/Music"
    );
    for locator in &archive.records["locators"] {
        assert_eq!(locator.value["source_flavor"], "opaque-windows");
        let components = locator.value["components"].as_array().unwrap();
        assert_eq!(components.len(), 2);
        let leaf: EncodedPath = serde_json::from_value(components[1].clone()).unwrap();
        assert!(decoded_utf16(&leaf).contains(&0xd800));
        let original: EncodedPath =
            serde_json::from_value(locator.value["original"].clone()).unwrap();
        assert!(decoded_utf16(&original).starts_with(&[92, 92, 63, 92]));
    }
}
#[test]
fn configured_filesystem_drive_share_and_volume_roots_group_sibling_artists() {
    for (windows, root) in [
        (true, r"C:\"),
        (true, r"\\server\share"),
        (false, "/Volumes/MusicDrive"),
        (false, "/"),
    ] {
        let native = |text: &str| {
            if windows {
                windows_native_blob(&text.encode_utf16().collect::<Vec<_>>())
            } else {
                let mut bytes = b"unix:".to_vec();
                bytes.extend(text.as_bytes());
                bytes
            }
        };
        let d = tempfile::tempdir().unwrap();
        let database = d.path().join("db");
        let lib = Library::open(&database).unwrap();
        let raw = rusqlite::Connection::open(&database).unwrap();
        raw.execute(
            "INSERT INTO organization_roots VALUES('configured',?1,'{}',1)",
            [native(root)],
        )
        .unwrap();
        for (index, artist) in ["ArtistA", "ArtistB"].iter().enumerate() {
            let text = format!("{}/{artist}/Track.wav", root.trim_end_matches(['/', '\\']));
            let blob = native(&text);
            let mut track = imported();
            track.id = format!("anchored-{index}");
            track.origin = "cuerithm".into();
            track.musical = None;
            track.locator =
                PortableLocator::from_native_path_bytes(format!("anchored-locator-{index}"), &blob)
                    .unwrap();
            lib.import_portable_tracks(&[track]).unwrap();
            raw.execute(
                "UPDATE tracks SET path=?1 WHERE id=?2",
                rusqlite::params![blob, format!("anchored-{index}")],
            )
            .unwrap();
        }
        let archive = lib.portable_snapshot().unwrap();
        assert_eq!(archive.records["roots"].len(), 1, "{root}");
        for locator in &archive.records["locators"] {
            assert_eq!(
                locator.value["components"].as_array().unwrap().len(),
                2,
                "{root}"
            );
        }
    }
}

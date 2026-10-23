use cuerithm_library::{
    BrokenScanRequest, ExtensionMode, Library, MaintenanceFileState, UnusedScanRequest,
};
use cuerithm_media::{CancellationToken, MediaRuntime};
use std::path::{Path, PathBuf};
struct Fixture {
    lib: Library,
    db: PathBuf,
    music: PathBuf,
    _dir: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir(&music).unwrap();
        let db = dir.path().join("app/library.sqlite");
        let lib = Library::open(&db).unwrap();
        Self {
            _dir: dir,
            lib,
            db,
            music,
        }
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.music.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }
    fn add(&self, p: &Path) -> String {
        let j = self
            .lib
            .enqueue(&[p.to_owned()], self.lib.settings().unwrap().revision)
            .unwrap()[0]
            .clone();
        self.lib
            .jobs()
            .unwrap()
            .into_iter()
            .find(|x| x.id == j)
            .unwrap()
            .track_id
    }
    fn request(&self) -> UnusedScanRequest {
        UnusedScanRequest {
            root: self.music.clone(),
            extension_mode: ExtensionMode::All,
            extensions: vec![],
        }
    }
    fn content(&self) -> String {
        let c = rusqlite::Connection::open(&self.db).unwrap();
        c.query_row("SELECT json_group_array(json_array(id,hex(path),state,source_hash,analysis_json,metadata_json,current_job,archived)) FROM tracks",[],|r|r.get(0)).unwrap()
    }
}
fn names(r: &cuerithm_library::UnusedScanReport) -> Vec<String> {
    r.files
        .iter()
        .map(|f| {
            Path::new(&f.display_path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into()
        })
        .collect()
}
#[test]
fn unused_scan_recurses_excludes_live_and_archived_references_and_exports_without_writes() {
    let f = Fixture::new();
    let a = f.file("referenced.flac", b"a");
    let b = f.file("archived.flac", b"b");
    f.add(&a);
    let id = f.add(&b);
    f.lib.set_archived(&id, true).unwrap();
    f.file("nested/loose.flac", b"orphan");
    f.file("cover.jpg", b"art");
    let before = f.content();
    let r = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    assert!(r.complete);
    assert!(!r.stale);
    assert_eq!(r.files.len(), 2);
    assert_eq!(r.size_bytes, 9);
    assert_eq!(f.content(), before);
    assert_eq!(f.lib.unused_scan(&r.id).unwrap().files.len(), 2);
    let text = f.lib.unused_scan_export(&r.id).unwrap();
    assert!(text.contains("loose.flac"));
    assert!(text.contains("cover.jpg"));
    assert!(a.exists() && b.exists());
}
#[test]
fn extension_modes_and_dj_sidecars_follow_the_requested_filters() {
    let f = Fixture::new();
    for p in [
        "track.MP3",
        "track.MP3.vdjstems",
        "track.1.2.serato-stems",
        "unpaired.vdjstems",
        ".DS_Store",
        "._hidden.mp3",
        "_Serato_/database V2",
        "notes.txt",
        ".Trash/trashed.flac",
        ".Trashes/trashed.flac",
        ".AppleDouble/resource",
        "$RECYCLE.BIN/old.wav",
        ".Spotlight-V100/index",
        ".fseventsd/events",
        "AppData/cache.mp3",
        "backup.DS_Store",
    ] {
        f.file(p, b"x");
    }
    let all = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    assert_eq!(
        names(&all),
        vec!["notes.txt", "track.MP3", "unpaired.vdjstems"]
    );
    let r = UnusedScanRequest {
        extension_mode: ExtensionMode::Include,
        extensions: vec![" .mP3 ".into(), "mp3".into()],
        ..f.request()
    };
    let incl = f
        .lib
        .scan_unused_files(r, &CancellationToken::default())
        .unwrap();
    assert_eq!(names(&incl), vec!["track.MP3"]);
    let r = UnusedScanRequest {
        extension_mode: ExtensionMode::Exclude,
        extensions: vec!["txt".into()],
        ..f.request()
    };
    let excl = f
        .lib
        .scan_unused_files(r, &CancellationToken::default())
        .unwrap();
    assert_eq!(names(&excl), vec!["track.MP3", "unpaired.vdjstems"]);
}
#[test]
fn file_and_directory_changes_and_new_library_references_invalidate_saved_unused_reports() {
    let f = Fixture::new();
    let p = f.file("loose.wav", b"original");
    let r = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    let time = p.metadata().unwrap().modified().unwrap();
    std::fs::write(&p, b"modified").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&p)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(time))
        .unwrap();
    assert!(f.lib.unused_scan(&r.id).unwrap().stale);
    let r = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    f.file("new.wav", b"new");
    assert!(f.lib.unused_scan(&r.id).unwrap().stale);
    let r = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    f.add(&p);
    assert!(f.lib.unused_scan(&r.id).unwrap().stale);
}
#[cfg(unix)]
#[test]
fn traversal_does_not_follow_directory_or_file_symlinks() {
    let f = Fixture::new();
    let outside = f._dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep.flac"), b"private").unwrap();
    std::os::unix::fs::symlink(&outside, f.music.join("linked-dir")).unwrap();
    std::os::unix::fs::symlink(outside.join("keep.flac"), f.music.join("linked.flac")).unwrap();
    let r = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    assert!(r.files.is_empty());
    assert!(r.complete);
    assert!(!r.stale);
}
#[test]
fn cancelled_or_changed_scans_cannot_publish_a_complete_fresh_result() {
    let f = Fixture::new();
    f.file("a.flac", b"a");
    let cancel = CancellationToken::default();
    let r = f
        .lib
        .scan_unused_files_with_progress(f.request(), &cancel, |p| {
            if p.completed > 0 {
                cancel.cancel();
            }
        });
    assert!(r.is_err());
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM maintenance_scans", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    drop(conn);
    let r = f
        .lib
        .scan_unused_files_with_progress(f.request(), &CancellationToken::default(), |p| {
            if p.total.is_some() {
                f.file("new.flac", b"new");
            }
        })
        .unwrap();
    assert!(r.stale);
}
fn runtime() -> MediaRuntime {
    MediaRuntime::from_bundle(
        &std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
            }),
    )
    .unwrap()
}
#[test]
fn missing_and_optional_audio_validation_distinguish_decode_errors_from_missing() {
    let f = Fixture::new();
    let good = f.music.join("good.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &good,
    )
    .unwrap();
    let good_id = f.add(&good);
    let bad = f.file("bad.mp3", b"broken media");
    let bad_id = f.add(&bad);
    let missing = f.file("missing.flac", b"x");
    let missing_id = f.add(&missing);
    std::fs::remove_file(&missing).unwrap();
    let before = f.content();
    let rt = runtime();
    let simple = f
        .lib
        .scan_broken_tracks(
            BrokenScanRequest::default(),
            &rt,
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(simple.decoder_identity.is_none());
    assert_eq!(
        simple.tracks.iter().find(|t| t.id == bad_id).unwrap().state,
        MaintenanceFileState::Present
    );
    let r = f
        .lib
        .scan_broken_tracks(
            BrokenScanRequest {
                inspect_audio: true,
                ..Default::default()
            },
            &rt,
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(!r.stale);
    assert_eq!(r.decoder_identity.as_deref(), Some(rt.identity.as_str()));
    assert_eq!(
        r.tracks.iter().find(|t| t.id == missing_id).unwrap().state,
        MaintenanceFileState::Missing
    );
    assert_eq!(
        r.tracks.iter().find(|t| t.id == bad_id).unwrap().state,
        MaintenanceFileState::DecodeError
    );
    assert_eq!(
        r.tracks.iter().find(|t| t.id == good_id).unwrap().state,
        MaintenanceFileState::Present
    );
    assert_eq!(f.content(), before);
    assert!(!f.lib.broken_scan(&r.id).unwrap().stale);
}

#[test]
fn reference_identity_respects_filesystem_case_rules_and_protects_hardlinks() {
    let f = Fixture::new();
    let upper = f.file("Case.wav", b"original");
    let lower = f.music.join("case.wav");
    let insensitive = lower.exists();
    if insensitive {
        f.add(&lower);
    } else {
        f.file("case.wav", b"different");
        f.add(&upper);
    }
    let alias = f.music.join("hardlink.wav");
    std::fs::hard_link(&upper, &alias).unwrap();
    let r = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    assert!(!names(&r).contains(&"hardlink.wav".into()));
    if insensitive {
        assert!(r.files.is_empty());
    } else {
        assert_eq!(names(&r), vec!["case.wav"]);
    }
}

#[test]
fn unsupported_audio_is_not_reported_as_corrupt() {
    let f = Fixture::new();
    let mut bytes = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    bytes.extend(20_u32.to_le_bytes());
    for n in [0x11_u16, 1] {
        bytes.extend(n.to_le_bytes());
    }
    bytes.extend(8000_u32.to_le_bytes());
    bytes.extend(4055_u32.to_le_bytes());
    for n in [256_u16, 4, 2, 505] {
        bytes.extend(n.to_le_bytes());
    }
    bytes.extend(b"data");
    bytes.extend(256_u32.to_le_bytes());
    bytes.extend([0; 256]);
    let length = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    let path = f.file("unsupported-adpcm.wav", &bytes);
    let id = f.add(&path);
    let r = f
        .lib
        .scan_broken_tracks(
            BrokenScanRequest {
                inspect_audio: true,
                ..Default::default()
            },
            &runtime(),
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(
        r.tracks.iter().find(|t| t.id == id).unwrap().state,
        MaintenanceFileState::Unsupported
    );
}

#[test]
fn disappearing_candidates_are_reported_as_partial_instead_of_silently_omitted() {
    let f = Fixture::new();
    let path = f.file("vanishing.flac", b"x");
    let r = f
        .lib
        .scan_unused_files_with_progress(f.request(), &CancellationToken::default(), |p| {
            if p.total.is_none() && path.exists() {
                std::fs::remove_file(&path).unwrap();
            }
        })
        .unwrap();
    assert!(!r.complete);
    assert!(!r.issues.is_empty());
    assert!(r.stale);
    assert!(r.files.is_empty());
}
#[test]
fn own_application_storage_is_excluded_even_with_a_case_alias() {
    let f = Fixture::new();
    let actual = f.music.join("ApplicationData");
    std::fs::create_dir(&actual).unwrap();
    let alias = f.music.join("applicationdata");
    let root = if alias.exists() { alias } else { actual };
    let lib = Library::open(&root.join("library.sqlite")).unwrap();
    std::fs::write(root.join("keep.flac"), b"cache").unwrap();
    f.file("unused.mp3", b"x");
    let r = lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    assert_eq!(names(&r), vec!["unused.mp3"]);
    assert!(!r.stale);
}
#[test]
fn broken_reports_exclude_archives_recheck_reappearing_files_and_cancel_without_saving() {
    let f = Fixture::new();
    let path = f.file("gone.flac", b"x");
    let id = f.add(&path);
    let archived = f.file("archive.mp3", b"bad");
    let old = f.add(&archived);
    f.lib.set_archived(&old, true).unwrap();
    std::fs::remove_file(&path).unwrap();
    let rt = runtime();
    let r = f
        .lib
        .scan_broken_tracks(
            BrokenScanRequest::default(),
            &rt,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(r.tracks.len(), 1);
    assert_eq!(r.tracks[0].id, id);
    std::fs::write(&path, b"back").unwrap();
    assert!(f.lib.broken_scan(&r.id).unwrap().stale);
    let cancel = CancellationToken::default();
    let run =
        f.lib
            .scan_broken_tracks_with_progress(BrokenScanRequest::default(), &rt, &cancel, |p| {
                if p.completed == 1 {
                    cancel.cancel();
                }
            });
    assert!(run.is_err());
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM maintenance_scans", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
}

#[test]
fn all_extension_mode_ignores_dormant_filter_text() {
    let f = Fixture::new();
    f.file("loose.flac", b"x");
    let request = UnusedScanRequest {
        extensions: vec![String::new(), "unused*".into()],
        ..f.request()
    };
    let r = f
        .lib
        .scan_unused_files(request, &CancellationToken::default())
        .unwrap();
    assert_eq!(names(&r), vec!["loose.flac"]);
}

#[test]
fn file_metadata_names_do_not_hide_ordinary_directories() {
    let f = Fixture::new();
    f.file("._album/song.flac", b"x");
    f.file("backup.DS_Store/other.mp3", b"y");
    let r = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    assert_eq!(r.files.len(), 2);
}

#[test]
fn explicit_music_root_under_an_application_data_ancestor_is_scanned_with_reserved_children_pruned()
{
    let mut f = Fixture::new();
    let reserved = if cfg!(target_os = "macos") {
        "Application Support"
    } else {
        "AppData"
    };
    f.music = f._dir.path().join(reserved).join("chosen music");
    std::fs::create_dir_all(&f.music).unwrap();
    f.file("loose.mp3", b"music");
    f.file(&format!("{reserved}/cache.mp3"), b"cache");
    f.file("_Serato_/database V2", b"database");
    let before = f.content();
    let scan = f
        .lib
        .scan_unused_files(f.request(), &CancellationToken::default())
        .unwrap();
    assert!(scan.complete && !scan.stale);
    assert_eq!(names(&scan), vec!["loose.mp3"]);
    assert_eq!(f.content(), before);
    assert!(
        f.lib
            .scan_unused_files(
                UnusedScanRequest {
                    root: f.music.join(reserved),
                    ..f.request()
                },
                &CancellationToken::default()
            )
            .is_err()
    );
}

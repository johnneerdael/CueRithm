//! Shared by the tests that need a library with ready tracks but no decoder:
//! tracks are made ready with a synthetic analysis and the "audio" is distinct
//! bytes, so only the identity (source SHA-256) and the stored path matter.
#![allow(dead_code)]
use camelot_core::{
    AnalysisConfig, FileAnalysis, KeyEstimate, Mode, MusicalKey, TempoEstimate,
    energy::TrackEnergy,
    lexicon_cues::{StructuralCue, StructuralCueResult},
};
use cuerithm_library::{
    BrokenScanReport, BrokenScanRequest, Library, PlaylistEdit, PlaylistKind, PreparationService,
    PreparationSettings, WatchMoveOutcome,
};
use cuerithm_media::{CancellationToken, MediaInfo, MediaRuntime};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

pub fn analysis(bpm: f64) -> FileAnalysis {
    let mut a =
        camelot_core::analyze_pcm(&vec![0.; 11025], 11025, &AnalysisConfig::default()).unwrap();
    a.key = Some(KeyEstimate {
        key: MusicalKey::new(0, Mode::Major).unwrap(),
        camelot: "8B".into(),
        margin: 0.5,
        features: None,
        candidates: vec![],
    });
    a.tempo = Some(TempoEstimate {
        bpm,
        strength: None,
        candidates: vec![],
        beats_seconds: vec![0., 0.5],
        downbeats_seconds: None,
        downbeat_time: Some(0.),
        method: "test".into(),
    });
    a.energy = Some(TrackEnergy {
        level: 5,
        method: "test".into(),
    });
    a.lexicon_cues = Some(StructuralCueResult {
        cues: vec![StructuralCue {
            time_seconds: 0.,
            name: format!("Start {bpm}"),
            color: "Red".into(),
        }],
        emergency_loop: None,
    });
    FileAnalysis {
        selection: None,
        selected_beatgrid: None,
        publication: None,
        analysis: a,
        complete: true,
        verified_integrity: None,
        source_sample_rate: 44100,
        beat_sample_rate: None,
        neural_key_sample_rate: None,
        openkeyscan_sample_rate: None,
        source_timing: None,
        elapsed_seconds: 0.,
        media: None,
    }
}
pub fn info() -> MediaInfo {
    MediaInfo {
        codec: "flac".into(),
        sample_rate: 44100,
        channels: 2,
        bits_per_sample: Some(16),
        tags: Default::default(),
        properties: Default::default(),
    }
}
/// A decoder folder whose binaries only satisfy the integrity check. Anything
/// that reaches the decoder fails, which is how "became a new track" shows up.
pub fn offline_runtime(dir: &Path) -> MediaRuntime {
    std::fs::create_dir_all(dir).unwrap();
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let mut manifest = serde_json::Map::new();
    for name in ["ffmpeg", "ffprobe"] {
        let file = dir.join(format!("{name}{ext}"));
        std::fs::write(&file, format!("not a decoder: {name}")).unwrap();
        manifest.insert(
            format!("{name}_sha256"),
            cuerithm_media::file_sha256(&file).unwrap().into(),
        );
    }
    manifest.insert("recipe".into(), "offline-test".into());
    std::fs::write(
        dir.join("runtime.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    MediaRuntime::from_bundle(dir).unwrap()
}
pub fn age(path: &Path) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
}
pub fn wait_until(what: &str, seconds: u64, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(seconds),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub struct Fixture {
    pub _dir: tempfile::TempDir,
    pub root: PathBuf,
    pub watch: PathBuf,
    pub db: PathBuf,
    pub lib: Library,
}
impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let watch = root.join("beatport");
        std::fs::create_dir_all(&watch).unwrap();
        let db = root.join("app/library.sqlite");
        let lib = Library::open(&db).unwrap();
        lib.save_settings(PreparationSettings {
            watch_folders: vec![watch.clone()],
            stability_millis: 20,
            ..Default::default()
        })
        .unwrap();
        Self {
            _dir: dir,
            root,
            watch,
            db,
            lib,
        }
    }
    /// Distinct, settled "audio" for `seed`.
    pub fn file(&self, folder: &Path, name: &str, seed: usize) -> PathBuf {
        std::fs::create_dir_all(folder).unwrap();
        let path = folder.join(name);
        std::fs::write(&path, format!("audio bytes of track {seed}\n").repeat(40)).unwrap();
        age(&path);
        path
    }
    pub fn ready(&self, path: &Path, bpm: f64) -> String {
        let revision = self.lib.settings().unwrap().revision;
        let job = self.lib.enqueue(&[path.to_path_buf()], revision).unwrap()[0].clone();
        let run = self.lib.claim_next().unwrap().unwrap();
        assert_eq!(run.id, job);
        self.lib
            .apply_analysis(&run.id, &analysis(bpm), &info(), &run.source_sha256)
            .unwrap();
        run.track_id
    }
    pub fn start(&self) -> PreparationService {
        PreparationService::start_with_workers(
            self.lib.clone(),
            offline_runtime(&self.root.join("decoder")),
            1,
        )
        .unwrap()
    }
    pub fn relink(&self, candidates: &[PathBuf]) -> WatchMoveOutcome {
        self.lib
            .relink_moved_tracks(candidates, std::slice::from_ref(&self.watch), |_| true)
            .unwrap()
    }
    pub fn sql<T>(&self, query: &str, f: impl FnOnce(&rusqlite::Row<'_>) -> T) -> T {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .query_row(query, [], |r| Ok(f(r)))
            .unwrap()
    }
    pub fn exec(&self, query: &str) {
        rusqlite::Connection::open(&self.db)
            .unwrap()
            .execute_batch(query)
            .unwrap();
    }
    pub fn total_tracks(&self) -> u64 {
        let c = self.lib.counts().unwrap();
        c.tracks + c.incoming + c.archive
    }
}

impl Fixture {
    /// A broken-file report over the current library, without decoding.
    pub fn scan(&self, include_archived: bool) -> BrokenScanReport {
        self.lib
            .scan_broken_tracks(
                BrokenScanRequest {
                    inspect_audio: false,
                    include_archived,
                },
                &offline_runtime(&self.root.join("decoder")),
                &CancellationToken::default(),
            )
            .unwrap()
    }
    pub fn playlist(&self, id: &str, members: &[&str]) {
        let revision = || self.lib.playlist_state().unwrap().revision;
        self.lib
            .apply_playlist_edit(
                PlaylistEdit::Create {
                    id: Some(id.into()),
                    parent_id: None,
                    kind: PlaylistKind::Playlist,
                    name: format!("Playlist {id}"),
                    definition: None,
                },
                revision(),
            )
            .unwrap();
        self.lib
            .apply_playlist_edit(
                PlaylistEdit::AddMembers {
                    id: id.into(),
                    track_ids: members.iter().map(|m| m.to_string()).collect(),
                },
                revision(),
            )
            .unwrap();
    }
    /// Every row of every table except the bookkeeping that legitimately moves
    /// when records come and go. Equal dumps mean an identical library.
    pub fn dump(&self) -> BTreeMap<String, Vec<String>> {
        const BOOKKEEPING: &[&str] = &[
            "meta",
            "maintenance_scans",
            "maintenance_track_epochs",
            "maintenance_playlist_epochs",
            "smart_playlist_status",
        ];
        let conn = rusqlite::Connection::open(&self.db).unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let mut dump = BTreeMap::new();
        for table in tables.iter().filter(|t| !BOOKKEEPING.contains(&t.as_str())) {
            // SQLite's internal rowid is handed out again after a delete and carries no
            // meaning; rows are compared by their declared columns.
            let select = "*";
            let mut statement = conn
                .prepare(&format!("SELECT {select} FROM \"{table}\""))
                .unwrap();
            let columns = statement.column_count();
            let mut rows: Vec<String> = statement
                .query_map([], |row| {
                    Ok((0..columns)
                        .map(|i| format!("{:?}", row.get_ref(i).unwrap()))
                        .collect::<Vec<_>>()
                        .join("|"))
                })
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            rows.sort();
            dump.insert(table.clone(), rows);
        }
        dump
    }
}

/// Equal dumps, or a message naming the first table and rows that differ.
pub fn assert_same_dump(now: &BTreeMap<String, Vec<String>>, was: &BTreeMap<String, Vec<String>>) {
    for (table, rows) in was {
        let current = &now[table];
        if current != rows {
            let missing: Vec<_> = rows.iter().filter(|r| !current.contains(r)).collect();
            let extra: Vec<_> = current.iter().filter(|r| !rows.contains(r)).collect();
            panic!("table {table} differs\nmissing: {missing:#?}\nextra: {extra:#?}");
        }
    }
    assert_eq!(
        now.keys().collect::<Vec<_>>(),
        was.keys().collect::<Vec<_>>()
    );
}

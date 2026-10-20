//! Large-library timing, memory and recovery run on generated files. Ignored by default:
//!
//! ```sh
//! CUERITHM_SCALE_TRACKS=10000 cargo test -p cuerithm-library --release --test scale_benchmark -- --ignored --nocapture
//! ```
//!
//! `CUERITHM_SCALE_TRACKS` sets the library size (default 10000) and `CUERITHM_SCALE_DUP_EVERY` makes every n-th
//! track a copy of the one before it (same audio, different file bytes; default 20, so 5% of tracks are duplicates). Each file is one second
//! of 8 kHz mono PCM, so the run measures how the library, maintenance scan, review and merge code scale with the
//! number of tracks, not decode throughput. Source files are hashed before and after to show nothing wrote to them.
use cuerithm_library::{DuplicateScanRequest, Library};
use cuerithm_media::{CancellationToken, MediaRuntime};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .unwrap_or(0.0)
        / 1024.0
}

fn write_wav(path: &Path, seed: u64) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut out = hound::WavWriter::create(path, spec).unwrap();
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    for _ in 0..8000 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        out.write_sample::<i16>((state >> 48) as i16 / 4).unwrap();
    }
    out.finalize().unwrap();
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn step<T>(name: &str, work: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = work();
    println!(
        "{name:<44} {:>9.2} s   rss {:>7.0} MB",
        start.elapsed().as_secs_f64(),
        rss_mb()
    );
    value
}

#[test]
#[ignore = "large generated library; run explicitly with --ignored"]
fn large_library_scan_review_merge_and_undo() {
    let tracks = env_usize("CUERITHM_SCALE_TRACKS", 10_000);
    let dup_every = env_usize("CUERITHM_SCALE_DUP_EVERY", 20).max(2);
    let dir = tempfile::tempdir().unwrap();
    let music = dir.path().join("music");
    std::fs::create_dir_all(&music).unwrap();
    let runtime_root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    let runtime = MediaRuntime::from_bundle(&runtime_root).unwrap();
    println!("\n{tracks} tracks, every {dup_every}th a copy of the one before it");

    let mut paths = Vec::with_capacity(tracks);
    let mut expected_duplicates = 0;
    step("generate files", || {
        for i in 0..tracks {
            let path = music.join(format!("t{i:06}.wav"));
            if i % dup_every == dup_every - 1 {
                // Same audio, different bytes (an extra RIFF chunk), so the scan has to decode to match them.
                let mut bytes = std::fs::read(&paths[i - 1]).unwrap();
                bytes.extend_from_slice(b"JUNK\x04\0\0\0tag!");
                let size = (bytes.len() - 8) as u32;
                bytes[4..8].copy_from_slice(&size.to_le_bytes());
                std::fs::write(&path, bytes).unwrap();
                expected_duplicates += 1;
            } else {
                write_wav(&path, i as u64 + 1);
            }
            paths.push(path);
        }
    });
    let sample: Vec<_> = paths
        .iter()
        .step_by((tracks / 200).max(1))
        .cloned()
        .collect();
    let hashes: Vec<_> = sample
        .iter()
        .map(|p| cuerithm_media::file_sha256(p).unwrap())
        .collect();

    let db = dir.path().join("app/library.sqlite");
    let lib = Library::open(&db).unwrap();
    step("enqueue (import) all tracks", || {
        let revision = lib.settings().unwrap().revision;
        for chunk in paths.chunks(500) {
            lib.enqueue(chunk, revision.clone()).unwrap();
        }
    });
    // The scan fingerprints every present file whose recorded duration is between 15 and 1500 seconds, whatever its
    // real length, so the rows say 20 s. Duplicates share a title and artist; every other track has its own.
    step("mark ready with metadata", || {
        let conn = rusqlite::Connection::open(&db).unwrap();
        let base = format!("(rowid - CASE WHEN rowid % {dup_every} = 0 THEN 1 ELSE 0 END)");
        conn.execute(
            &format!(
                "UPDATE tracks SET title='Track '||{base}, artist='Artist '||({base}%500), duration=20, state='ready', \
                 analysis_json='{{\"untouched\":true}}', \
                 metadata_json='{{\"codec\":\"pcm_s16le\",\"sample_rate\":8000,\"channels\":1,\"tags\":{{}},\"properties\":{{\"audio.bit_rate\":\"128000\"}}}}'"
            ),
            [],
        )
        .unwrap();
    });
    let token = CancellationToken::default();
    let scan = step("duplicate scan, cold (fingerprints)", || {
        lib.scan_duplicates(DuplicateScanRequest::default(), &runtime, &token)
            .unwrap()
    });
    println!(
        "  tracks {} groups {} fingerprints computed {} cache hits {}",
        scan.tracks.len(),
        scan.groups.len(),
        scan.fingerprints_computed,
        scan.cache_hits
    );
    assert_eq!(scan.tracks.len(), tracks);
    assert_eq!(scan.groups.len(), expected_duplicates);
    let warm = step("duplicate scan, warm (cache)", || {
        lib.scan_duplicates(DuplicateScanRequest::default(), &runtime, &token)
            .unwrap()
    });
    assert_eq!(warm.fingerprints_computed, 0);
    assert_eq!(warm.groups.len(), expected_duplicates);
    let review = step("begin duplicate review", || {
        lib.begin_duplicate_review(&warm.id, &token).unwrap()
    });
    let saved_bytes: i64 = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT length(payload_json) FROM maintenance_scans WHERE id=?1",
            [&review.id],
            |row| row.get(0),
        )
        .unwrap_or(-1);
    println!(
        "  saved review row: {:.1} MB",
        saved_bytes as f64 / 1_048_576.0
    );
    let review = step("reload duplicate review", || {
        lib.duplicate_review(&review.id, &token).unwrap()
    });
    assert_eq!(review.groups.len(), expected_duplicates);
    // Every choice in the review screen is one persisted edit, so this is the latency of a single click. It runs on
    // a second review so the one that is merged below keeps every group unlocked.
    let spare = lib.begin_duplicate_review(&warm.id, &token).unwrap();
    step("one review edit (lock a group)", || {
        lib.edit_duplicate_review(
            &spare.id,
            spare.revision,
            cuerithm_library::DuplicateReviewEdit::Group {
                group_id: spare.groups[0].id.clone(),
                edit: cuerithm_library::GroupEdit::Lock,
            },
            &token,
        )
        .unwrap()
    });
    let groups: Vec<String> = review.groups.iter().map(|g| g.id.clone()).collect();
    let preview = step("preview merge of every group", || {
        lib.preview_duplicate_merge(&review.id, review.revision, groups, &token)
            .unwrap()
    });
    let applied = step("apply merge", || {
        lib.apply_duplicate_merge(&preview.id, &token).unwrap()
    });
    println!("  archived {}", applied.archived_ids.len());
    assert_eq!(applied.archived_ids.len(), expected_duplicates);
    let undone = step("undo merge", || {
        lib.undo_duplicate_merge(&applied.id, &token).unwrap()
    });
    assert_eq!(undone.undo_of.as_deref(), Some(applied.id.as_str()));
    let archived: i64 = rusqlite::Connection::open(&db)
        .unwrap()
        .query_row("SELECT count(*) FROM tracks WHERE archived=1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(archived, 0, "undo restores every archived track");
    drop(lib);
    let reopened = step("reopen library", || Library::open(&db).unwrap());
    assert_eq!(reopened.duplicate_merge_history().unwrap().len(), 1);
    let after: Vec<_> = sample
        .iter()
        .map(|p| cuerithm_media::file_sha256(p).unwrap())
        .collect();
    assert_eq!(hashes, after, "no source file was modified");
    println!("sampled source files unchanged: {}", sample.len());
}

/// Cold and warm duplicate scans over real audio files, for decode and hashing cost that generated one-second
/// files cannot show. Point `CUERITHM_SCALE_REAL_DIR` at a folder of audio (copies; nothing is written to it).
#[test]
#[ignore = "needs CUERITHM_SCALE_REAL_DIR pointing at a folder of real audio files"]
fn real_audio_cold_and_warm_duplicate_scan() {
    let Some(folder) = std::env::var_os("CUERITHM_SCALE_REAL_DIR").map(PathBuf::from) else {
        panic!("set CUERITHM_SCALE_REAL_DIR");
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    let megabytes: f64 = paths
        .iter()
        .map(|p| std::fs::metadata(p).unwrap().len() as f64)
        .sum::<f64>()
        / 1_048_576.0;
    println!("\n{} real files, {:.0} MB", paths.len(), megabytes);
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("app/library.sqlite");
    let lib = Library::open(&db).unwrap();
    let runtime_root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    let runtime = MediaRuntime::from_bundle(&runtime_root).unwrap();
    let revision = lib.settings().unwrap().revision;
    for chunk in paths.chunks(500) {
        lib.enqueue(chunk, revision.clone()).unwrap();
    }
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE tracks SET title='Track '||rowid, artist='Artist', duration=300, state='ready'",
            [],
        )
        .unwrap();
    let token = CancellationToken::default();
    let started = Instant::now();
    let cold = step("duplicate scan, cold", || {
        lib.scan_duplicates(DuplicateScanRequest::default(), &runtime, &token)
            .unwrap()
    });
    let seconds = started.elapsed().as_secs_f64();
    println!(
        "  fingerprints computed {} groups {} ({:.1} files/s, {:.0} MB/s of source audio)",
        cold.fingerprints_computed,
        cold.groups.len(),
        paths.len() as f64 / seconds,
        megabytes / seconds
    );
    let warm = step("duplicate scan, warm", || {
        lib.scan_duplicates(DuplicateScanRequest::default(), &runtime, &token)
            .unwrap()
    });
    assert_eq!(warm.fingerprints_computed, 0);
    assert_eq!(warm.cache_hits, cold.fingerprints_computed);
}

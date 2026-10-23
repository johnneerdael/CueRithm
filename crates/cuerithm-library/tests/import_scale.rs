use cuerithm_dj_sync::import::*;
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use std::{collections::BTreeSet, io::Write, time::Instant};
const TRACKS: u64 = 100_000;
fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("VmHWM:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0)
}
#[test]
fn hierarchy_depth_128_is_valid_and_129_is_rejected() {
    for depth in [128, 129] {
        let header:ObservationHeader=serde_json::from_value(serde_json::json!({"provider":"m3u","library_instance_id":"authored-depth","schema_fingerprint":"a".repeat(64),"snapshot_hash":"b".repeat(64),"completeness":"whole_library","accounting":{"tracks_seen":0,"collections_seen":depth,"memberships_seen":0}})).unwrap();
        let mut records = (0..depth).map(|n| {
            ObservationRecord::Collection(CollectionObservation {
                external_id: format!("folder-{n}"),
                parent_id: (n > 0).then(|| format!("folder-{}", n - 1)),
                name: format!("Folder {n}"),
                kind: CollectionKind::Folder,
                ..Default::default()
            })
        });
        let result = validate_observation(&header, &mut records, &ImportLimits::default());
        assert_eq!(result.is_ok(), depth == 128, "{depth}: {result:?}");
    }
}
struct CancelAtBatch {
    cancel: CancellationToken,
    records: usize,
    cancelled_at: Option<Instant>,
}
impl ObservationSink for CancelAtBatch {
    fn put_blob(&mut self, _: &[u8], _: Option<&str>) -> anyhow::Result<BlobRef> {
        anyhow::bail!("M3U must not read audio tags")
    }
    fn push(&mut self, _: ObservationRecord) -> anyhow::Result<()> {
        self.records += 1;
        if self.records == 1000 {
            self.cancelled_at = Some(Instant::now());
            self.cancel.cancel();
        }
        Ok(())
    }
}
#[test]
fn hundred_thousand_tracks_remain_paged_and_cancellable() {
    let root = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let baseline = rss_kib();
    let start = Instant::now();
    let media = root.path().join("media");
    std::fs::create_dir(&media).unwrap();
    let original = root.path().join("authored.wav");
    let mut wav = hound::WavWriter::create(
        &original,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    wav.write_sample(0i16).unwrap();
    wav.finalize().unwrap();
    let mut playlist =
        std::io::BufWriter::new(std::fs::File::create(root.path().join("scale.m3u8")).unwrap());
    for n in 0..TRACKS {
        std::fs::copy(&original, media.join(format!("{n}.wav"))).unwrap();
        writeln!(playlist, "media/{n}.wav").unwrap();
    }
    // Preserve repeated A/B/A membership without inventing additional tracks.
    writeln!(playlist, "media/0.wav\nmedia/1.wav\nmedia/0.wav").unwrap();
    drop(playlist);
    let artifacts = vec![ArtifactRequest::stable("scale.m3u8")];
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::M3u,
        root.path(),
        artifacts.clone(),
    )
    .unwrap();
    let snapshot = capture_source(
        &request,
        db.path(),
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let cancel = CancellationToken::default();
    let mut sink = CancelAtBatch {
        cancel: cancel.clone(),
        records: 0,
        cancelled_at: None,
    };
    assert!(
        adapter(SourceProvider::M3u)
            .read(&snapshot, &ReadOptions::default(), &mut sink, &cancel)
            .is_err()
    );
    let cancellation = sink.cancelled_at.unwrap().elapsed();
    assert!(
        cancellation.as_secs_f64() <= 2.,
        "cancel acknowledgement {cancellation:?}"
    );
    assert!((1000..=1001).contains(&sink.records));
    let library = Library::open(&db.path().join("library.db")).unwrap();
    let before_inspect = Instant::now();
    let inspection = library
        .inspect_import(
            ImportInspectRequest {
                provider: SourceProvider::M3u,
                selection: ImportSourceSelection {
                    root: root.path().to_owned(),
                    artifacts,
                },
                binding: SourceBindingChoice::Automatic,
                origin: None,
                path_mappings: vec![],
                capture_file_tags: false,
                include_history: false,
                include_queue: false,
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let inspect_time = before_inspect.elapsed();
    assert_eq!(inspection.counts.tracks, TRACKS);
    assert_eq!(inspection.counts.memberships, TRACKS + 3);
    let mut cursor = None;
    let mut ids = BTreeSet::new();
    let mut max_page = 0;
    loop {
        let page = library
            .import_page(
                &inspection.inspection_id,
                ImportPageQuery {
                    kind: ImportRecordKind::Track,
                    limit: 1000,
                    cursor,
                },
            )
            .unwrap();
        max_page = max_page.max(page.records.len());
        for record in page.records {
            let ObservationRecord::Track(track) = record else {
                panic!("wrong page kind")
            };
            assert!(ids.insert(track.external_id));
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ids.len() as u64, TRACKS);
    assert!(max_page <= 1000);
    let before_preview = Instant::now();
    let preview = library
        .preview_import(
            ImportPreviewRequest {
                inspection_id: inspection.inspection_id,
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let preview_time = before_preview.elapsed();
    assert_eq!(preview.tracks_created, TRACKS);
    assert!(!preview.processing_requested);
    let mut cursor = None;
    let mut effects = 0;
    loop {
        let page = library
            .import_preview_page(&preview.preview_id, cursor.as_deref(), 1000)
            .unwrap();
        assert!(page.effects.len() <= 1000);
        effects += page.effects.iter().filter(|e| e.kind == "track").count();
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(effects as u64, TRACKS);
    let before_apply = Instant::now();
    let result = library
        .apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    let apply_time = before_apply.elapsed();
    assert_eq!(result.tracks_created, TRACKS);
    let mut cursor = None;
    let mut revision = None;
    let mut tracks = BTreeSet::new();
    loop {
        let page = library
            .tracks(TrackQuery {
                limit: 500,
                cursor,
                expected_revision: revision,
                ..Default::default()
            })
            .unwrap();
        revision = Some(page.revision);
        for track in page.rows {
            assert_eq!(track.import_state.unwrap().availability, "available");
            assert!(tracks.insert(track.id));
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(tracks.len() as u64, TRACKS);
    assert!(library.jobs().unwrap().is_empty());
    assert!(
        library
            .import_processing_stages(&result.operation_id, None, 1000)
            .unwrap()
            .records
            .is_empty()
    );
    let peak = rss_kib();
    let incremental = peak.saturating_sub(baseline);
    assert!(
        incremental <= 512 * 1024,
        "incremental peak memory {incremental} KiB"
    );
    println!(
        "authored_scale os={} tracks={TRACKS} available_media={TRACKS} memberships={} max_page={max_page} cancellation_ms={:.3} peak_kib={peak} incremental_peak_kib={incremental} inspect_s={:.3} preview_s={:.3} apply_s={:.3} total_s={:.3}",
        std::env::consts::OS,
        TRACKS + 3,
        cancellation.as_secs_f64() * 1000.,
        inspect_time.as_secs_f64(),
        preview_time.as_secs_f64(),
        apply_time.as_secs_f64(),
        start.elapsed().as_secs_f64()
    );
}

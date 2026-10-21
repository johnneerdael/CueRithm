use cuerithm_dj_sync::*;
use cuerithm_media::{CancellationToken, MediaRuntime};
use cuerithm_rekordbox::{NativeCue, NativeCueSlot, NativeTime, anlz::*};
use std::path::{Path, PathBuf};

fn runtime() -> MediaRuntime {
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&root)
        .expect("packaged runtime is required for native format coverage")
}

#[test]
fn every_source_layout_gets_measured_native_waveforms_and_all_required_sections() {
    let d = tempfile::tempdir().unwrap();
    let archive = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/media.tar.gz"),
    )
    .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(d.path())
        .unwrap();
    let rows: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let runtime = runtime();
    let cancel = CancellationToken::default();
    for row in rows.as_array().unwrap() {
        let path = d.path().join(row["file"].as_str().unwrap());
        let audio = runtime.decode(&path, d.path()).unwrap();
        let track = SyncTrack {
            native_path: None,
            id: "fixture".into(),
            path: path.clone(),
            sample_rate: audio.info.sample_rate,
            duration_seconds: audio.clock.duration_seconds(),
            export_timing: audio.export_timing.clone(),
            beatgrid: vec![SyncBeat {
                seconds: 0.1,
                bpm: 120.,
                beat: 1,
            }],
            ..Default::default()
        };
        let timed = timing::export_track(&track, DjSoftware::Rekordbox).unwrap();
        let offset = timed.native_timeline.as_ref().unwrap().offset_seconds;
        let cues = vec![NativeCue {
            slot: NativeCueSlot::Memory,
            start: NativeTime::from_seconds(0.2 + offset).unwrap(),
            end: Some(NativeTime::from_seconds(0.5 + offset).unwrap()),
            active_loop: true,
            order_first: u16::MAX,
            order_last: u16::MAX,
            comment: "Fixture loop".into(),
            memory_color: 0,
            color: [0; 4],
            loop_fraction: (4, 1),
        }];
        let generated = native_rekordbox_analysis(&timed, &cues, &runtime, &cancel).unwrap();
        let dat = AnlzFile::parse(&generated.dat, ParseLimits::default()).unwrap();
        let ext = AnlzFile::parse(&generated.ext, ParseLimits::default()).unwrap();
        let modern = AnlzFile::parse(&generated.two_ex, ParseLimits::default()).unwrap();
        let ids = |file: &AnlzFile| file.tags().iter().map(AnlzTag::fourcc).collect::<Vec<_>>();
        assert_eq!(
            ids(&dat),
            [
                *b"PPTH", *b"PVBR", *b"PQTZ", *b"PWAV", *b"PWV2", *b"PCOB", *b"PCOB"
            ]
        );
        assert_eq!(
            ids(&ext),
            [
                *b"PPTH", *b"PWV3", *b"PWV4", *b"PWV5", *b"PCOB", *b"PCOB", *b"PCO2", *b"PCO2"
            ]
        );
        assert_eq!(ids(&modern), [*b"PPTH", *b"PWV6", *b"PWV7", *b"PWVC"]);
        assert_eq!(dat.tags()[0].audio_path().unwrap(), path.to_str().unwrap());
        let waveform = dat.tags()[3].bytes();
        assert!(
            waveform[20..].windows(2).any(|pair| pair[0] != pair[1]),
            "measured waveform must reflect authored pulses: {}",
            path.display()
        );
        assert!(!dat.tags()[2].beats().unwrap().is_empty());
        assert_eq!(
            native_rekordbox_beats(&timed).unwrap(),
            dat.tags()[2].beats().unwrap()
        );
        let memory = dat.tags()[6].cues().unwrap();
        assert_eq!(memory.entries[0].legacy.as_ref().unwrap().status, 4);
        assert_eq!(memory.entries[0].time_ms, cues[0].start.millis());
        assert_eq!(
            memory.entries[0].loop_time_ms,
            cues[0].end.unwrap().millis()
        );
        assert_eq!(
            runtime.decode(&path, d.path()).unwrap().source_sha256,
            audio.source_sha256
        );
        assert!(native_rekordbox_analysis(&track, &cues, &runtime, &cancel).is_err());
    }
}

#[test]
fn explicit_native_target_rejects_the_generic_writer_before_any_staging() {
    let d = tempfile::tempdir().unwrap();
    let error = prepare_sync(
        d.path(),
        SyncTarget {
            environment: Default::default(),
            software: DjSoftware::Rekordbox,
            path: d.path().join("missing.db"),
            source_root: None,
        },
        &SyncLibrary::default(),
        &SyncOptions::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("native rekordbox"));
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
    assert_eq!(
        serde_json::to_string(&DjSoftware::Rekordbox).unwrap(),
        "\"rekordbox\""
    );
}

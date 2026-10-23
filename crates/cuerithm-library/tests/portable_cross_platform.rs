//! Each OS runner restores authored libraries from all three source OS flavors.
//! The ordinary desktop CI matrix therefore exercises nine source/destination
//! combinations, with decoded PCM playback and no physical audio-device claim.
use cuerithm_library::{backup::*, playback::CacheSource, *};
use cuerithm_media::{CancellationToken, MediaRuntime, file_sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[test]
fn source_os_locators_restore_and_play_every_bundled_fixture_format() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let music = root.join("mapped-music");
    std::fs::create_dir_all(&music).unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = std::fs::File::open(workspace.join("tools/fixtures/media.tar.gz")).unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(fixtures))
        .unpack(&music)
        .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(music.join("cases.json")).unwrap()).unwrap();
    let runtime_path = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("tools/media-runtime/bin"));
    let runtime = MediaRuntime::from_bundle(&runtime_path)
        .expect("Bundled runtime required; codec coverage must not skip");
    let limits = ArchiveLimits::default();
    for (source_os, flavor, source_root) in [
        ("macos", PathFlavor::Posix, "/Volumes/Source/Music"),
        ("windows", PathFlavor::WindowsDrive, "D:/Music"),
        ("linux", PathFlavor::Posix, "/srv/music"),
    ] {
        let source =
            Library::open(&root.join(format!("{source_os}-source/library.sqlite"))).unwrap();
        let mut records = vec![];
        for (index, row) in cases.as_array().unwrap().iter().enumerate() {
            let filename = row["file"].as_str().unwrap();
            let path = music.join(filename);
            let hash = file_sha256(&path).unwrap();
            records.push(PortableTrack {
                id: format!("{source_os}-{index}"),
                locator: PortableLocator {
                    id: format!("locator-{source_os}-{index}"),
                    flavor: flavor.clone(),
                    original: EncodedPath::utf8(format!("{source_root}/{filename}")),
                    source_root: Some(EncodedPath::utf8(source_root)),
                    components: Some(filename.split('/').map(EncodedPath::utf8).collect()),
                },
                source_sha256: Some(hash),
                title: format!("Authored {source_os} format {index}"),
                artist: "Synthetic fixture".into(),
                album: String::new(),
                genre: String::new(),
                metadata: BTreeMap::new(),
                key: Some("8A".into()),
                bpm: Some(128.),
                energy: Some(7),
                duration: Some(row["frames"].as_u64().unwrap() as f64 / 44100.),
                archived: false,
                origin: "lexicon".into(),
                analysis_status: AnalysisStatus::Imported,
                musical: Some(AcceptedMusicalData {
                    id: format!("musical-{source_os}-{index}"),
                    source: "lexicon".into(),
                    cues: vec![AcceptedCue {
                        id: format!("cue-{source_os}-{index}"),
                        kind: CueKind::Cue,
                        start_seconds: -0.25,
                        end_seconds: None,
                        slot: Some(0),
                        memory: true,
                        name: Some("Saved lead-in".into()),
                        color: Some("blue".into()),
                        active_loop: false,
                        source_data: BTreeMap::new(),
                    }],
                    tempo_markers: vec![],
                }),
            });
        }
        source.import_portable_tracks(&records).unwrap();
        let archive = source
            .portable_snapshot_spooled(&limits, &|_, _| Ok(()))
            .unwrap();
        let file = root.join(format!("{source_os}.cuerithm"));
        write_portable_spooled(&file, &archive, &limits, &|_, _| Ok(())).unwrap();
        drop(archive);
        drop(source);
        let restored =
            Library::open(&root.join(format!("{source_os}-restored/library.sqlite"))).unwrap();
        let read = read_portable_spooled(&file, &limits, &|_, _| Ok(())).unwrap();
        restored
            .restore_spooled(&read, &limits, &|_, _| Ok(()))
            .unwrap();
        assert!(restored.jobs().unwrap().is_empty());
        for (index, row) in cases.as_array().unwrap().iter().enumerate() {
            let id = format!("{source_os}-{index}");
            let path = music.join(row["file"].as_str().unwrap());
            let hash = file_sha256(&path).unwrap();
            let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
            let locator = restored.source_locator(&id).unwrap().unwrap();
            assert_eq!(locator.flavor, flavor);
            assert_eq!(locator.original, records[index].locator.original);
            runtime.probe(&path).unwrap();
            restored
                .bind_portable_track_in_root(
                    &id,
                    &path,
                    &music,
                    &hash,
                    &CancellationToken::default(),
                )
                .unwrap();
            let audio = Arc::new(
                runtime
                    .decode(
                        &restored.track_path(&id).unwrap(),
                        &root.join("playback-cache"),
                    )
                    .unwrap(),
            );
            assert_eq!(audio.frames, row["frames"].as_u64().unwrap());
            assert!(
                CacheSource::open(audio)
                    .unwrap()
                    .take(1024)
                    .all(f32::is_finite)
            );
            assert_eq!(
                restored.accepted_musical_data(&id).unwrap().unwrap().cues[0].start_seconds,
                -0.25
            );
            assert!(restored.analysis(&id).unwrap().is_none());
            assert_eq!(file_sha256(&path).unwrap(), hash);
            assert_eq!(
                std::fs::metadata(path).unwrap().modified().unwrap(),
                modified
            );
        }
        println!(
            "source_os={source_os} destination_os={} formats={} all bindings and PCM playback passed",
            std::env::consts::OS,
            cases.as_array().unwrap().len()
        );
    }
}

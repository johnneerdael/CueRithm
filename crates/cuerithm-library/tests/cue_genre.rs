#![cfg(feature = "autotag")]
use cuerithm_cleanup::{CanonicalMapping, CleanupConfig, GenreRules};
use cuerithm_library::{Library, PreparationService, PreparationSettings, TrackQuery};
use cuerithm_media::MediaRuntime;
use onetagger_tag::{Field, Tag};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[test]
fn background_preparation_uses_the_genre_after_configured_cleanup() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("original.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &path,
    )
    .unwrap();
    let mut tag = Tag::load_file(&path, true).unwrap();
    tag.tag_mut()
        .set_field(Field::Genre, vec!["House".into()], true);
    tag.tag_mut().save_file(&path).unwrap();
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    let rt = MediaRuntime::from_bundle(&root).unwrap();
    let lib = Library::open(&d.path().join("app/db")).unwrap();
    let mut settings = PreparationSettings {
        stability_millis: 20,
        cleanup_enabled: true,
        cleanup: Some(CleanupConfig {
            genres: GenreRules {
                enabled: true,
                mappings: vec![CanonicalMapping {
                    from: "House".into(),
                    to: Some("Techno".into()),
                }],
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    settings.engine.lexicon_timing = true;
    settings.engine.source_profile_timing = false;
    settings.engine.lexicon_energy = true;
    settings.engine.energy_model = None;
    let revision = lib.save_settings(settings).unwrap();
    lib.enqueue(std::slice::from_ref(&path), revision).unwrap();
    let service = PreparationService::start_with_workers(lib.clone(), rt, 1).unwrap();
    let started = Instant::now();
    while lib.tracks(TrackQuery::default()).unwrap().total != 1 {
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "jobs: {:?}",
            lib.jobs().unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    service.shutdown();
    let id = &lib.jobs().unwrap()[0].track_id;
    let row = lib.track(id).unwrap();
    assert_eq!(row.genre, "Techno");
    let result = lib.analysis(id).unwrap().unwrap();
    assert_eq!(result.media.unwrap().cue_genre.as_deref(), Some("Techno"));
}

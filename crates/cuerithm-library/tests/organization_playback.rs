use cuerithm_library::{
    Library,
    organization::{OrganizationMode, OrganizationPhase, OrganizationRequest},
    playback::{CacheSource, Player},
};
use cuerithm_media::MediaRuntime;
use rodio::Source;
use std::{path::Path, sync::Arc, time::Duration};

#[test]
fn organization_keeps_loaded_pcm_position_and_track_identity() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().canonicalize().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let original = folder.join("source.flac");
    let destination = folder.join("organized.flac");
    std::fs::copy(
        root.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &original,
    )
    .unwrap();
    let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
    let job = library
        .enqueue(
            std::slice::from_ref(&original),
            library.settings().unwrap().revision,
        )
        .unwrap()
        .remove(0);
    let track_id = library
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.id == job)
        .unwrap()
        .track_id;
    let source_hash = library.source_hash(&track_id).unwrap();
    let audio = Arc::new(
        runtime
            .decode(&original, &temp.path().join("cache"))
            .unwrap(),
    );
    let pcm_hash = audio.pcm_sha256().unwrap();
    let mut source = CacheSource::open(audio.clone()).unwrap();
    source.try_seek(Duration::from_secs(10)).unwrap();
    let mut player = Player::with_output_factory(|| anyhow::bail!("No output needed")).unwrap();
    player.load(track_id.clone(), audio.clone()).unwrap();
    player.seek(10.).unwrap();

    let operation = library
        .plan_organization(OrganizationRequest {
            track_id: track_id.clone(),
            mode: OrganizationMode::Move,
            source_root: folder.clone(),
            destination_root: folder,
            files: vec![(original.clone(), destination.clone())],
        })
        .unwrap();
    assert_eq!(
        library.execute_organization(&operation).unwrap().phase,
        OrganizationPhase::AwaitingReferences
    );
    assert!(
        original.is_file(),
        "Original stays available until reference repair commits"
    );
    library
        .acknowledge_organization_references(&operation)
        .unwrap();
    assert_eq!(
        library.execute_organization(&operation).unwrap().phase,
        OrganizationPhase::Complete
    );

    assert!(!original.exists());
    assert_eq!(library.track_path(&track_id).unwrap(), destination);
    assert_eq!(library.source_hash(&track_id).unwrap(), source_hash);
    assert_eq!(audio.pcm_sha256().unwrap(), pcm_hash);
    assert_eq!(
        source.next(),
        Some(audio.read_frames(441000, 1).unwrap()[0])
    );
    assert_eq!(player.status().track_id.as_deref(), Some(track_id.as_str()));
    assert_eq!(player.status().position_seconds, 10.);
    assert!(
        !player.status().playing,
        "A path change never starts transport"
    );
    assert_eq!(
        audio.source_path, original,
        "Decode origin remains historical snapshot evidence"
    );
    assert!(audio.snapshot_path.is_file());
    player.seek(20.).unwrap();
    assert_eq!(player.status().position_seconds, 20.);
}

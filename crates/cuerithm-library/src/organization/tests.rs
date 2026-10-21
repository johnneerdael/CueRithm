use super::*;
#[test]
fn intent_captures_complete_audio_and_sidecar_scope_before_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let source = root.join("original.wav");
    std::fs::write(&source, b"audio with opaque metadata").unwrap();
    let library = Library::open(&root.join("library.sqlite")).unwrap();
    let job = library
        .enqueue(
            std::slice::from_ref(&source),
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
    let planned = library.plan_organization(OrganizationRequest {
        track_id,
        mode: OrganizationMode::Move,
        source_root: root.clone(),
        destination_root: root.clone(),
        files: vec![(source.clone(), root.join("renamed.wav"))],
    });
    assert!(
        planned.is_ok(),
        "Expected durable organization intent, got {planned:?}"
    );
    assert_eq!(
        std::fs::read(source).unwrap(),
        b"audio with opaque metadata"
    );
    assert!(!root.join("renamed.wav").exists());
}

include!("tests_additional.rs");
include!("tests_cancellation.rs");

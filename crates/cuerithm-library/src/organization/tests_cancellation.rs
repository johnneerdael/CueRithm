#[test]
fn staging_can_stop_after_a_partial_copy_and_recover_without_losing_original_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let library = Library::open(&root.join("data/library.sqlite")).unwrap();
    let source = root.join("original.wav");
    let destination = root.join("organized.wav");
    let bytes = vec![0x5a; 3 * 64 * 1024 + 17];
    std::fs::write(&source, &bytes).unwrap();
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
    let id = library
        .plan_organization(OrganizationRequest {
            track_id,
            mode: OrganizationMode::Move,
            source_root: root.clone(),
            destination_root: root.clone(),
            files: vec![(source.clone(), destination.clone())],
        })
        .unwrap();
    let record = library.organization_record(&id).unwrap();
    let stage = root.join(&record.files[0].stage_name);
    let mut reserved = false;
    let result = library.execute_organization_with(&id, |boundary| {
        if boundary == super::publish::Boundary::StageReserved {
            reserved = true;
        } else if reserved {
            let length = std::fs::metadata(&stage).unwrap().len();
            assert!(
                length > 0 && length < bytes.len() as u64,
                "Cancellation must reach a partially copied stage, got {length} bytes"
            );
            anyhow::bail!("Fixture cancellation during copy");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    assert!(!destination.exists());
    assert!(library.organization_record(&id).unwrap().error.is_some());
    let partial = std::fs::read(&stage).unwrap();
    assert_eq!(partial, bytes[..partial.len()]);

    library.recover_organization().unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    assert_eq!(std::fs::read(&destination).unwrap(), bytes);
    library.acknowledge_organization_references(&id).unwrap();
    assert_eq!(
        library.execute_organization(&id).unwrap().phase,
        OrganizationPhase::Complete
    );
    assert!(!source.exists());
    assert_eq!(std::fs::read(&destination).unwrap(), bytes);
    assert_eq!(
        std::fs::read(stage).unwrap(),
        partial,
        "Interrupted bytes stay retained for recovery"
    );
}

use cuerithm_dj_sync::{
    DjSoftware, SourcePathChange, SyncTarget, apply_path_repair, inspect_path_reference,
    prepare_path_repair, transaction::file_hash,
};
#[test]
fn virtualdj_reference_only_repair_preserves_external_metadata_cues_and_grid() {
    let dir = tempfile::tempdir().unwrap();
    let canonical = dir.path().canonicalize().unwrap();
    let source = canonical.join("old.wav");
    let destination = canonical.join("new.wav");
    std::fs::write(&source, b"invented fixture audio").unwrap();
    std::fs::copy(&source, &destination).unwrap();
    let database = canonical.join("database.xml");
    let old = source.to_str().unwrap();
    let new = destination.to_str().unwrap();
    let xml = format!(
        r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{old}" Custom="external"><Tags Author="Remote artist"/><Scan BeatGrid="remote grid"/><Poi Type="cue" Pos="99"/></Song><Song FilePath="/unrelated.wav"/></VirtualDJ_Database>"#
    );
    std::fs::write(&database, &xml).unwrap();
    let target = SyncTarget {
        software: DjSoftware::VirtualDj,
        path: database.clone(),
        source_root: None,
    };
    let snapshot = inspect_path_reference(&target, &source).unwrap();
    let change = SourcePathChange {
        track_id: "fixture".into(),
        from: source.clone(),
        to: destination.clone(),
        source_sha256: file_hash(&source).unwrap().unwrap(),
        approved_from_sha256: None,
    };
    let root = canonical.join("journals");
    let plan = prepare_path_repair(&root, &snapshot, &change).unwrap();
    assert_eq!(std::fs::read_to_string(&database).unwrap(), xml);
    apply_path_repair(&root, &plan.id).unwrap();
    let output = std::fs::read_to_string(database).unwrap();
    assert!(output.contains(new));
    assert!(!output.contains(old));
    for expected in [
        "external",
        "Remote artist",
        "remote grid",
        "99",
        "/unrelated.wav",
    ] {
        assert!(output.contains(expected));
    }
    assert_eq!(std::fs::read(source).unwrap(), b"invented fixture audio");
}

#[test]
fn sqlite_reference_repairs_preserve_track_ids_remote_metadata_and_performance_blobs() {
    for software in [DjSoftware::EngineDj, DjSoftware::Serato4] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.wav");
        let destination = root.join("new.wav");
        std::fs::write(&source, b"invented fixture audio").unwrap();
        std::fs::copy(&source, &destination).unwrap();
        let database = if software == DjSoftware::EngineDj {
            root.join("Engine Library/Database2/m.db")
        } else {
            root.join("Serato/Library/root.sqlite")
        };
        std::fs::create_dir_all(database.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(&database).unwrap();
        if software == DjSoftware::EngineDj {
            conn.execute_batch("CREATE TABLE Information(uuid TEXT,schemaVersionMajor INTEGER); INSERT INTO Information VALUES('native-db',3); CREATE TABLE Track(id INTEGER PRIMARY KEY,path TEXT,filename TEXT,title TEXT,cues BLOB,grid BLOB); INSERT INTO Track VALUES(73,'../old.wav','old.wav','Remote title',X'00FF1700',X'FF0019');").unwrap();
        } else {
            conn.execute_batch("PRAGMA user_version=1; CREATE TABLE master(revision INTEGER,last_sync_time INTEGER); INSERT INTO master VALUES(9,123); CREATE TABLE asset(id INTEGER PRIMARY KEY,portable_id TEXT,file_name TEXT,revision INTEGER,name TEXT,cues BLOB,grid BLOB);").unwrap();
            conn.execute(
                "INSERT INTO asset VALUES(73,?1,'old.wav',8,'Remote title',X'00FF1700',X'FF0019')",
                [source
                    .components()
                    .filter_map(|component| match component {
                        std::path::Component::Normal(value) => Some(value.to_str().unwrap()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("/")],
            )
            .unwrap();
        }
        drop(conn);
        let target = SyncTarget {
            software,
            path: database.clone(),
            source_root: None,
        };
        let snapshot = inspect_path_reference(&target, &source).unwrap();
        let change = SourcePathChange {
            track_id: "fixture".into(),
            from: source.clone(),
            to: destination.clone(),
            source_sha256: file_hash(&source).unwrap().unwrap(),
            approved_from_sha256: None,
        };
        let journals = root.join("journals");
        let plan = prepare_path_repair(&journals, &snapshot, &change).unwrap();
        apply_path_repair(&journals, &plan.id).unwrap();
        let conn = rusqlite::Connection::open(&database).unwrap();
        let query = if software == DjSoftware::EngineDj {
            "SELECT id,filename,title,cues,grid FROM Track"
        } else {
            "SELECT id,file_name,name,cues,grid FROM asset"
        };
        let row: (i64, String, String, Vec<u8>, Vec<u8>) = conn
            .query_row(query, [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap();
        assert_eq!(
            row,
            (
                73,
                "new.wav".into(),
                "Remote title".into(),
                vec![0, 255, 23, 0],
                vec![255, 0, 25]
            )
        );
        assert!(source.is_file());
        assert_eq!(
            file_hash(&destination).unwrap(),
            file_hash(&source).unwrap()
        );
    }
}
#[test]
fn changed_native_database_blocks_apply_without_overwriting_remote_edit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("old.wav");
    let destination = root.join("new.wav");
    std::fs::write(&source, b"invented fixture").unwrap();
    std::fs::copy(&source, &destination).unwrap();
    let database = root.join("database.xml");
    std::fs::write(
        &database,
        format!(
            r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{}"/></VirtualDJ_Database>"#,
            source.display()
        ),
    )
    .unwrap();
    let target = SyncTarget {
        software: DjSoftware::VirtualDj,
        path: database.clone(),
        source_root: None,
    };
    let snapshot = inspect_path_reference(&target, &source).unwrap();
    let change = SourcePathChange {
        track_id: "fixture".into(),
        from: source.clone(),
        to: destination,
        source_sha256: file_hash(&source).unwrap().unwrap(),
        approved_from_sha256: None,
    };
    let journals = root.join("journals");
    let plan = prepare_path_repair(&journals, &snapshot, &change).unwrap();
    let edited = std::fs::read_to_string(&database)
        .unwrap()
        .replace("<Song ", "<Song UserEdited=\"keep\" ");
    std::fs::write(&database, &edited).unwrap();
    assert!(apply_path_repair(&journals, &plan.id).is_err());
    assert_eq!(std::fs::read_to_string(database).unwrap(), edited);
    assert!(source.is_file());
}

#[test]
fn case_normalization_cannot_adopt_a_different_physical_source() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("case.wav");
    let other = root.join("CASE.wav");
    std::fs::write(&source, b"source A").unwrap();
    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&other)
    else {
        return;
    };
    use std::io::Write;
    file.write_all(b"source B").unwrap();
    drop(file);
    let database = root.join("database.xml");
    std::fs::write(
        &database,
        format!(
            r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{}"/></VirtualDJ_Database>"#,
            other.display()
        ),
    )
    .unwrap();
    let target = SyncTarget {
        software: DjSoftware::VirtualDj,
        path: database,
        source_root: None,
    };
    assert!(inspect_path_reference(&target, &source).is_err());
}

#[test]
fn interrupted_reference_publication_restores_only_its_own_known_bytes() {
    for external_edit in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("old.wav");
        let destination = root.join("new.wav");
        std::fs::write(&source, b"invented audio").unwrap();
        std::fs::copy(&source, &destination).unwrap();
        let database = root.join("database.xml");
        let before = format!(
            r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{}"><Poi Pos="37"/></Song></VirtualDJ_Database>"#,
            source.display()
        );
        std::fs::write(&database, &before).unwrap();
        let target = SyncTarget {
            software: DjSoftware::VirtualDj,
            path: database.clone(),
            source_root: None,
        };
        let snapshot = inspect_path_reference(&target, &source).unwrap();
        let change = SourcePathChange {
            track_id: "fixture".into(),
            from: source.clone(),
            to: destination.clone(),
            source_sha256: file_hash(&source).unwrap().unwrap(),
            approved_from_sha256: None,
        };
        let journals = root.join("journals");
        let plan = prepare_path_repair(&journals, &snapshot, &change).unwrap();
        let transaction =
            cuerithm_dj_sync::transaction::FileTransaction::open(&journals, &plan.id).unwrap();
        let before_hash = file_hash(&database).unwrap();
        let result = transaction.apply(|| {
            anyhow::ensure!(
                file_hash(&database)? == before_hash,
                "Simulated process stop after native publication"
            );
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(
            transaction.inspect().unwrap().state,
            cuerithm_dj_sync::transaction::JournalState::Applying
        );
        assert!(
            std::fs::read_to_string(&database)
                .unwrap()
                .contains(destination.to_str().unwrap())
        );
        if external_edit {
            let edited = std::fs::read_to_string(&database)
                .unwrap()
                .replace("37", "38");
            std::fs::write(&database, &edited).unwrap();
            assert!(cuerithm_dj_sync::restore_path_repair(&journals, &plan.id).is_err());
            assert_eq!(std::fs::read_to_string(&database).unwrap(), edited);
        } else {
            // A later explicit source decision may arrive after interruption.
            // It updates only the source-side proof of the existing journal.
            std::fs::write(&source, b"explicitly approved newer original").unwrap();
            let mut approved = change.clone();
            approved.approved_from_sha256 = file_hash(&source).unwrap();
            let mut wrong_destination = approved.clone();
            wrong_destination.source_sha256 = "different captured content".into();
            assert!(
                cuerithm_dj_sync::refresh_path_repair_source_proof(
                    &journals,
                    &plan.id,
                    &wrong_destination
                )
                .is_err()
            );
            cuerithm_dj_sync::refresh_path_repair_source_proof(&journals, &plan.id, &approved)
                .unwrap();
            cuerithm_dj_sync::restore_path_repair(&journals, &plan.id).unwrap();
            assert_eq!(std::fs::read_to_string(&database).unwrap(), before);
            // A second restore is safe after interruption of local acknowledgement.
            cuerithm_dj_sync::restore_path_repair(&journals, &plan.id).unwrap();
        }
        assert!(source.is_file());
        assert!(destination.is_file());
    }
}

#[test]
fn separately_approved_original_revision_never_changes_destination_content_proof() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = root.join("old.wav");
    let destination = root.join("new.wav");
    std::fs::write(&source, b"captured audio").unwrap();
    std::fs::copy(&source, &destination).unwrap();
    let captured = file_hash(&source).unwrap().unwrap();
    let database = root.join("database.xml");
    std::fs::write(
        &database,
        format!(
            r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{}"/></VirtualDJ_Database>"#,
            source.display()
        ),
    )
    .unwrap();
    let target = SyncTarget {
        software: DjSoftware::VirtualDj,
        path: database.clone(),
        source_root: None,
    };
    let snapshot = inspect_path_reference(&target, &source).unwrap();
    std::fs::write(&source, b"approved changed original").unwrap();
    let approved = file_hash(&source).unwrap().unwrap();
    // Deserialize to exercise compatibility of old journals and the new optional proof.
    let change: SourcePathChange = serde_json::from_value(serde_json::json!({
        "track_id":"fixture", "from":source, "to":destination,
        "source_sha256":captured, "approved_from_sha256":approved
    }))
    .unwrap();
    let journals = root.join("journals");
    let plan = prepare_path_repair(&journals, &snapshot, &change).unwrap();
    std::fs::write(&destination, b"wrong destination").unwrap();
    assert!(apply_path_repair(&journals, &plan.id).is_err());
    assert!(
        std::fs::read_to_string(&database)
            .unwrap()
            .contains(source.to_str().unwrap())
    );
    std::fs::write(&destination, b"captured audio").unwrap();
    apply_path_repair(&journals, &plan.id).unwrap();
    assert!(
        std::fs::read_to_string(&database)
            .unwrap()
            .contains(destination.to_str().unwrap())
    );
    assert_eq!(
        std::fs::read(&source).unwrap(),
        b"approved changed original"
    );
}

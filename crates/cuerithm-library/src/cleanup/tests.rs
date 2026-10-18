use super::*;
#[path = "../../tests/support/cleanup_fixture.rs"]
mod fixture;
use fixture::{Fixture, config};
fn prepare(f: &Fixture) -> (CleanupService, String, String) {
    let service = f.service();
    let preview = service
        .preview_cleanup(CleanupScope::AllReady, config())
        .unwrap();
    let plan: String = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT payload_json FROM cleanup_plans WHERE id=?1",
            [&preview.id],
            |r| r.get(0),
        )
        .unwrap();
    let plan: Plan = serde_json::from_str(&plan).unwrap();
    let batch = uuid::Uuid::new_v4().to_string();
    let result = CleanupBatchResult {
        id: batch.clone(),
        ..Default::default()
    };
    f.lib
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO cleanup_batches VALUES(?1,?2,NULL,'apply','running',?3,?4)",
            params![
                batch,
                preview.id,
                serde_json::to_string(&result).unwrap(),
                now()
            ],
        )
        .unwrap();
    let write = service.stage_proposal(&batch, &plan.proposals[0]).unwrap();
    (service, write, batch)
}
#[test]
fn restart_rolls_back_before_copy_after_copy_and_after_atomic_swap() {
    for boundary in 0..3 {
        let f = Fixture::new(1);
        let original = std::fs::read(&f.paths[0]).unwrap();
        let (service, write, batch) = prepare(&f);
        let journal = journal::load(&f.lib, &write).unwrap();
        let file = &journal.files[0];
        if boundary > 0 {
            crate::publish_files::copy_prepared(&file.staged, &file.publish, &file.after).unwrap();
        }
        if boundary > 1 {
            crate::publish_files::replace(&file.publish, &file.target, &file.displaced).unwrap();
        }
        let Fixture {
            dir,
            db,
            lib,
            rt: _,
            ids,
            paths,
        } = f;
        drop(service);
        drop(lib);
        let lib = Library::open(&db).unwrap();
        assert_eq!(
            std::fs::read(&paths[0]).unwrap(),
            original,
            "boundary {boundary}"
        );
        assert_eq!(lib.track(&ids[0]).unwrap().title, "signal__track 0");
        assert_eq!(lib.cue_revision_count(&ids[0]).unwrap(), 1);
        let (state, clean): (String, bool) = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT state,cleanup_complete FROM cleanup_writes WHERE id=?1",
                [write],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, "rolled_back");
        assert!(clean);
        let state: String = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT state FROM cleanup_batches WHERE id=?1",
                [batch],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "interrupted");
        drop(dir);
    }
}
#[test]
fn recovery_preserves_later_file_edit_and_keeps_recovery_original() {
    let f = Fixture::new(1);
    let (service, write, _) = prepare(&f);
    let journal = journal::load(&f.lib, &write).unwrap();
    let file = &journal.files[0];
    crate::publish_files::copy_prepared(&file.staged, &file.publish, &file.after).unwrap();
    crate::publish_files::replace(&file.publish, &file.target, &file.displaced).unwrap();
    std::fs::write(&f.paths[0], b"later user edit").unwrap();
    let backup = file.backup.clone().unwrap();
    let Fixture {
        dir,
        db,
        lib,
        rt: _,
        ids: _,
        paths,
    } = f;
    drop(service);
    drop(lib);
    let lib = Library::open(&db).unwrap();
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"later user edit");
    assert!(backup.is_file());
    let state: String = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT state FROM cleanup_writes WHERE id=?1",
            [write],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "conflict");
    drop(dir);
}
#[test]
fn one_pending_cleanup_reserves_its_destination_and_watch_events() {
    let f = Fixture::new(1);
    let (service, write, _) = prepare(&f);
    let other = service
        .preview_cleanup(CleanupScope::AllReady, config())
        .unwrap();
    let result = service
        .apply_cleanup(&other.id, &[other.proposals[0].id.clone()])
        .unwrap();
    assert!(result.applied.is_empty());
    assert_eq!(result.failed.len(), 1);
    let j = journal::load(&f.lib, &write).unwrap();
    let file = &j.files[0];
    crate::publish_files::copy_prepared(&file.staged, &file.publish, &file.after).unwrap();
    crate::publish_files::replace(&file.publish, &file.target, &file.displaced).unwrap();
    f.lib
        .enqueue_discovered(&[f.paths[0].clone()], f.lib.settings().unwrap().revision)
        .unwrap();
    assert_eq!(f.lib.track(&f.ids[0]).unwrap().state, "ready");
    assert_eq!(f.lib.jobs().unwrap().len(), 1);
}
#[test]
fn backup_storage_counts_committed_metadata_originals() {
    let f = Fixture::new(1);
    let service = f.service();
    let p = service
        .preview_cleanup(CleanupScope::AllReady, config())
        .unwrap();
    service
        .apply_cleanup(&p.id, &[p.proposals[0].id.clone()])
        .unwrap();
    let storage = f.lib.tagging_storage().unwrap();
    assert_eq!(storage.original_files, 1);
    assert!(storage.original_bytes > 0);
}

#[test]
fn interrupted_undo_restores_the_applied_version_and_can_retry_undo() {
    let f = Fixture::new(1);
    let original_bytes = std::fs::read(&f.paths[0]).unwrap();
    let service = f.service();
    let preview = service
        .preview_cleanup(CleanupScope::AllReady, config())
        .unwrap();
    let batch = service
        .apply_cleanup(&preview.id, &[preview.proposals[0].id.clone()])
        .unwrap();
    let applied_bytes = std::fs::read(&f.paths[0]).unwrap();
    let write: String = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT id FROM cleanup_writes WHERE batch_id=?1",
            [&batch.id],
            |r| r.get(0),
        )
        .unwrap();
    let original = journal::load(&f.lib, &write).unwrap();
    let before = service.undo_snapshot(&original).unwrap();
    let undo = uuid::Uuid::new_v4().to_string();
    let result = CleanupBatchResult {
        id: undo.clone(),
        ..Default::default()
    };
    f.lib
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO cleanup_batches VALUES(?1,NULL,?2,'undo','running',?3,?4)",
            params![
                undo,
                batch.id,
                serde_json::to_string(&result).unwrap(),
                now()
            ],
        )
        .unwrap();
    let undo_write = service
        .stage_undo(&undo, &write, &original, &before)
        .unwrap();
    let journal = journal::load(&f.lib, &undo_write).unwrap();
    let file = &journal.files[0];
    crate::publish_files::copy_prepared(&file.staged, &file.publish, &file.after).unwrap();
    crate::publish_files::replace(&file.publish, &file.target, &file.displaced).unwrap();
    let Fixture {
        dir,
        db,
        lib,
        rt,
        ids,
        paths,
    } = f;
    drop(service);
    drop(lib);
    let lib = Library::open(&db).unwrap();
    assert_eq!(std::fs::read(&paths[0]).unwrap(), applied_bytes);
    assert_eq!(lib.track(&ids[0]).unwrap().title, "signal track 0");
    assert_eq!(journal::load(&lib, &write).unwrap().state, "committed");
    let service = CleanupService::new(lib.clone(), rt);
    let retry = service.undo_cleanup(&batch.id).unwrap();
    assert_eq!(retry.applied, ids);
    assert_eq!(std::fs::read(&paths[0]).unwrap(), original_bytes);
    drop(service);
    drop(lib);
    drop(dir);
}
#[test]
fn committed_publication_finishes_pending_temporary_cleanup_after_restart() {
    let f = Fixture::new(1);
    let (service, write, batch) = prepare(&f);
    journal::publish(&f.lib, &write).unwrap();
    let j = journal::load(&f.lib, &write).unwrap();
    let file = &j.files[0];
    std::fs::copy(&file.target, &file.staged).unwrap();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE cleanup_writes SET cleanup_complete=0 WHERE id=?1",
            [&write],
        )
        .unwrap();
    let hash = file_sha256(&file.target).unwrap();
    let Fixture {
        dir,
        db,
        lib,
        rt: _,
        ids,
        paths,
    } = f;
    drop(service);
    drop(lib);
    let lib = Library::open(&db).unwrap();
    assert_eq!(file_sha256(&paths[0]).unwrap(), hash);
    assert_eq!(lib.track(&ids[0]).unwrap().title, "signal track 0");
    assert_eq!(lib.cue_revision_count(&ids[0]).unwrap(), 1);
    assert!(!file.staged.exists());
    let clean: bool = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT cleanup_complete FROM cleanup_writes WHERE id=?1",
            [&write],
            |r| r.get(0),
        )
        .unwrap();
    assert!(clean);
    let state: String = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT state FROM cleanup_batches WHERE id=?1",
            [batch],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "interrupted");
    drop(lib);
    drop(dir);
}
#[test]
fn custom_tag_only_conversion_keeps_file_bytes_and_undo_preserves_prior_categories() {
    use cuerithm_cleanup::{GenreConversion, GenreRules};
    let f = Fixture::new(1);
    let service = f.service();
    let info = f.rt.probe(&f.paths[0]).unwrap();
    let metadata = cuerithm_autotag::read_metadata(&f.paths[0], &info).unwrap();
    let genres = metadata.fields[&cuerithm_cleanup::MetadataField::Genre].clone();
    f.lib.conn().unwrap().execute_batch("INSERT INTO custom_tag_categories VALUES('mood','Mood');INSERT INTO custom_tags VALUES('peak','mood','Peak');").unwrap();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO track_custom_tags VALUES(?1,'peak')",
            [&f.ids[0]],
        )
        .unwrap();
    let bytes = std::fs::read(&f.paths[0]).unwrap();
    let config = CleanupConfig {
        genres: GenreRules {
            enabled: true,
            conversion: Some(GenreConversion {
                category: "Source genre".into(),
                genres: genres.clone().into_iter().collect(),
                remove_genre: false,
            }),
            ..Default::default()
        },
        ..Default::default()
    };
    let preview = service
        .preview_cleanup(CleanupScope::AllReady, config)
        .unwrap();
    assert!(preview.proposals[0].report.changes.is_empty());
    let applied = service
        .apply_cleanup(&preview.id, &[preview.proposals[0].id.clone()])
        .unwrap();
    assert_eq!(applied.applied.len(), 1);
    assert_eq!(std::fs::read(&f.paths[0]).unwrap(), bytes);
    assert_eq!(f.lib.tagging_storage().unwrap().original_files, 0);
    assert_eq!(
        custom_tags(&f.lib.conn().unwrap(), &f.ids[0])
            .unwrap()
            .len(),
        genres.len() + 1
    );
    assert_eq!(service.undo_cleanup(&applied.id).unwrap().applied.len(), 1);
    assert_eq!(
        custom_tags(&f.lib.conn().unwrap(), &f.ids[0]).unwrap(),
        vec![cuerithm_cleanup::CustomTag {
            category: "Mood".into(),
            value: "Peak".into()
        }]
    );
    assert_eq!(std::fs::read(&f.paths[0]).unwrap(), bytes);
    assert_eq!(f.lib.cue_revision_count(&f.ids[0]).unwrap(), 1);
}

#[test]
fn custom_tag_and_metadata_edits_invalidate_preview_and_undo() {
    let f = Fixture::new(1);
    let service = f.service();
    let preview = service
        .preview_cleanup(CleanupScope::AllReady, config())
        .unwrap();
    let before = file_sha256(&f.paths[0]).unwrap();
    f.lib.conn().unwrap().execute_batch("INSERT INTO custom_tag_categories VALUES('mood','Mood');INSERT INTO custom_tags VALUES('peak','mood','Peak');").unwrap();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO track_custom_tags VALUES(?1,'peak')",
            [&f.ids[0]],
        )
        .unwrap();
    let result = service
        .apply_cleanup(&preview.id, &[preview.proposals[0].id.clone()])
        .unwrap();
    assert_eq!(result.conflicted.len(), 1);
    assert_eq!(file_sha256(&f.paths[0]).unwrap(), before);
    let preview = service
        .preview_cleanup(CleanupScope::AllReady, config())
        .unwrap();
    let applied = service
        .apply_cleanup(&preview.id, &[preview.proposals[0].id.clone()])
        .unwrap();
    let hash = file_sha256(&f.paths[0]).unwrap();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET title='Later library edit' WHERE id=?1",
            [&f.ids[0]],
        )
        .unwrap();
    let undo = service.undo_cleanup(&applied.id).unwrap();
    assert_eq!(undo.conflicted.len(), 1);
    assert_eq!(file_sha256(&f.paths[0]).unwrap(), hash);
    assert_eq!(f.lib.track(&f.ids[0]).unwrap().title, "Later library edit");
}
#[test]
fn damaged_backup_rejects_undo_and_invalid_selections_do_not_write() {
    let f = Fixture::new(1);
    let service = f.service();
    let preview = service
        .preview_cleanup(CleanupScope::AllReady, config())
        .unwrap();
    let hash = file_sha256(&f.paths[0]).unwrap();
    assert!(
        service
            .apply_cleanup(&preview.id, &["not-a-proposal".into()])
            .is_err()
    );
    assert!(
        service
            .apply_cleanup(
                &preview.id,
                &[
                    preview.proposals[0].id.clone(),
                    preview.proposals[0].id.clone()
                ]
            )
            .is_err()
    );
    assert_eq!(file_sha256(&f.paths[0]).unwrap(), hash);
    assert!(service.cleanup_history().unwrap().is_empty());
    let applied = service
        .apply_cleanup(&preview.id, &[preview.proposals[0].id.clone()])
        .unwrap();
    let write: String = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT id FROM cleanup_writes WHERE batch_id=?1",
            [&applied.id],
            |r| r.get(0),
        )
        .unwrap();
    let journal = journal::load(&f.lib, &write).unwrap();
    std::fs::write(journal.files[0].backup.as_ref().unwrap(), b"damaged backup").unwrap();
    let after = file_sha256(&f.paths[0]).unwrap();
    let undo = service.undo_cleanup(&applied.id).unwrap();
    assert_eq!(undo.failed.len(), 1);
    assert!(undo.applied.is_empty());
    assert_eq!(file_sha256(&f.paths[0]).unwrap(), after);
}

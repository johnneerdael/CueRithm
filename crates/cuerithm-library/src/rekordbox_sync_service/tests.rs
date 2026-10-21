use super::*;
use std::cell::Cell;
mod native_acceptance;
mod native_reorder_ack;
const KEY: &[u8] = b"authored native coordinator fixture";
fn key() -> DatabaseKey {
    DatabaseKey::new(KEY).unwrap()
}
fn open(path: &Path) -> Connection {
    let c = Connection::open(path).unwrap();
    assert_eq!(
        unsafe { rusqlite::ffi::sqlite3_key(c.handle(), KEY.as_ptr().cast(), KEY.len() as i32) },
        rusqlite::ffi::SQLITE_OK
    );
    c.pragma_update(None, "cipher_compatibility", 4).unwrap();
    c
}
fn native_fixture(path: &Path) {
    let c = open(path);
    c.execute_batch("CREATE TABLE djmdProperty(DBID TEXT,DeviceID TEXT,DBVersion TEXT); INSERT INTO djmdProperty VALUES('authored','device','7'); CREATE TABLE agentRegistry(registry_id TEXT,int_1 INTEGER); INSERT INTO agentRegistry VALUES('localUpdateCount',1); CREATE TABLE djmdContent(ID TEXT PRIMARY KEY,UUID TEXT,MasterSongID TEXT,Title TEXT,FolderPath TEXT,ServiceID INTEGER,rb_local_deleted INTEGER,AnalysisDataPath TEXT,Opaque BLOB); INSERT INTO djmdContent VALUES('1','uuid','master','Before','/missing/authored.flac',0,0,'/old.DAT',X'00FF'); CREATE TABLE unknownRows(id TEXT,payload BLOB); INSERT INTO unknownRows VALUES('external',X'BEEF');").unwrap();
    for t in [
        "djmdArtist",
        "djmdAlbum",
        "djmdGenre",
        "djmdLabel",
        "djmdKey",
        "djmdCue",
        "contentCue",
        "djmdPlaylist",
        "djmdSongPlaylist",
        "djmdMyTag",
        "djmdSongMyTag",
    ] {
        c.execute_batch(&format!("CREATE TABLE {t}(ID TEXT PRIMARY KEY)"))
            .unwrap();
    }
}
struct Fixture {
    _dir: tempfile::TempDir,
    library: Library,
    native: PathBuf,
    old_asset: PathBuf,
    new_asset: PathBuf,
    id: String,
}
fn fixture(change: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    let library = Library::open(&home.join("library.db")).unwrap();
    let native = home.join("master.db");
    native_fixture(&native);
    let share = home.join("share");
    fs::create_dir(&share).unwrap();
    let old_asset = share.join("old.DAT");
    fs::write(&old_asset, b"original complete resource").unwrap();
    let new_asset = share.join("versioned/new.DAT");
    let cancel = CancellationToken::default();
    let collection = ReadOnlyDatabase::open(&native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let observed = library
        .import_rekordbox_collection(
            &collection,
            NativeImportOptions {
                settings_revision: library.settings().unwrap().revision,
                content_ids: None,
                initial_owners: BTreeMap::new(),
            },
            &cancel,
        )
        .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let workspace = root(&library).join("operations").join(&id);
    fs::create_dir_all(&workspace).unwrap();
    let row = &collection.records["djmdContent"][0];
    let plan = NativeWritePlan {
        source: native.clone(),
        schema_fingerprint: collection.schema_fingerprint.clone(),
        rows: if change {
            vec![NativeRowMutation {
                table: "djmdContent".into(),
                id: "1".into(),
                expected_fingerprint: Some(row.fingerprint.clone()),
                values: BTreeMap::from([
                    ("Title".into(), TargetValue::Text("After".into())),
                    (
                        "AnalysisDataPath".into(),
                        TargetValue::Text("/versioned/new.DAT".into()),
                    ),
                ]),
            }]
        } else {
            vec![]
        },
    };
    let before = {
        let lease = NativeDatabaseLease::acquire(&native, &key(), &cancel).unwrap();
        lease.state(&cancel).unwrap()
    };
    let staged = stage_native_write(&plan, &workspace, &key(), &cancel).unwrap();
    let after = {
        let lease = NativeDatabaseLease::acquire(&staged.snapshot.path, &key(), &cancel).unwrap();
        lease.state(&cancel).unwrap()
    };
    let files = FileTransaction::create(&root(&library).join("assets")).unwrap();
    if change {
        let bytes = workspace.join("new.DAT");
        fs::write(&bytes, b"new immutable complete resource").unwrap();
        assets::add_publications(
            &files,
            &[FilePublication {
                target: new_asset.clone(),
                before_sha256: None,
                staged: Some(bytes),
            }],
        )
        .unwrap();
    }
    let target = RekordboxSyncTarget {
        id: "discovered".into(),
        installation: TargetInstallation {
            root: home.join("Authored.app"),
            executable: home.join("Authored.app/rekordbox"),
            version: "7.fixture".into(),
            major: 7,
            version_source: "authored".into(),
        },
        database: native.clone(),
        options_file: home.join("options.json"),
        asset_root: share,
    };
    let request = RekordboxSyncRequest {
        target_id: target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options: RekordboxSyncOptions::default(),
        decisions: BTreeMap::new(),
    };
    let preview = RekordboxSyncPreview {
        id: id.clone(),
        target_id: target.id.clone(),
        library_revision: library.revision().unwrap(),
        fields: vec![],
        entity_changes: vec![],
        pending_entities: vec![],
        tracks: 0,
        changed_rows: staged.changed_rows,
        skipped_fields: 2,
        warnings: vec![],
        can_apply: true,
        media_copies: 0,
        copied_audio_bytes: 0,
    };
    let ledger_before = ledger::capture(&library.conn().unwrap(), &observed.target_id).unwrap();
    let op = Operation {
        version: 1,
        id: id.clone(),
        target,
        native_target_id: observed.target_id,
        native_revision: observed.revision,
        request,
        revision: library.revision().unwrap(),
        settings_revision: library.settings().unwrap().revision,
        sources: vec![],
        asset_sources: vec![AssetSource {
            path: old_asset.clone(),
            hash: file_sha256_cancellable(&old_asset, &cancel).unwrap(),
        }],
        before,
        after,
        plan,
        resolved: vec![],
        playlist_plan: None,
        receipts: vec![],
        acknowledgements: vec![],
        preview,
        asset_journal_id: files.inspect().unwrap().id,
        before_snapshot: None,
        before_snapshot_hash: None,
        ledger_before,
        ledger_after_hash: None,
        status: "prepared".into(),
        created: 1,
        updated: 1,
        outcome: None,
    };
    save(&library.conn().unwrap(), &op).unwrap();
    Fixture {
        _dir: dir,
        library,
        native,
        old_asset,
        new_asset,
        id,
    }
}
fn status(library: &Library, id: &str) -> Result<String> {
    let c = Connection::open(library.data_dir().join("library.db"))?;
    Ok(c.query_row(
        "SELECT status FROM rekordbox_sync_operations WHERE id=?1",
        [id],
        |r| r.get(0),
    )?)
}
fn title(f: &Fixture) -> String {
    open(&f.native)
        .query_row("SELECT Title FROM djmdContent", [], |r| r.get(0))
        .unwrap()
}
fn native_hash(f: &Fixture) -> String {
    let c = CancellationToken::default();
    NativeDatabaseLease::acquire(&f.native, &key(), &c)
        .unwrap()
        .state(&c)
        .unwrap()
        .logical_sha256
}
#[test]
fn publishes_database_last_preserves_old_assets_and_restores_on_original_inode() {
    let f = fixture(true);
    let old = fs::read(&f.old_asset).unwrap();
    let before = native_hash(&f);
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(&f.native).unwrap().ino()
    };
    let result = publication::fixture_apply(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.status, "applied");
    assert_eq!(result.changed_rows, 1);
    assert_eq!(result.skipped_fields, 2);
    assert_eq!(title(&f), "After");
    assert!(f.new_asset.exists());
    assert_eq!(fs::read(&f.old_asset).unwrap(), old);
    assert_eq!(
        open(&f.native)
            .query_row("SELECT payload FROM unknownRows", [], |r| r
                .get::<_, Vec<u8>>(0))
            .unwrap(),
        vec![190, 239]
    );
    let restored = publication::fixture_restore(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(restored.status, "restored");
    assert_eq!(native_hash(&f), before);
    assert!(!f.new_asset.exists());
    assert_eq!(fs::read(&f.old_asset).unwrap(), old);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(fs::metadata(&f.native).unwrap().ino(), inode);
    }
}
#[test]
fn process_appearing_during_assets_never_changes_database_and_restart_rolls_back() {
    let mut f = fixture(true);
    let before = native_hash(&f);
    let count = Cell::new(0);
    let result = publication::fixture_apply(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| {
            count.set(count.get() + 1);
            ensure!(count.get() < 3, "Authored native agent appeared");
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(native_hash(&f), before);
    assert_eq!(title(&f), "Before");
    assert_eq!(load(&f.library, &f.id).unwrap().status, "publishing");
    let db = f.library.data_dir().join("library.db");
    // The library's data_dir is the parent of its database, not necessarily its filename.
    let original = f._dir.path().canonicalize().unwrap().join("library.db");
    let _ = db;
    drop(f.library);
    f.library = Library::open(&original).unwrap();
    let result = publication::fixture_recover(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.status, "rolled_back");
    assert_eq!(native_hash(&f), before);
    assert!(!f.new_asset.exists());
}
#[test]
fn interruption_after_sql_commit_recovers_verified_publication_idempotently() {
    let f = fixture(true);
    let error = publication::fixture_apply(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| {
            ensure!(
                status(&f.library, &f.id)? != "native_published",
                "Authored crash after native commit"
            );
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("after native commit"));
    assert_eq!(title(&f), "After");
    assert!(f.new_asset.exists());
    let result = publication::fixture_recover(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.status, "applied");
    let saved = load(&f.library, &f.id).unwrap();
    assert!(saved.ledger_after_hash.is_some());
    assert_eq!(
        f.library.history_rekordbox_sync().unwrap()[0].status,
        "applied"
    );
    assert!(f.library.discard_rekordbox_sync(&f.id).is_err());
}
#[test]
fn unknown_rows_including_wal_edits_invalidate_frozen_native_proof() {
    let f = fixture(true);
    let c = open(&f.native);
    c.pragma_update(None, "journal_mode", "WAL").unwrap();
    c.execute("UPDATE unknownRows SET payload=X'1234'", [])
        .unwrap();
    c.set_db_config(
        rusqlite::config::DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE,
        true,
    )
    .unwrap();
    drop(c);
    let result = publication::fixture_apply(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("changed after preview")
    );
    assert!(!f.new_asset.exists());
    assert_eq!(title(&f), "Before");
}
#[test]
fn restore_refuses_valid_before_image_changed_after_file_hash_without_mutating_native_or_ledger() {
    let f = fixture(true);
    let cancel = CancellationToken::default();
    publication::fixture_apply(&f.library, &f.id, &key(), &cancel, &|| Ok(())).unwrap();
    let op = load(&f.library, &f.id).unwrap();
    let snapshot = op.before_snapshot.as_ref().unwrap();
    let ledger_before =
        digest(&ledger::capture(&f.library.conn().unwrap(), &op.native_target_id).unwrap())
            .unwrap();
    let new_bytes = fs::read(&f.new_asset).unwrap();
    let old_bytes = fs::read(&f.old_asset).unwrap();
    let calls = Cell::new(0);
    let error = publication::fixture_restore(&f.library, &f.id, &key(), &cancel, &|| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            // The process guard immediately before the SDK call runs after the
            // ordinary file hash. Change a valid encrypted source at that boundary.
            open(snapshot).execute("UPDATE unknownRows SET payload=X'BAD0'", [])?;
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("before-image"), "{error:#}");
    assert_eq!(calls.get(), 2);
    assert_eq!(native_hash(&f), op.after.logical_sha256);
    assert_eq!(fs::read(&f.new_asset).unwrap(), new_bytes);
    assert_eq!(fs::read(&f.old_asset).unwrap(), old_bytes);
    assert_eq!(
        digest(&ledger::capture(&f.library.conn().unwrap(), &op.native_target_id).unwrap())
            .unwrap(),
        ledger_before
    );
}

#[test]
fn later_native_edits_or_original_asset_edits_refuse_restore() {
    for edit_asset in [false, true] {
        let f = fixture(true);
        publication::fixture_apply(
            &f.library,
            &f.id,
            &key(),
            &CancellationToken::default(),
            &|| Ok(()),
        )
        .unwrap();
        if edit_asset {
            fs::write(&f.old_asset, b"external asset change").unwrap();
        } else {
            open(&f.native)
                .execute("UPDATE unknownRows SET payload=X'1234'", [])
                .unwrap();
        }
        assert!(
            publication::fixture_restore(
                &f.library,
                &f.id,
                &key(),
                &CancellationToken::default(),
                &|| Ok(())
            )
            .is_err()
        );
        assert!(f.new_asset.exists());
        assert_eq!(title(&f), "After");
    }
}
#[test]
fn no_op_changes_no_native_rows_and_settles_without_rewriting_resources() {
    let f = fixture(false);
    let before = native_hash(&f);
    let bytes = fs::read(&f.native).unwrap();
    let result = publication::fixture_apply(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.changed_rows, 0);
    assert_eq!(native_hash(&f), before);
    assert_eq!(fs::read(&f.native).unwrap(), bytes);
    assert!(!f.new_asset.exists());
}
#[test]
fn forged_paths_and_null_or_clear_choices_cannot_authorize_implicit_deletion() {
    let local = NativeFieldObservation::Present(Value::Null);
    let remote = NativeFieldObservation::Present(json!("existing"));
    let result = NativeReconciliation::Conflict {
        reason: NativeConflict::InitialAcknowledgement,
        suggestion: None,
    };
    assert!(
        planning::field_choice(
            &local,
            &remote,
            &result,
            Some(NativeFieldDecision::UseLocal)
        )
        .is_err()
    );
    assert!(
        planning::field_choice(
            &local,
            &remote,
            &result,
            Some(NativeFieldDecision::ClearLocal)
        )
        .is_err()
    );
    assert!(
        planning::field_choice(
            &local,
            &remote,
            &result,
            Some(NativeFieldDecision::AcceptEqual)
        )
        .is_err()
    );
    assert_eq!(
        planning::field_choice(
            &local,
            &remote,
            &NativeReconciliation::Proposed {
                decision: NativeFieldDecision::UseLocal
            },
            None
        )
        .unwrap(),
        None,
        "An automatic source proposal never turns observed null into a target clear"
    );
    assert!(
        planning::field_choice(
            &NativeFieldObservation::Unavailable,
            &remote,
            &result,
            Some(NativeFieldDecision::UseRemote)
        )
        .is_err()
    );
    assert_eq!(
        planning::field_choice(&local, &remote, &result, None).unwrap(),
        None
    );
    assert_eq!(
        planning::field_choice(
            &local,
            &remote,
            &result,
            Some(NativeFieldDecision::ClearRemote)
        )
        .unwrap(),
        Some(NativeFieldDecision::ClearRemote)
    );
}

fn add_local(f: &Fixture, title: &str) -> String {
    let source = f._dir.path().canonicalize().unwrap().join("authored.wav");
    fs::write(
        &source,
        b"authored audio identity, metadata-only coordinator test",
    )
    .unwrap();
    let media = cuerithm_media::MediaInfo {
        codec: "pcm_s16le".into(),
        sample_rate: 48000,
        channels: 2,
        bits_per_sample: Some(16),
        tags: BTreeMap::new(),
        properties: BTreeMap::new(),
    };
    let analysis = json!({"source_sample_rate":48000,"publication":{"grid":{"bpm":120.0,"startTime":0.0},"cues":[],"implementation_sha256":"authored","inputs_sha256":"authored"}});
    f.library.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,analysis_json,metadata_json,musical_key,bpm,duration,created,updated) VALUES('local',?1,?2,?3,'ready',?4,?5,'8A',120,120,1,1)",params![crate::model::path_bytes(&source),title,file_hash(&source).unwrap().unwrap(),analysis.to_string(),serde_json::to_string(&media).unwrap()]).unwrap();
    open(&f.native)
        .execute(
            "UPDATE djmdContent SET FolderPath=?1",
            [source.to_str().unwrap()],
        )
        .unwrap();
    for (column, kind) in [
        ("rb_local_usn", "INTEGER"),
        ("rb_data_status", "INTEGER"),
        ("updated_at", "TEXT"),
        ("TrackInfoUpdated", "TEXT"),
    ] {
        open(&f.native)
            .execute_batch(&format!(
                "ALTER TABLE djmdContent ADD COLUMN {column} {kind}"
            ))
            .unwrap();
    }
    "local".into()
}
fn metadata_preview(f: &Fixture, decision: Option<NativeFieldDecision>) -> RekordboxSyncPreview {
    // Drop the manually prepared operation, keeping only the observed identity.
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::from(["title".into()]),
        cues: false,
        beat_grid: false,
        artwork: false,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let decisions = decision
        .map(|d| BTreeMap::from([("local".into(), BTreeMap::from([("title".into(), d)]))]))
        .unwrap_or_default();
    planning::preview(
        &f.library,
        RekordboxSyncRequest {
            target_id: prior.target.id.clone(),
            scope: RekordboxSyncScope::Full,
            options,
            decisions,
        },
        prior.target,
        &key(),
        None,
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap()
}
#[test]
fn initial_equal_field_requires_review_and_accept_equal_only_acknowledges_after_apply() {
    let f = fixture(false);
    add_local(&f, "Before");
    let preview = metadata_preview(&f, None);
    assert_eq!(preview.fields.len(), 1);
    assert_eq!(preview.fields[0].decision, None);
    assert_eq!(preview.skipped_fields, 1);
    assert_eq!(preview.changed_rows, 0);
    let op = load(&f.library, &preview.id).unwrap();
    assert!(
        !f.library
            .native_track_link(&op.native_target_id, "1")
            .unwrap()
            .fields["title"]
            .acknowledged
    );
    f.library.discard_rekordbox_sync(&preview.id).unwrap();
    let mut request = op.request;
    request.decisions = BTreeMap::from([(
        "local".into(),
        BTreeMap::from([("title".into(), NativeFieldDecision::AcceptEqual)]),
    )]);
    let preview = planning::preview(
        &f.library,
        request,
        op.target,
        &key(),
        None,
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(preview.changed_rows, 0);
    let result = publication::fixture_apply(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.acknowledged_fields, 1);
    let link = f
        .library
        .native_track_link(&op.native_target_id, "1")
        .unwrap();
    assert!(link.fields["title"].acknowledged);
}
#[test]
fn use_remote_acknowledges_the_frozen_pair_without_overwriting_local_music() {
    let f = fixture(false);
    add_local(&f, "Local draft");
    let preview = metadata_preview(&f, Some(NativeFieldDecision::UseRemote));
    assert_eq!(preview.changed_rows, 0);
    publication::fixture_apply(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    let op = load(&f.library, &preview.id).unwrap();
    let link = f
        .library
        .native_track_link(&op.native_target_id, "1")
        .unwrap();
    assert_eq!(link.fields["title"].local, json!(["Local draft"]));
    assert_eq!(link.fields["title"].remote, json!(["Before"]));
    assert!(link.fields["title"].acknowledged);
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row("SELECT title FROM tracks WHERE id='local'", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "Local draft"
    );
    assert_eq!(title(&f), "Before");
}
#[test]
fn local_edits_after_native_commit_cannot_be_falsely_acknowledged_and_restore_preserves_them() {
    let f = fixture(false);
    add_local(&f, "Local published");
    let preview = metadata_preview(&f, Some(NativeFieldDecision::UseLocal));
    assert!(preview.changed_rows > 0);
    let edit = Cell::new(false);
    let result = publication::fixture_apply(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| {
            if status(&f.library, &preview.id)? == "native_published" && !edit.get() {
                edit.set(true);
                f.library.conn()?.execute(
                    "UPDATE tracks SET title='Later local edit' WHERE id='local'",
                    [],
                )?;
            }
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(title(&f), "Local published");
    assert!(
        publication::fixture_recover(
            &f.library,
            &preview.id,
            &key(),
            &CancellationToken::default(),
            &|| Ok(())
        )
        .is_err()
    );
    publication::fixture_restore(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(title(&f), "Before");
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row("SELECT title FROM tracks WHERE id='local'", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "Later local edit"
    );
}
#[test]
fn local_epoch_and_source_byte_edits_reject_before_any_native_asset_publication() {
    for bytes in [false, true] {
        let f = fixture(false);
        add_local(&f, "Local published");
        let preview = metadata_preview(&f, Some(NativeFieldDecision::UseLocal));
        let op = load(&f.library, &preview.id).unwrap();
        if bytes {
            fs::write(&op.sources[0].path, b"later source bytes").unwrap();
        } else {
            f.library
                .conn()
                .unwrap()
                .execute("UPDATE tracks SET title='Later' WHERE id='local'", [])
                .unwrap();
        }
        assert!(
            publication::fixture_apply(
                &f.library,
                &preview.id,
                &key(),
                &CancellationToken::default(),
                &|| Ok(())
            )
            .is_err()
        );
        assert_eq!(title(&f), "Before");
        assert_eq!(load(&f.library, &preview.id).unwrap().status, "prepared");
    }
}

fn columns(c: &Connection, table: &str, names: &[&str]) {
    let existing = c
        .prepare(&format!("PRAGMA table_info({table})"))
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<rusqlite::Result<BTreeSet<_>>>()
        .unwrap();
    for name in names.iter().filter(|name| !existing.contains(**name)) {
        c.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {name}"))
            .unwrap();
    }
}
fn cue_fixture(f: &Fixture) {
    add_local(f, "Local cues");
    let source = f._dir.path().canonicalize().unwrap().join("authored.wav");
    let mut wav = hound::WavWriter::create(
        &source,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..48000 {
        wav.write_sample::<i16>(
            ((i as f64 / 48000.0 * 440.0 * std::f64::consts::TAU).sin() * 1000.) as i16,
        )
        .unwrap();
    }
    wav.finalize().unwrap();
    let publication = json!({"source_sample_rate":48000,"publication":{"grid":{"bpm":120.0,"startTime":0.0},"cues":[{"name":"Local drop","startTime":0.2,"type":"1","position":0,"color":null}],"implementation_sha256":"authored","inputs_sha256":"authored"}});
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET analysis_json=?1,source_hash=?2,duration=1 WHERE id='local'",
            params![
                publication.to_string(),
                file_hash(&source).unwrap().unwrap()
            ],
        )
        .unwrap();
    let c = open(&f.native);
    columns(&c, "djmdContent", &["CueUpdated", "AnalysisUpdated"]);
    columns(
        &c,
        "djmdCue",
        &[
            "ContentID",
            "ContentUUID",
            "UUID",
            "Kind",
            "InMsec",
            "OutMsec",
            "InFrame",
            "OutFrame",
            "InMpegFrame",
            "InMpegAbs",
            "OutMpegAbs",
            "OutMpegFrame",
            "ActiveLoop",
            "Comment",
            "Color",
            "ColorTableIndex",
            "BeatLoopSize",
            "rb_local_usn",
            "rb_data_status",
            "rb_local_data_status",
            "rb_local_deleted",
            "rb_local_synced",
            "created_at",
            "updated_at",
        ],
    );
    columns(
        &c,
        "contentCue",
        &[
            "ContentID",
            "Cues",
            "rb_cue_count",
            "UUID",
            "rb_local_usn",
            "rb_data_status",
            "rb_local_data_status",
            "rb_local_deleted",
            "rb_local_synced",
            "created_at",
            "updated_at",
        ],
    );
    c.execute_batch("CREATE TABLE contentFile(ID TEXT PRIMARY KEY)")
        .unwrap();
    columns(
        &c,
        "contentFile",
        &[
            "ContentID",
            "Path",
            "Hash",
            "Size",
            "rb_local_path",
            "UUID",
            "rb_file_hash_dirty",
            "rb_local_file_status",
            "rb_in_progress",
            "rb_process_type",
            "rb_priority",
            "rb_file_size_dirty",
            "rb_local_deleted",
            "rb_local_usn",
            "created_at",
            "updated_at",
        ],
    );
    c.execute("INSERT INTO djmdCue(ID,ContentID,ContentUUID,UUID,Kind,InMsec,OutMsec,ActiveLoop,Comment,Color,ColorTableIndex,BeatLoopSize,rb_local_deleted,created_at,updated_at) VALUES('foreign-hot','1','uuid','cue-uuid',1,100,-1,0,'Foreign drop',-1,0,0,0,'2026-10-21 12:00:00.000 +00:00','2026-10-21 12:00:00.000 +00:00')",[]).unwrap();
    drop(c);
    let cue = NativeCue {
        slot: NativeCueSlot::Hot(0),
        start: NativeTime::from_seconds(0.1).unwrap(),
        end: None,
        active_loop: false,
        order_first: 0,
        order_last: 0,
        comment: "Foreign drop".into(),
        memory_color: 0,
        color: [0; 4],
        loop_fraction: (0, 0),
    };
    for (extension, kind) in [
        ("DAT", NativeAnalysisKind::Dat),
        ("EXT", NativeAnalysisKind::Ext),
        ("2EX", NativeAnalysisKind::TwoEx),
    ] {
        fs::write(
            f.old_asset.with_extension(extension),
            new_native_analysis(
                source.to_str().unwrap(),
                &[],
                std::slice::from_ref(&cue),
                kind,
            )
            .unwrap(),
        )
        .unwrap();
    }
    fs::write(
        f.old_asset.with_extension("3EX"),
        [0x92, 0xa6, b'o', b'p', b'a', b'q', b'u', b'e', 0xc0],
    )
    .unwrap();
    creation_columns(f);
}
fn runtime() -> cuerithm_media::MediaRuntime {
    cuerithm_media::MediaRuntime::from_bundle(
        &std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
            }),
    )
    .unwrap()
}
#[test]
fn explicit_cue_review_replaces_listed_known_hot_cues_and_transfers_only_verified_ownership() {
    let f = fixture(false);
    cue_fixture(&f);
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::new(),
        cues: true,
        beat_grid: false,
        artwork: false,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            "local".into(),
            BTreeMap::from([("cues".into(), NativeFieldDecision::UseLocal)]),
        )]),
    };
    let original = fs::read(&f.old_asset).unwrap();
    let preview = planning::preview(
        &f.library,
        request,
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    let op = load(&f.library, &preview.id).unwrap();
    assert_eq!(op.resolved[0].owned_cue_ids, vec!["foreign-hot"]);
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM native_sync_track_ownership",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        0,
        "Preview must not transfer ownership"
    );
    publication::fixture_apply(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    let read = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert_eq!(read.tracks[0].cues.len(), 1);
    assert_eq!(read.tracks[0].cues[0].start_seconds, Some(0.2));
    assert_eq!(read.tracks[0].cues[0].comment, "Local drop");
    assert_eq!(fs::read(&f.old_asset).unwrap(), original);
    let path = prior.target.asset_root.join(
        read.tracks[0]
            .source
            .analysis_path
            .as_deref()
            .unwrap()
            .trim_start_matches('/'),
    );
    assert_eq!(
        fs::read(path.with_extension("3EX")).unwrap(),
        fs::read(f.old_asset.with_extension("3EX")).unwrap()
    );
    let owned: String = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT cue_ids_json FROM native_sync_track_ownership WHERE local_track_id='local'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&owned).unwrap(),
        vec!["foreign-hot"]
    );
}
#[test]
fn native_database_hardlinks_cannot_become_hashed_sources_or_asset_inputs() {
    let f = fixture(false);
    assert!(ensure_not_native_file(&f.native, &f.native).is_err());
    let copy = f._dir.path().join("distinct-source.wav");
    fs::copy(&f.native, &copy).unwrap();
    ensure_not_native_file(&f.native, &copy).unwrap();
    let alias = f._dir.path().join("native-as-audio.wav");
    fs::hard_link(&f.native, &alias).unwrap();
    assert!(ensure_not_native_file(&f.native, &alias).is_err());
}

#[test]
fn automatic_cue_proposals_cannot_take_unowned_native_slots() {
    let f = fixture(false);
    cue_fixture(&f);
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let cancel = CancellationToken::default();
    let collection = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let observed = f
        .library
        .import_rekordbox_collection(
            &collection,
            NativeImportOptions {
                settings_revision: f.library.settings().unwrap().revision,
                content_ids: None,
                initial_owners: BTreeMap::new(),
            },
            &cancel,
        )
        .unwrap();
    let link = f
        .library
        .native_track_link(&observed.target_id, "1")
        .unwrap();
    let mut fields = link.fields;
    fields.insert(
        "cues".into(),
        NativeFieldBaseline {
            local: json!([]),
            remote: serde_json::to_value(&collection.tracks[0].cues).unwrap(),
            owner: NativeFieldOwner::Shared,
            acknowledged: true,
        },
    );
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE native_track_links SET baseline_json=?1 WHERE target_id=?2 AND content_id='1'",
            params![serde_json::to_string(&fields).unwrap(), observed.target_id],
        )
        .unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::new(),
        cues: true,
        beat_grid: false,
        artwork: false,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::new(),
    };
    let original = fs::read(&f.old_asset).unwrap();
    let preview = planning::preview(
        &f.library,
        request,
        prior.target,
        &key(),
        Some(&runtime()),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert!(matches!(
        preview.fields[0].result,
        NativeReconciliation::Conflict {
            reason: NativeConflict::Ownership,
            ..
        }
    ));
    assert_eq!(preview.fields[0].decision, None);
    assert_eq!(preview.changed_rows, 0);
    f.library.discard_rekordbox_sync(&preview.id).unwrap();
    assert_eq!(fs::read(&f.old_asset).unwrap(), original);
    assert_eq!(
        fs::read_dir(root(&f.library).join("operations"))
            .unwrap()
            .count(),
        0,
        "Failed previews release only their owned staging resources"
    );
    assert_eq!(
        open(&f.native)
            .query_row(
                "SELECT InMsec FROM djmdCue WHERE ID='foreign-hot'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        100
    );
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM native_sync_track_ownership",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn later_versioned_asset_edits_refuse_before_restoring_database_references() {
    let f = fixture(true);
    publication::fixture_apply(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    fs::write(&f.new_asset, b"later native analysis edit").unwrap();
    let after = native_hash(&f);
    assert!(
        publication::fixture_restore(
            &f.library,
            &f.id,
            &key(),
            &CancellationToken::default(),
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(native_hash(&f), after);
    assert_eq!(title(&f), "After");
    assert_eq!(
        fs::read(&f.new_asset).unwrap(),
        b"later native analysis edit"
    );
}

fn creation_columns(f: &Fixture) {
    let c = open(&f.native);
    columns(
        &c,
        "djmdContent",
        &[
            "MasterDBID",
            "DeviceID",
            "OrgFolderPath",
            "FileNameL",
            "FileNameS",
            "ImagePath",
            "ContentLink",
            "HotCueAutoLoad",
            "SamplerTrackInfo",
            "Analysed",
            "CueUpdated",
            "AnalysisUpdated",
            "TrackInfoUpdated",
            "SamplerPlayOffset",
            "SamplerGain",
            "VideoAssociate",
            "LyricStatus",
            "ExtInfo",
            "rb_file_id",
            "FileType",
            "Length",
            "FileSize",
            "SampleRate",
            "BitDepth",
            "BitRate",
            "BPM",
            "rb_local_usn",
            "rb_data_status",
            "rb_local_data_status",
            "rb_local_deleted",
            "rb_local_synced",
            "created_at",
            "updated_at",
        ],
    );
    for table in [
        "djmdPlaylist",
        "djmdSongPlaylist",
        "djmdMyTag",
        "djmdSongMyTag",
    ] {
        columns(
            &c,
            table,
            &[
                "UUID",
                "ParentID",
                "Name",
                "Attribute",
                "Seq",
                "SmartList",
                "PlaylistID",
                "MyTagID",
                "ContentID",
                "TrackNo",
                "rb_local_usn",
                "rb_data_status",
                "rb_local_data_status",
                "rb_local_deleted",
                "rb_local_synced",
                "created_at",
                "updated_at",
            ],
        );
    }
}
#[test]
fn new_native_tracks_and_owned_playlist_share_frozen_ids_without_duplicate_local_songs() {
    let f = fixture(false);
    cue_fixture(&f);
    creation_columns(&f);
    open(&f.native)
        .execute(
            "UPDATE djmdContent SET FolderPath='/missing/authored.flac' WHERE ID='1'",
            [],
        )
        .unwrap();
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    f.library
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("set".into()),
                parent_id: None,
                kind: PlaylistKind::Playlist,
                name: "Prepared set".into(),
                definition: None,
            },
            f.library.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.library
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "set".into(),
                track_ids: vec!["local".into()],
            },
            f.library.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.library.conn().unwrap().execute_batch("INSERT INTO custom_tag_categories(id,name) VALUES('category','Mood'); INSERT INTO custom_tags(id,category_id,name) VALUES('tag','category','Warm'); INSERT INTO track_custom_tags(track_id,tag_id) VALUES('local','tag');").unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::from(["title".into()]),
        cues: true,
        beat_grid: true,
        artwork: false,
        playlists: true,
        my_tags: true,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            "local".into(),
            BTreeMap::from([
                ("title".into(), NativeFieldDecision::UseLocal),
                ("cues".into(), NativeFieldDecision::UseLocal),
                ("beat_grid".into(), NativeFieldDecision::UseLocal),
            ]),
        )]),
    };
    let mut initial_request = request.clone();
    initial_request.decisions.clear();
    let initial = planning::preview(
        &f.library,
        initial_request,
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .expect("First sync returns field choices before resolving new entity members");
    assert!(!initial.can_apply);
    assert_eq!(initial.pending_entities.len(), 2);
    assert!(
        initial
            .pending_entities
            .iter()
            .any(|entity| entity.name == "Prepared set" && entity.unresolved_members == 1)
    );
    assert!(
        initial
            .pending_entities
            .iter()
            .any(|entity| entity.name == "Warm" && entity.kind == RekordboxEntityKind::Tag)
    );
    assert!(initial.entity_changes.is_empty());
    assert!(
        initial
            .fields
            .iter()
            .any(|field| field.field == "title" && field.decision.is_none())
    );
    let initial_op = load(&f.library, &initial.id).unwrap();
    assert!(
        initial_op.playlist_plan.is_none(),
        "No partial membership mutation is publishable"
    );
    assert!(initial_op.plan.rows.is_empty());
    f.library.discard_rekordbox_sync(&initial.id).unwrap();
    let mut tag_request = request.clone();
    tag_request.decisions.clear();
    tag_request.options.playlists = false;
    let tags_only = planning::preview(
        &f.library,
        tag_request,
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .expect("My Tag dependencies also return the initial field review");
    assert!(!tags_only.can_apply);
    assert_eq!(tags_only.pending_entities.len(), 1);
    assert_eq!(tags_only.pending_entities[0].name, "Warm");
    f.library.discard_rekordbox_sync(&tags_only.id).unwrap();
    let mut partial_request = request.clone();
    partial_request
        .decisions
        .get_mut("local")
        .unwrap()
        .remove("title");
    let partial = planning::preview(
        &f.library,
        partial_request,
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .expect("Cue and grid choices can precede the explicit title creation choice");
    assert!(!partial.can_apply);
    assert_eq!(partial.pending_entities.len(), 2);
    assert!(load(&f.library, &partial.id).unwrap().plan.rows.is_empty());
    f.library.discard_rekordbox_sync(&partial.id).unwrap();
    let preview = planning::preview(
        &f.library,
        request.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(preview.media_copies, 0);
    assert!(preview.can_apply);
    assert!(preview.pending_entities.is_empty());
    assert_eq!(preview.entity_changes.len(), 3);
    let playlist_review = preview
        .entity_changes
        .iter()
        .find(|entity| entity.kind == RekordboxEntityKind::Playlist)
        .unwrap();
    assert_eq!(playlist_review.action, RekordboxEntityAction::Create);
    assert_eq!(playlist_review.ownership, RekordboxEntityOwnership::New);
    let state = playlist_review.after.as_ref().unwrap();
    assert_eq!(state.name, "Prepared set");
    assert_eq!(state.member_count, 1);
    assert_eq!(state.members[0].position, Some(1));
    let op = load(&f.library, &preview.id).unwrap();
    assert!(op.resolved[0].created);
    let native_id = op.resolved[0].identity.content_id.clone();
    assert_ne!(native_id, "1");
    publication::fixture_apply(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    let read = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert_eq!(read.tracks.len(), 2);
    let track = read
        .tracks
        .iter()
        .find(|t| t.identity.content_id == native_id)
        .unwrap();
    assert_eq!(track.source.file_type, Some(11));
    assert_eq!(track.source.sample_rate, Some(48000));
    assert_eq!(track.cues.len(), 1);
    assert_eq!(read.playlists.len(), 1);
    assert_eq!(read.tags.len(), 2);
    assert_eq!(read.tag_links.len(), 1);
    assert_eq!(read.tag_links[0].content_id, native_id);
    assert_eq!(read.playlist_links[0].content_id, native_id);
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.library
            .native_track_link(&op.native_target_id, &native_id)
            .unwrap()
            .local_track_id
            .as_deref(),
        Some("local")
    );
    let mut repeat = request;
    repeat.decisions.clear();
    let second = planning::preview(
        &f.library,
        repeat.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(second.changed_rows, 0);
    assert_eq!(second.media_copies, 0);
    f.library.discard_rekordbox_sync(&second.id).unwrap();
    open(&f.native)
        .execute(
            "UPDATE djmdContent SET Title='Unrelated external title' WHERE ID='1'",
            [],
        )
        .unwrap();
    let third = planning::preview(
        &f.library,
        repeat.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(
        third.changed_rows, 0,
        "Unrelated native metadata cannot invalidate unchanged entity/link ownership proofs"
    );
    f.library.discard_rekordbox_sync(&third.id).unwrap();
    open(&f.native)
        .execute("UPDATE djmdPlaylist SET Name='External rename'", [])
        .unwrap();
    assert!(
        planning::preview(
            &f.library,
            repeat.clone(),
            prior.target.clone(),
            &key(),
            Some(&runtime()),
            &CancellationToken::default(),
            planning::PreviewPermit::AuthoredFixture
        )
        .is_err(),
        "A real native entity change still needs reconciliation"
    );
    open(&f.native)
        .execute("UPDATE djmdPlaylist SET Name='Prepared set'", [])
        .unwrap();
    f.library
        .apply_playlist_edit(
            PlaylistEdit::Remove { id: "set".into() },
            f.library.playlist_state().unwrap().revision,
        )
        .unwrap();
    let removal = planning::preview(
        &f.library,
        repeat.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(removal.entity_changes.len(), 1);
    let deletion = &removal.entity_changes[0];
    assert_eq!(deletion.action, RekordboxEntityAction::Remove);
    assert_eq!(deletion.ownership, RekordboxEntityOwnership::Owned);
    assert_eq!(deletion.before.as_ref().unwrap().name, "Prepared set");
    assert!(deletion.after.is_none());
    let removal_op = load(&f.library, &removal.id).unwrap();
    assert!(matches!(
        removal_op
            .playlist_plan
            .as_ref()
            .unwrap()
            .playlists
            .as_slice(),
        [NativePlaylistMutation::Remove { .. }]
    ));
    publication::fixture_apply(
        &f.library,
        &removal.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    let removed = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert!(
        removed
            .playlists
            .iter()
            .all(|playlist| playlist.deleted == Some(true))
    );
    assert!(
        removed
            .playlist_links
            .iter()
            .all(|link| link.deleted == Some(true))
    );
    assert_eq!(removed.tracks.len(), 2);

    let curate = crate::CurationService::new(f.library.clone(), None);
    curate
        .edit_taxonomy(crate::CurationTaxonomyRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: curate.catalog().unwrap().revision,
            change: crate::CurationTaxonomyChange::DeleteGroup {
                id: "category".into(),
                replacement_id: None,
            },
        })
        .unwrap();
    let tags = planning::preview(
        &f.library,
        repeat,
        prior.target,
        &key(),
        Some(&runtime()),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(tags.entity_changes.len(), 2);
    assert!(
        tags.entity_changes
            .iter()
            .all(|entity| entity.action == RekordboxEntityAction::Remove
                && entity.ownership == RekordboxEntityOwnership::Owned)
    );
    assert!(
        tags.entity_changes
            .iter()
            .any(|entity| entity.kind == RekordboxEntityKind::Category
                && entity.before.as_ref().unwrap().name == "Mood")
    );
    let tag_op = load(&f.library, &tags.id).unwrap();
    assert_eq!(tag_op.playlist_plan.as_ref().unwrap().tags.len(), 2);
    publication::fixture_apply(
        &f.library,
        &tags.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    let removed_tags = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert!(
        removed_tags
            .tags
            .iter()
            .all(|tag| tag.deleted == Some(true))
    );
    assert!(
        removed_tags
            .tag_links
            .iter()
            .all(|link| link.deleted == Some(true))
    );
    assert_eq!(removed_tags.tracks.len(), 2);
}

#[test]
fn explicit_source_only_relocation_rewrites_complete_bundle_paths_and_preserves_foreign_cues() {
    let f = fixture(false);
    cue_fixture(&f);
    creation_columns(&f);
    let cancel = CancellationToken::default();
    let current = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    f.library
        .import_rekordbox_collection(
            &current,
            NativeImportOptions {
                settings_revision: f.library.settings().unwrap().revision,
                content_ids: None,
                initial_owners: BTreeMap::new(),
            },
            &cancel,
        )
        .unwrap();
    let original_path = f._dir.path().canonicalize().unwrap().join("authored.wav");
    let moved = f._dir.path().canonicalize().unwrap().join("relocated.wav");
    fs::rename(&original_path, &moved).unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET path=?1 WHERE id='local'",
            [crate::model::path_bytes(&moved)],
        )
        .unwrap();
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::from(["source_path".into()]),
        cues: false,
        beat_grid: false,
        artwork: false,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            "local".into(),
            BTreeMap::from([("source_path".into(), NativeFieldDecision::UseLocal)]),
        )]),
    };
    let old = fs::read(&f.old_asset).unwrap();
    let preview = planning::preview(
        &f.library,
        request,
        prior.target.clone(),
        &key(),
        Some(&runtime()),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    publication::fixture_apply(&f.library, &preview.id, &key(), &cancel, &|| Ok(())).unwrap();
    let published = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let track = &published.tracks[0];
    assert_eq!(track.source.path.as_deref(), moved.to_str());
    assert_eq!(track.cues.len(), 1);
    assert_eq!(track.cues[0].id, "foreign-hot");
    assert_eq!(track.cues[0].start_seconds, Some(0.1));
    let native = prior.target.asset_root.join(
        track
            .source
            .analysis_path
            .as_deref()
            .unwrap()
            .trim_start_matches('/'),
    );
    for extension in ["DAT", "EXT", "2EX"] {
        let file = cuerithm_rekordbox::anlz::AnlzFile::parse(
            &fs::read(native.with_extension(extension)).unwrap(),
            Default::default(),
        )
        .unwrap();
        assert!(
            file.tags()
                .iter()
                .filter(|tag| tag.fourcc() == *b"PPTH")
                .all(|tag| tag.audio_path().unwrap() == moved.to_str().unwrap())
        );
    }
    assert_eq!(fs::read(&f.old_asset).unwrap(), old);
    assert_eq!(
        fs::read(native.with_extension("3EX")).unwrap(),
        fs::read(f.old_asset.with_extension("3EX")).unwrap()
    );
}

#[test]
fn unsupported_source_needs_opt_in_and_publishes_owned_audio_without_duplicate_import_or_clock_shift()
 {
    let f = fixture(false);
    cue_fixture(&f);
    creation_columns(&f);
    let sources = f._dir.path().join("media");
    fs::create_dir(&sources).unwrap();
    let archive =
        File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/media.tar.gz"))
            .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(&sources)
        .unwrap();
    let cases: Value =
        serde_json::from_slice(&fs::read(sources.join("cases.json")).unwrap()).unwrap();
    let case = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["file"].as_str().unwrap().ends_with(".wma"))
        .unwrap();
    let original = sources
        .join(case["file"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    let runtime = runtime();
    let decoded = runtime.decode(&original, &sources).unwrap();
    let hash = file_hash(&original).unwrap().unwrap();
    f.library.conn().unwrap().execute("UPDATE tracks SET path=?1,source_hash=?2,metadata_json=?3,duration=?4 WHERE id='local'",params![crate::model::path_bytes(&original),hash,serde_json::to_string(&decoded.info).unwrap(),decoded.clock.duration_seconds()]).unwrap();
    open(&f.native)
        .execute(
            "UPDATE djmdContent SET FolderPath='/missing/authored.flac' WHERE ID='1'",
            [],
        )
        .unwrap();
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::from(["title".into()]),
        cues: true,
        beat_grid: true,
        artwork: false,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let mut request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            "local".into(),
            BTreeMap::from([
                ("title".into(), NativeFieldDecision::UseLocal),
                ("cues".into(), NativeFieldDecision::UseLocal),
                ("beat_grid".into(), NativeFieldDecision::UseLocal),
            ]),
        )]),
    };
    assert!(
        planning::preview(
            &f.library,
            request.clone(),
            prior.target.clone(),
            &key(),
            Some(&runtime),
            &CancellationToken::default(),
            planning::PreviewPermit::AuthoredFixture
        )
        .unwrap_err()
        .to_string()
        .contains("compatible audio copies")
    );
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        1,
        "Preview does not implicitly import unrelated native-only audio"
    );
    request.options.convert_incompatible_audio = true;
    let preview = planning::preview(
        &f.library,
        request.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(preview.media_copies, 1);
    assert!(preview.copied_audio_bytes > 44);
    let op = load(&f.library, &preview.id).unwrap();
    let native_id = op.resolved[0].identity.content_id.clone();
    let proof = op.sources[0].native_media.as_ref().unwrap();
    assert!(proof.converted);
    assert_eq!(proof.native_file_type, 11);
    assert_eq!(proof.original.sha256, hash);
    assert!(!proof.native_path.exists());
    publication::fixture_apply(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    let read = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    let native = read
        .tracks
        .iter()
        .find(|t| t.identity.content_id == native_id)
        .unwrap();
    assert_eq!(native.source.path.as_deref(), proof.native_path.to_str());
    assert_eq!(native.source.file_type, Some(11));
    assert_eq!(native.cues[0].start_seconds, Some(0.2));
    assert!(proof.native_path.is_file());
    assert_eq!(
        file_hash(&original).unwrap().as_deref(),
        Some(hash.as_str())
    );
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        f.library
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, u64>(0))
            .unwrap(),
        0
    );
    let dat = prior.target.asset_root.join(
        native
            .source
            .analysis_path
            .as_deref()
            .unwrap()
            .trim_start_matches('/'),
    );
    let dat =
        cuerithm_rekordbox::anlz::AnlzFile::parse(&fs::read(dat).unwrap(), Default::default())
            .unwrap();
    assert_eq!(
        dat.tags()
            .iter()
            .find(|t| t.fourcc() == *b"PPTH")
            .unwrap()
            .audio_path()
            .unwrap(),
        proof.native_path.to_str().unwrap()
    );
    let repeat = planning::preview(
        &f.library,
        request,
        prior.target,
        &key(),
        Some(&runtime),
        &CancellationToken::default(),
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(repeat.media_copies, 0);
    assert_eq!(repeat.changed_rows, 0);
    assert!(
        !repeat
            .fields
            .iter()
            .any(|field| field.field == "source_path"),
        "The owned compatible-copy relationship is not a new source conflict"
    );
    f.library.discard_rekordbox_sync(&repeat.id).unwrap();
    publication::fixture_restore(
        &f.library,
        &preview.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!proof.native_path.exists());
    assert_eq!(
        file_hash(&original).unwrap().as_deref(),
        Some(hash.as_str())
    );
}

#[test]
fn artwork_observation_uses_native_pioneer_root_and_retains_escape_guards() {
    let f = fixture(false);
    let target = load(&f.library, &f.id).unwrap().target;
    let artwork = target.asset_root.join("PIONEER/Artwork");
    fs::create_dir_all(&artwork).unwrap();
    fs::write(artwork.join("cover.jpg"), b"native artwork").unwrap();
    fs::create_dir_all(target.asset_root.join("Artwork")).unwrap();
    fs::write(target.asset_root.join("Artwork/cover.jpg"), b"wrong root").unwrap();
    let cancel = CancellationToken::default();
    assert_eq!(
        assets::artwork_observation(&target, "/PIONEER/Artwork/cover.jpg", &cancel).unwrap(),
        json!(file_hash(&artwork.join("cover.jpg")).unwrap().unwrap())
    );
    assert!(assets::artwork_observation(&target, "/Artwork/cover.jpg", &cancel).is_err());
    assert!(
        assets::artwork_observation(&target, "/PIONEER/Artwork/../cover.jpg", &cancel).is_err()
    );
    #[cfg(unix)]
    {
        let outside = f._dir.path().join("outside.jpg");
        fs::write(&outside, b"outside asset root").unwrap();
        std::os::unix::fs::symlink(outside, artwork.join("escaped.jpg")).unwrap();
        assert!(
            assets::artwork_observation(&target, "/PIONEER/Artwork/escaped.jpg", &cancel).is_err()
        );
    }
}

#[test]
fn artwork_uses_pioneer_root_for_native_references_and_repeated_source_choice_is_a_byte_no_op() {
    let f = fixture(false);
    cue_fixture(&f);
    creation_columns(&f);
    let original = f._dir.path().canonicalize().unwrap().join("covered.flac");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../cuerithm-media/tests/fixtures/covered.flac"),
        &original,
    )
    .unwrap();
    let hash = file_hash(&original).unwrap().unwrap();
    let runtime = runtime();
    let info = runtime.probe(&original).unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET path=?1,source_hash=?2,metadata_json=?3 WHERE id='local'",
            params![
                crate::model::path_bytes(&original),
                hash,
                serde_json::to_string(&info).unwrap()
            ],
        )
        .unwrap();
    open(&f.native)
        .execute(
            "UPDATE djmdContent SET FolderPath=?1 WHERE ID='1'",
            [original.to_str().unwrap()],
        )
        .unwrap();
    let prior = load(&f.library, &f.id).unwrap();
    let existing_artwork = prior.target.asset_root.join("PIONEER/Artwork/Existing");
    fs::create_dir_all(&existing_artwork).unwrap();
    let preserved =
        ["cover.jpg", "cover_m.jpg", "cover_s.jpg"].map(|name| existing_artwork.join(name));
    for path in &preserved {
        fs::write(path, b"native-owned artwork").unwrap();
    }
    open(&f.native)
        .execute(
            "UPDATE djmdContent SET ImagePath='/PIONEER/Artwork/Existing/cover.jpg' WHERE ID='1'",
            [],
        )
        .unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::new(),
        cues: false,
        beat_grid: false,
        artwork: true,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            "local".into(),
            BTreeMap::from([("artwork".into(), NativeFieldDecision::UseLocal)]),
        )]),
    };
    let cancel = CancellationToken::default();
    let preview = planning::preview(
        &f.library,
        request.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    let op = load(&f.library, &preview.id).unwrap();
    assert_eq!(op.asset_sources.len(), preserved.len());
    assert_eq!(
        op.asset_sources
            .iter()
            .map(|source| source.path.clone())
            .collect::<BTreeSet<_>>(),
        preserved.iter().cloned().collect()
    );
    assert_eq!(
        FileTransaction::open(&root(&f.library).join("assets"), &op.asset_journal_id)
            .unwrap()
            .inspect()
            .unwrap()
            .changes
            .len(),
        3
    );
    publication::fixture_apply(&f.library, &preview.id, &key(), &cancel, &|| Ok(())).unwrap();
    let published = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let image_path = published.tracks[0].source.artwork_path.as_deref().unwrap();
    assert!(image_path.starts_with("/PIONEER/Artwork/CUERITHM/"));
    let main = prior
        .target
        .asset_root
        .join(image_path.trim_start_matches('/'));
    assert!(
        main.is_file(),
        "native ImagePath must resolve relative to share"
    );
    assert!(
        !prior
            .target
            .asset_root
            .join("PIONEER")
            .join(image_path.trim_start_matches('/'))
            .exists()
    );
    let stem = main.file_stem().unwrap().to_str().unwrap();
    let expected = cuerithm_dj_sync::native_rekordbox_artwork(&original, &runtime, &cancel)
        .unwrap()
        .unwrap();
    for (suffix, bytes) in [
        ("", expected.main),
        ("_m", expected.medium),
        ("_s", expected.small),
    ] {
        let path = main.with_file_name(format!("{stem}{suffix}.jpg"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    assert_eq!(
        file_hash(&original).unwrap().as_deref(),
        Some(hash.as_str())
    );
    let repeat = planning::preview(
        &f.library,
        request,
        prior.target,
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(repeat.changed_rows, 0);
    let repeat_op = load(&f.library, &repeat.id).unwrap();
    assert!(
        FileTransaction::open(
            &root(&f.library).join("assets"),
            &repeat_op.asset_journal_id
        )
        .unwrap()
        .inspect()
        .unwrap()
        .changes
        .is_empty()
    );
    f.library.discard_rekordbox_sync(&repeat.id).unwrap();
    fs::write(&preserved[0], b"later native artwork edit").unwrap();
    assert!(
        publication::fixture_restore(&f.library, &preview.id, &key(), &cancel, &|| Ok(())).is_err(),
        "restore must validate the original artwork under share/PIONEER"
    );
    assert!(main.is_file());
    fs::write(&preserved[0], b"native-owned artwork").unwrap();
    publication::fixture_restore(&f.library, &preview.id, &key(), &cancel, &|| Ok(())).unwrap();
    for suffix in ["", "_m", "_s"] {
        assert!(!main.with_file_name(format!("{stem}{suffix}.jpg")).exists());
    }
    for path in preserved {
        assert_eq!(fs::read(path).unwrap(), b"native-owned artwork");
    }
    let restored = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    assert_eq!(
        restored.tracks[0].source.artwork_path.as_deref(),
        Some("/PIONEER/Artwork/Existing/cover.jpg")
    );
    assert_eq!(
        file_hash(&original).unwrap().as_deref(),
        Some(hash.as_str())
    );
}

#[test]
fn cancelled_asset_publication_retains_recoverable_journal_and_never_acknowledges_database() {
    let f = fixture(true);
    let cancel = CancellationToken::default();
    let result = publication::fixture_apply(&f.library, &f.id, &key(), &cancel, &|| {
        if f.new_asset.exists() {
            cancel.cancel();
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(title(&f), "Before");
    assert_eq!(load(&f.library, &f.id).unwrap().status, "publishing");
    assert!(load(&f.library, &f.id).unwrap().outcome.is_none());
    let result = publication::fixture_recover(
        &f.library,
        &f.id,
        &key(),
        &CancellationToken::default(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(result.status, "rolled_back");
    assert!(!f.new_asset.exists());
    assert_eq!(title(&f), "Before");
}

#[test]
fn a_source_byte_change_refuses_restore_without_starting_an_interrupted_operation() {
    let f = fixture(false);
    add_local(&f, "Published local");
    let preview = metadata_preview(&f, Some(NativeFieldDecision::UseLocal));
    let cancel = CancellationToken::default();
    publication::fixture_apply(&f.library, &preview.id, &key(), &cancel, &|| Ok(())).unwrap();
    let op = load(&f.library, &preview.id).unwrap();
    fs::write(&op.sources[0].path, b"later user music bytes").unwrap();
    assert!(
        publication::fixture_restore(&f.library, &preview.id, &key(), &cancel, &|| Ok(())).is_err()
    );
    assert_eq!(load(&f.library, &preview.id).unwrap().status, "applied");
    assert_eq!(title(&f), "Published local");
    assert_eq!(
        fs::read(&op.sources[0].path).unwrap(),
        b"later user music bytes"
    );
}

#[test]
fn replacing_prepared_pcm_rebuilds_known_waveforms_and_retains_opaque_analysis_data() {
    let f = fixture(false);
    cue_fixture(&f);
    let runtime = runtime();
    let cancel = CancellationToken::default();
    let library = f
        .library
        .native_sync_library(DjSyncScope::Full, Some(&runtime))
        .unwrap();
    let timed = cuerithm_dj_sync::timing::export_track(
        &library.tracks[0],
        cuerithm_dj_sync::DjSoftware::Rekordbox,
    )
    .unwrap();
    let foreign = NativeCue {
        slot: NativeCueSlot::Hot(0),
        start: NativeTime::from_seconds(0.1).unwrap(),
        end: None,
        active_loop: false,
        order_first: 0,
        order_last: 0,
        comment: "Foreign drop".into(),
        memory_color: 0,
        color: [0; 4],
        loop_fraction: (0, 0),
    };
    let generated =
        cuerithm_dj_sync::native_rekordbox_analysis(&timed, &[foreign], &runtime, &cancel).unwrap();
    let opaque = [b'X', b'T', b'R', b'A', 0, 0, 0, 12, 0, 0, 0, 16, 1, 2, 3, 4];
    for (extension, bytes) in [
        ("DAT", generated.dat),
        ("EXT", generated.ext),
        ("2EX", generated.two_ex),
    ] {
        let bytes = if extension == "DAT" {
            let file =
                cuerithm_rekordbox::anlz::AnlzFile::parse(&bytes, Default::default()).unwrap();
            let mut tags = file.tags().to_vec();
            tags.push(
                cuerithm_rekordbox::anlz::AnlzTag::parse(&opaque, Default::default()).unwrap(),
            );
            cuerithm_rekordbox::anlz::AnlzFile::new(tags)
                .unwrap()
                .to_bytes()
                .unwrap()
        } else {
            bytes
        };
        fs::write(f.old_asset.with_extension(extension), bytes).unwrap();
    }
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::new(),
        cues: true,
        beat_grid: false,
        artwork: false,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            "local".into(),
            BTreeMap::from([("cues".into(), NativeFieldDecision::UseLocal)]),
        )]),
    };
    let first = planning::preview(
        &f.library,
        request.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    publication::fixture_apply(&f.library, &first.id, &key(), &cancel, &|| Ok(())).unwrap();
    let read = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let old_native = prior.target.asset_root.join(
        read.tracks[0]
            .source
            .analysis_path
            .as_deref()
            .unwrap()
            .trim_start_matches('/'),
    );
    let old_bytes = fs::read(&old_native).unwrap();
    let original = f._dir.path().canonicalize().unwrap().join("authored.wav");
    let mut wav = hound::WavWriter::create(
        &original,
        hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for frame in 0..72000 {
        let sample = if frame < 24000 {
            0
        } else {
            ((frame as f64 / 48000. * 1700. * std::f64::consts::TAU).sin() * 20000.) as i16
        };
        wav.write_sample(sample).unwrap();
    }
    wav.finalize().unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET source_hash=?1,duration=1.5 WHERE id='local'",
            [file_hash(&original).unwrap().unwrap()],
        )
        .unwrap();
    let second = planning::preview(
        &f.library,
        request,
        prior.target.clone(),
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert!(
        second
            .warnings
            .iter()
            .any(|warning| warning.contains("Rebuild rekordbox waveforms"))
    );
    publication::fixture_apply(&f.library, &second.id, &key(), &cancel, &|| Ok(())).unwrap();
    let read = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let new_native = prior.target.asset_root.join(
        read.tracks[0]
            .source
            .analysis_path
            .as_deref()
            .unwrap()
            .trim_start_matches('/'),
    );
    let new_bytes = fs::read(&new_native).unwrap();
    assert_ne!(old_native, new_native);
    assert_eq!(fs::read(&old_native).unwrap(), old_bytes);
    let old = cuerithm_rekordbox::anlz::AnlzFile::parse(&old_bytes, Default::default()).unwrap();
    let new = cuerithm_rekordbox::anlz::AnlzFile::parse(&new_bytes, Default::default()).unwrap();
    let tag = |file: &cuerithm_rekordbox::anlz::AnlzFile, kind| {
        file.tags()
            .iter()
            .find(|tag| tag.fourcc() == kind)
            .unwrap()
            .bytes()
            .to_vec()
    };
    assert_ne!(tag(&old, *b"PWAV"), tag(&new, *b"PWAV"));
    assert_eq!(tag(&new, *b"XTRA"), opaque);
    assert_eq!(read.tracks[0].cues[0].start_seconds, Some(0.2));
    assert_eq!(
        read.tracks[0].source.file_size,
        Some(fs::metadata(&original).unwrap().len())
    );
    assert_eq!(read.tracks[0].duration_seconds, Some(2.0));
}

#[test]
fn recreated_cue_ids_remain_unowned_after_remote_acknowledgement() {
    let f = fixture(false);
    cue_fixture(&f);
    let runtime = runtime();
    let cancel = CancellationToken::default();
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::new(),
        cues: true,
        beat_grid: false,
        artwork: false,
        playlists: false,
        my_tags: false,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let mut request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            "local".into(),
            BTreeMap::from([("cues".into(), NativeFieldDecision::UseLocal)]),
        )]),
    };
    let first = planning::preview(
        &f.library,
        request.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    publication::fixture_apply(&f.library, &first.id, &key(), &cancel, &|| Ok(())).unwrap();
    let first = load(&f.library, &first.id).unwrap();
    let mut fields = f
        .library
        .native_track_link(&first.native_target_id, "1")
        .unwrap()
        .fields;
    fields.get_mut("cues").unwrap().owner = NativeFieldOwner::Shared;
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE native_track_links SET baseline_json=?1 WHERE target_id=?2 AND content_id='1'",
            params![
                serde_json::to_string(&fields).unwrap(),
                first.native_target_id
            ],
        )
        .unwrap();
    open(&f.native)
        .execute(
            "UPDATE djmdCue SET UUID='recreated-uuid' WHERE ID='foreign-hot'",
            [],
        )
        .unwrap();
    request.decisions.clear();
    let remote = planning::preview(
        &f.library,
        request.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert_eq!(
        remote.fields[0].decision,
        Some(NativeFieldDecision::UseRemote)
    );
    publication::fixture_apply(&f.library, &remote.id, &key(), &cancel, &|| Ok(())).unwrap();
    let mut analysis: Value = serde_json::from_str(
        &f.library
            .conn()
            .unwrap()
            .query_row(
                "SELECT analysis_json FROM tracks WHERE id='local'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    analysis["publication"]["cues"][0]["startTime"] = json!(0.3);
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET analysis_json=?1 WHERE id='local'",
            [analysis.to_string()],
        )
        .unwrap();
    let local = planning::preview(
        &f.library,
        request,
        prior.target,
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    assert!(matches!(
        local.fields[0].result,
        NativeReconciliation::Conflict {
            reason: NativeConflict::Ownership,
            ..
        }
    ));
    assert_eq!(local.fields[0].decision, None);
    assert_eq!(local.changed_rows, 0);
    assert_eq!(
        open(&f.native)
            .query_row(
                "SELECT UUID FROM djmdCue WHERE ID='foreign-hot'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "recreated-uuid"
    );
}
#[test]
fn changed_recorded_native_selection_invalidates_a_metadata_only_preview() {
    let f = fixture(false);
    add_local(&f, "Local publish");
    let preview = metadata_preview(&f, Some(NativeFieldDecision::UseLocal));
    let cancel = CancellationToken::default();
    let collection = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    f.library
        .import_rekordbox_collection(
            &collection,
            NativeImportOptions {
                settings_revision: f.library.settings().unwrap().revision,
                content_ids: Some(BTreeSet::new()),
                initial_owners: BTreeMap::new(),
            },
            &cancel,
        )
        .unwrap();
    let error = publication::fixture_apply(&f.library, &preview.id, &key(), &cancel, &|| Ok(()))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Recorded native observation changed")
    );
    assert_eq!(load(&f.library, &preview.id).unwrap().status, "prepared");
    assert_eq!(title(&f), "Before");
}

#[test]
fn importing_recreated_playlist_and_mytag_links_never_reclaims_their_owned_ids() {
    let f = fixture(false);
    cue_fixture(&f);
    creation_columns(&f);
    let runtime = runtime();
    let cancel = CancellationToken::default();
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    f.library
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("owned".into()),
                parent_id: None,
                kind: PlaylistKind::Playlist,
                name: "Owned set".into(),
                definition: None,
            },
            f.library.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.library
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "owned".into(),
                track_ids: vec!["local".into()],
            },
            f.library.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.library.conn().unwrap().execute_batch("INSERT INTO custom_tag_categories(id,name) VALUES('category','Mood'); INSERT INTO custom_tags(id,category_id,name) VALUES('tag','category','Warm'); INSERT INTO track_custom_tags(track_id,tag_id) VALUES('local','tag');").unwrap();
    let options = RekordboxSyncOptions {
        metadata_fields: BTreeSet::new(),
        cues: false,
        beat_grid: false,
        artwork: false,
        playlists: true,
        my_tags: true,
        materialize_smart_playlists: false,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let mut request = RekordboxSyncRequest {
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::new(),
    };
    let preview = planning::preview(
        &f.library,
        request.clone(),
        prior.target.clone(),
        &key(),
        Some(&runtime),
        &cancel,
        planning::PreviewPermit::AuthoredFixture,
    )
    .unwrap();
    publication::fixture_apply(&f.library, &preview.id, &key(), &cancel, &|| Ok(())).unwrap();
    open(&f.native).execute_batch("UPDATE djmdSongPlaylist SET UUID='external-playlist-link-generation'; UPDATE djmdSongMyTag SET UUID='external-tag-link-generation';").unwrap();
    let current = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let observed = f
        .library
        .import_rekordbox_collection(
            &current,
            NativeImportOptions {
                settings_revision: f.library.settings().unwrap().revision,
                content_ids: None,
                initial_owners: BTreeMap::new(),
            },
            &cancel,
        )
        .unwrap();
    f.library
        .import_native_playlists(
            &observed.target_id,
            &current,
            NativePlaylistImportOptions {
                native_revision: observed.revision,
                playlist_revision: f.library.playlist_state().unwrap().revision,
                materialize_native_ids: BTreeSet::new(),
            },
        )
        .unwrap();
    let owned = f
        .library
        .conn()
        .unwrap()
        .prepare("SELECT owned_links_json FROM native_playlist_bindings WHERE target_id=?1")
        .unwrap()
        .query_map([&observed.target_id], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(owned.iter().all(|ids| {
        serde_json::from_str::<BTreeSet<String>>(ids)
            .unwrap()
            .is_empty()
    }));
    f.library
        .apply_playlist_edit(
            PlaylistEdit::Remove { id: "owned".into() },
            f.library.playlist_state().unwrap().revision,
        )
        .unwrap();
    assert!(
        planning::preview(
            &f.library,
            request.clone(),
            prior.target.clone(),
            &key(),
            Some(&runtime),
            &cancel,
            planning::PreviewPermit::AuthoredFixture
        )
        .is_err()
    );
    let curate = crate::CurationService::new(f.library.clone(), None);
    curate
        .edit_taxonomy(crate::CurationTaxonomyRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            expected_revision: curate.catalog().unwrap().revision,
            change: crate::CurationTaxonomyChange::DeleteGroup {
                id: "category".into(),
                replacement_id: None,
            },
        })
        .unwrap();
    request.options.playlists = false;
    assert!(
        planning::preview(
            &f.library,
            request,
            prior.target,
            &key(),
            Some(&runtime),
            &cancel,
            planning::PreviewPermit::AuthoredFixture
        )
        .is_err()
    );
    assert_eq!(
        open(&f.native)
            .query_row("SELECT rb_local_deleted FROM djmdSongPlaylist", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        open(&f.native)
            .query_row("SELECT rb_local_deleted FROM djmdSongMyTag", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
}

#[test]
fn concrete_entity_review_matches_staged_order_foreign_members_tags_and_materialization() {
    let f = fixture(false);
    creation_columns(&f);
    open(&f.native).execute_batch("UPDATE djmdContent SET Title='External song'; INSERT INTO djmdContent(ID,UUID,MasterSongID,Title,FolderPath,ServiceID,rb_local_deleted) VALUES('2','second','second','Second song','/missing/two.flac',0,0),('3','third','third','Third song','/missing/three.flac',0,0); INSERT INTO djmdPlaylist(ID,UUID,Name,ParentID,Seq,Attribute,rb_local_deleted) VALUES('folder','folder','Weekend','root',1,1,0),('list','list','Before','root',2,0,0); INSERT INTO djmdSongPlaylist(ID,UUID,PlaylistID,ContentID,TrackNo,rb_local_deleted) VALUES('foreign','foreign','list','1',1,0),('owned','owned','list','2',2,0); INSERT INTO djmdMyTag(ID,UUID,Name,ParentID,Seq,Attribute,rb_local_deleted) VALUES('category','category','Mood','root',1,1,0),('tag','tag','Warm','category',1,0,0); INSERT INTO djmdSongMyTag(ID,UUID,MyTagID,ContentID,rb_local_deleted) VALUES('tag-owned','tag-owned','tag','2',0);").unwrap();
    let cancel = CancellationToken::default();
    let before = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let identity =
        |table: &str, id: &str, owned: bool, links: Vec<NativeOwnedRow>| NativeEntityIdentity {
            native_id: id.into(),
            uuid: Some(id.into()),
            expected_raw_hash: before.records[table]
                .iter()
                .find(|row| row.id == id)
                .unwrap()
                .fingerprint
                .clone(),
            owned,
            owned_links: links,
        };
    let link = |table: &str, id: &str| NativeOwnedRow {
        id: id.into(),
        expected_raw_hash: before.records[table]
            .iter()
            .find(|row| row.id == id)
            .unwrap()
            .fingerprint
            .clone(),
    };
    let list_identity = identity(
        "djmdPlaylist",
        "list",
        false,
        vec![link("djmdSongPlaylist", "owned")],
    );
    let drafts = vec![
        NativeEntityDraft {
            kind: NativeEntityKind::Playlist,
            local_id: "set".into(),
            identity: Some(list_identity.clone()),
            parent: Some(NativeEntityRef::Native("folder".into())),
            name: "After".into(),
            attribute: 0,
            sequence: 1,
            smart_rule: None,
            members: vec![
                NativeEntityRef::Native("3".into()),
                NativeEntityRef::Native("2".into()),
            ],
            remove: false,
        },
        NativeEntityDraft {
            kind: NativeEntityKind::MyTag,
            local_id: "category".into(),
            identity: Some(identity("djmdMyTag", "category", true, vec![])),
            parent: None,
            name: "Atmosphere".into(),
            attribute: 1,
            sequence: 2,
            smart_rule: None,
            members: vec![],
            remove: false,
        },
        NativeEntityDraft {
            kind: NativeEntityKind::MyTag,
            local_id: "tag".into(),
            identity: Some(identity(
                "djmdMyTag",
                "tag",
                true,
                vec![link("djmdSongMyTag", "tag-owned")],
            )),
            parent: Some(NativeEntityRef::Native("category".into())),
            name: "Warm".into(),
            attribute: 0,
            sequence: 2,
            smart_rule: None,
            members: vec![NativeEntityRef::Native("3".into())],
            remove: false,
        },
    ];
    let request = NativePlanRequest {
        source: f.native.clone(),
        now: "2026-10-21 12:00:00.000 +00:00".into(),
        iso_now: "2026-10-21T12:00:00.000Z".into(),
        id_seed: [7; 32],
        tracks: vec![],
        entities: drafts.clone(),
        auxiliary_records: vec![],
        share_root: None,
    };
    let prepared = prepare_native_rows(&before, &request).unwrap();
    let workspace = f._dir.path().join("entity-review");
    fs::create_dir(&workspace).unwrap();
    let staged = stage_native_write(&prepared.plan, &workspace, &key(), &cancel).unwrap();
    let after = ReadOnlyDatabase::open(&staged.snapshot.path, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let plan = NativePlaylistPlan {
        target_id: "fixture".into(),
        native_revision: 1,
        playlist_revision: 1,
        library_revision: 1,
        source_fingerprint: String::new(),
        playlists: vec![NativePlaylistMutation::Upsert {
            local_id: "set".into(),
            identity: Some(NativePlaylistIdentity {
                native_id: "list".into(),
                uuid: Some("list".into()),
                expected_raw_hash: list_identity.expected_raw_hash,
                owned: false,
                owned_link_ids: BTreeSet::from(["owned".into()]),
            }),
            parent_local_id: None,
            name: "After".into(),
            position: 0,
            representation: NativePlaylistRepresentation::MaterializedSmart {
                content_ids: vec!["3".into(), "2".into()],
                definition_json: json!({"fixture":"whole-list"}),
            },
            preserve_external_members: true,
        }],
        tags: vec![],
        bindings: BTreeMap::new(),
        existing_native_ids: BTreeSet::new(),
    };
    let changes =
        planning::entity_changes(Some(&plan), &drafts, &prepared.entities, &before, &after)
            .unwrap();
    assert_eq!(changes.len(), 3);
    let list = &changes[0];
    assert_eq!(list.ownership, RekordboxEntityOwnership::Matched);
    assert!(list.materialized_smart);
    assert_eq!(list.before.as_ref().unwrap().name, "Before");
    assert_eq!(list.before.as_ref().unwrap().position, 2);
    let next = list.after.as_ref().unwrap();
    assert_eq!(next.name, "After");
    assert_eq!(next.parent_path, vec!["Weekend"]);
    assert_eq!(next.position, 1);
    assert_eq!(next.member_count, 3);
    assert_eq!(next.external_member_count, 1);
    assert_eq!(
        next.members
            .iter()
            .map(|member| (member.title.as_str(), member.position, member.external))
            .collect::<Vec<_>>(),
        vec![
            ("External song", Some(1), true),
            ("Third song", Some(2), false),
            ("Second song", Some(3), false)
        ]
    );
    let category = &changes[1];
    assert_eq!(category.kind, RekordboxEntityKind::Category);
    assert_eq!(category.before.as_ref().unwrap().name, "Mood");
    assert_eq!(category.after.as_ref().unwrap().name, "Atmosphere");
    assert_eq!(category.after.as_ref().unwrap().position, 2);
    let tag = &changes[2];
    assert_eq!(tag.before.as_ref().unwrap().members[0].title, "Second song");
    assert_eq!(tag.after.as_ref().unwrap().members[0].title, "Third song");
    assert_eq!(tag.after.as_ref().unwrap().parent_path, vec!["Atmosphere"]);
    // The review contract survives JSON round trips and old journals without the added fields.
    let mut payload = serde_json::to_value(load(&f.library, &f.id).unwrap().preview).unwrap();
    payload.as_object_mut().unwrap().remove("entity_changes");
    payload.as_object_mut().unwrap().remove("pending_entities");
    let old: RekordboxSyncPreview = serde_json::from_value(payload).unwrap();
    assert!(old.entity_changes.is_empty() && old.pending_entities.is_empty());
    let wire = serde_json::to_value(&changes).unwrap();
    assert_eq!(wire[0]["ownership"], "matched");
    assert_eq!(wire[0]["after"]["members"][0]["position"], 1);
    assert_eq!(wire[1]["kind"], "category");
}

#[test]
fn native_pqt2_grid_staging_keeps_dat_ext_consistent_and_foreign_sections_unchanged() {
    use cuerithm_rekordbox::anlz::{AnlzFile, AnlzTag, Beat};
    let f = fixture(false);
    cue_fixture(&f);
    let cancel = CancellationToken::default();
    let prior = load(&f.library, &f.id).unwrap();
    let local = f
        .library
        .native_sync_library(DjSyncScope::Full, Some(&runtime()))
        .unwrap();
    let mut timed = cuerithm_dj_sync::timing::export_track(
        &local.tracks[0],
        cuerithm_dj_sync::DjSoftware::Rekordbox,
    )
    .unwrap();
    timed.beatgrid[0].bpm = 128.0;
    timed.beatgrid[0].seconds = 0.125;
    let original_beats = [
        Beat {
            number: 1,
            tempo: 12000,
            time_ms: 80,
        },
        Beat {
            number: 2,
            tempo: 12000,
            time_ms: 580,
        },
    ];
    let mut extended = AnlzTag::microsecond_grid(&original_beats, &[90, 90], false, 0)
        .unwrap()
        .bytes()
        .to_vec();
    extended[48..52].copy_from_slice(&0u32.to_be_bytes());
    let mut originals = BTreeMap::new();
    for extension in ["DAT", "EXT"] {
        let file = AnlzFile::parse(
            &fs::read(f.old_asset.with_extension(extension)).unwrap(),
            Default::default(),
        )
        .unwrap();
        let mut tags: Vec<_> = file
            .tags()
            .iter()
            .filter(|tag| tag.fourcc() != *b"PQTZ")
            .cloned()
            .collect();
        tags.push(if extension == "DAT" {
            AnlzTag::grid(&original_beats).unwrap()
        } else {
            AnlzTag::parse(&extended, Default::default()).unwrap()
        });
        let bytes = AnlzFile::new(tags).unwrap().to_bytes().unwrap();
        fs::write(f.old_asset.with_extension(extension), &bytes).unwrap();
        originals.insert(extension, bytes);
    }
    let collection = ReadOnlyDatabase::open(&f.native, &key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let bundle = assets::build(
        &f.library,
        &prior.target,
        "native-pqt2-test",
        &timed,
        &timed.path,
        &timed.path,
        Some(&collection.tracks[0]),
        &[],
        &BTreeSet::new(),
        false,
        true,
        false,
        false,
        false,
        false,
        None,
        &cancel,
    )
    .unwrap();
    let mut staged_grid = None;
    let mut staged_extended = None;
    for publication in &bundle.publications {
        let extension = publication.target.extension().unwrap().to_str().unwrap();
        if let Some(original) = originals.get(extension) {
            assert_eq!(
                &fs::read(f.old_asset.with_extension(extension)).unwrap(),
                original
            );
            let before = AnlzFile::parse(original, Default::default()).unwrap();
            let after = AnlzFile::parse(
                &fs::read(publication.staged.as_ref().unwrap()).unwrap(),
                Default::default(),
            )
            .unwrap();
            for (old, new) in before.tags().iter().zip(after.tags()) {
                match &new.fourcc() {
                    b"PQTZ" => staged_grid = Some(new.beats().unwrap()),
                    b"PQT2" => staged_extended = Some(new.clone()),
                    _ => assert_eq!(old.bytes(), new.bytes()),
                }
            }
        }
    }
    let beats = staged_grid.unwrap();
    let extended = staged_extended.unwrap();
    let read = extended.native_microsecond_grid(&beats).unwrap();
    assert_eq!(
        beats.iter().map(|beat| beat.time_ms).collect::<Vec<_>>(),
        [125, 593]
    );
    assert_eq!(read.fractions, [0, 750]);
    assert_eq!(
        beats.iter().map(|beat| beat.tempo).collect::<Vec<_>>(),
        [12800, 12800]
    );
    assert_eq!(extended.extended_grid().unwrap().downbeat, 0);
    // An unrecognized producer layout must stop staging, with source files intact.
    let mut bad = originals["EXT"].clone();
    let offset = bad.windows(4).position(|value| value == b"PQT2").unwrap();
    bad[offset + 48..offset + 52].copy_from_slice(&1u32.to_be_bytes());
    fs::write(f.old_asset.with_extension("EXT"), &bad).unwrap();
    let result = assets::build(
        &f.library,
        &prior.target,
        "native-pqt2-unsupported",
        &timed,
        &timed.path,
        &timed.path,
        Some(&collection.tracks[0]),
        &[],
        &BTreeSet::new(),
        false,
        true,
        false,
        false,
        false,
        false,
        None,
        &cancel,
    );
    assert!(result.is_err());
    assert_eq!(fs::read(f.old_asset.with_extension("EXT")).unwrap(), bad);
}

mod restore_gates;

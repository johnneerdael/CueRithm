use crate::*;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use serde_json::json;
use std::{collections::BTreeMap, fs};

struct Reader {
    tracks: Vec<TrackObservation>,
    collections: Vec<CollectionObservation>,
    memberships: Vec<(String, MembershipObservation)>,
    completeness: Completeness,
}
impl SourceAdapter for Reader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::Mixxx
    }
    fn inspect(&self, _: &ImportSnapshot) -> anyhow::Result<SourceProfile> {
        Ok(SourceProfile {
            provider: self.provider(),
            profile_id: "reconcile-fixture".into(),
            schema_fingerprint: "a".repeat(64),
            origin: BTreeMap::new(),
            fields: ImportPolicy::default()
                .selected_fields
                .iter()
                .map(|f| {
                    (
                        serde_json::to_value(f).unwrap().as_str().unwrap().into(),
                        FieldCapability {
                            read: true,
                            write: false,
                            lossless: true,
                            evidence: EvidenceLevel::D2,
                            reason: None,
                        },
                    )
                })
                .collect(),
            issues: vec![],
        })
    }
    fn read(
        &self,
        snapshot: &ImportSnapshot,
        _: &ReadOptions,
        sink: &mut dyn ObservationSink,
        cancel: &CancellationToken,
    ) -> anyhow::Result<ObservationHeader> {
        for t in &self.tracks {
            cancel.check()?;
            sink.push(ObservationRecord::Track(Box::new(t.clone())))?;
        }
        for c in &self.collections {
            sink.push(ObservationRecord::Collection(c.clone()))?;
        }
        for (collection_id, occurrence) in &self.memberships {
            sink.push(ObservationRecord::Membership {
                collection_id: collection_id.clone(),
                occurrence: occurrence.clone(),
            })?;
        }
        Ok(ObservationHeader {
            contract_version: 1,
            source: SourceKey {
                provider: self.provider(),
                library_instance_id: "fixture-library".into(),
            },
            product_version: "fixture".into(),
            schema_fingerprint: "a".repeat(64),
            snapshot_hash: snapshot.snapshot_hash.clone(),
            adapter_version: "fixture".into(),
            completeness: self.completeness,
            scope: json!({"all":true}),
            observed_at: String::new(),
            accounting: RecordAccounting::default(),
            extensions: BTreeMap::new(),
        })
    }
}
pub(super) fn track(title: &str) -> TrackObservation {
    TrackObservation {
        external_id: "a".into(),
        metadata: BTreeMap::from([("title".into(), Presence::Value(json!(title)))]),
        locations: LocationEvidence {
            availability: "missing".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub(super) struct Fixture {
    pub(super) lib: Library,
    pub(super) root: tempfile::TempDir,
    _db: tempfile::TempDir,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let db = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("source.bin"), b"invented").unwrap();
        Self {
            lib: Library::open(&db.path().join("library.db")).unwrap(),
            root,
            _db: db,
        }
    }
    pub(super) fn reopen(self) -> Self {
        let Self { lib, root, _db } = self;
        drop(lib);
        let lib = Library::open(&_db.path().join("library.db")).unwrap();
        Self { lib, root, _db }
    }
    fn try_inspect_records(
        &self,
        tracks: Vec<TrackObservation>,
        collections: Vec<CollectionObservation>,
        memberships: Vec<(String, MembershipObservation)>,
        completeness: Completeness,
    ) -> anyhow::Result<ImportInspection> {
        self.lib.inspect_import_with_reader(
            ImportInspectRequest {
                provider: SourceProvider::Mixxx,
                selection: ImportSourceSelection {
                    root: self.root.path().into(),
                    artifacts: vec![ArtifactRequest::stable("source.bin")],
                },
                binding: SourceBindingChoice::Automatic,
                origin: None,
                path_mappings: vec![],
                capture_file_tags: false,
                include_history: false,
                include_queue: false,
            },
            &Reader {
                tracks,
                collections,
                memberships,
                completeness,
            },
            &CancellationToken::default(),
        )
    }
    fn try_inspect(
        &self,
        tracks: Vec<TrackObservation>,
        completeness: Completeness,
    ) -> anyhow::Result<ImportInspection> {
        self.try_inspect_records(tracks, vec![], vec![], completeness)
    }
    pub(super) fn inspect(
        &self,
        tracks: Vec<TrackObservation>,
        completeness: Completeness,
    ) -> ImportInspection {
        self.try_inspect(tracks, completeness).unwrap()
    }
    pub(super) fn request(&self, i: &ImportInspection) -> ImportPreviewRequest {
        ImportPreviewRequest {
            inspection_id: i.inspection_id.clone(),
            ..Default::default()
        }
    }
    pub(super) fn preview(&self, request: ImportPreviewRequest) -> ImportPreview {
        self.lib
            .preview_import(request, &CancellationToken::default())
            .unwrap()
    }
    pub(super) fn apply(&self, p: &ImportPreview) -> ImportApplyResult {
        self.lib
            .apply_import(&p.preview_id, &CancellationToken::default())
            .unwrap()
    }
    fn title(&self) -> String {
        self.lib.tracks(TrackQuery::default()).unwrap().rows[0]
            .title
            .clone()
    }
    fn edit_title(&self, title: &str) {
        let conn = self.lib.conn().unwrap();
        let id: String = conn
            .query_row("SELECT id FROM import_items LIMIT 1", [], |r| r.get(0))
            .unwrap();
        conn.execute("UPDATE import_active_fields SET value_json=?1,revision=revision+1 WHERE item_id=?2 AND field='metadata.title'",rusqlite::params![json!({"presence":"value","value":title}).to_string(),id]).unwrap();
        super::super::projection::rebuild_summary_tx(&conn, &id).unwrap();
        Library::bump(&conn).unwrap();
    }
}
#[test]
fn all_off_import_has_no_external_effects_and_apply_is_idempotent() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("Source")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
    let a = f.apply(&p);
    assert_eq!(a.tracks_created, 1);
    assert_eq!(f.title(), "Source");
    assert!(f.lib.jobs().unwrap().is_empty());
    let id = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    assert!(f.lib.analysis(&id).unwrap().is_none());
    assert_eq!(f.lib.cue_revision_count(&id).unwrap(), 0);
    let revision = f.lib.revision().unwrap();
    assert_eq!(f.apply(&p).operation_id, a.operation_id);
    assert_eq!(f.lib.revision().unwrap(), revision);
    assert!(!a.processing_requested);
}
#[test]
fn initial_conflicts_keep_local_until_decided() {
    let f = Fixture::new();
    f.lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('existing',?1,'Local','hash','ready',1,1)",[crate::model::path_bytes(std::path::Path::new("/Synthetic/existing.flac"))]).unwrap();
    let i = f.inspect(vec![track("Source")], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.target_items.insert("a".into(), "existing".into());
    let p = f.preview(r.clone());
    assert_eq!(p.conflicts, 1);
    f.apply(&p);
    assert_eq!(f.lib.track("existing").unwrap().title, "Local");
    r.decisions.push(ImportConflictDecision {
        external_id: "a".into(),
        field: "metadata.title".into(),
        decision: ImportDecision::UseSource,
    });
    let p = f.preview(r);
    f.apply(&p);
    assert_eq!(f.lib.track("existing").unwrap().title, "Source");
    assert_eq!(f.lib.tracks(TrackQuery::default()).unwrap().total, 1);
}
#[test]
fn refresh_uses_applied_not_observed_baseline() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    let i = f.inspect(vec![track("B")], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.policy.selected_fields.clear();
    f.apply(&f.preview(r));
    f.edit_title("Local");
    let i = f.inspect(vec![track("B")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.conflicts, 1);
    f.apply(&p);
    assert_eq!(f.title(), "Local");
    let s: String = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT source_json FROM import_applied_baselines WHERE field='metadata.title'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&s).unwrap()["value"],
        "A"
    );
}
#[test]
fn unchanged_refresh_is_zero_work() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    let revision = f.lib.revision().unwrap();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.fields_changed, 0);
    assert_eq!(p.baselines_changed, 0);
    assert_eq!(p.collections_changed, 0);
    f.apply(&p);
    assert_eq!(f.lib.revision().unwrap(), revision);
}
#[test]
fn partial_scope_never_tombstones() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    for completeness in [Completeness::SelectedScope, Completeness::PartialWithErrors] {
        let i = f.inspect(vec![], completeness);
        f.apply(&f.preview(f.request(&i)));
        assert_eq!(f.title(), "A");
        assert_eq!(f.lib.counts().unwrap().tracks, 1);
    }
}
#[test]
fn stale_preview_cannot_apply() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    Library::bump(&f.lib.conn().unwrap()).unwrap();
    assert!(
        f.lib
            .apply_import(&p.preview_id, &CancellationToken::default())
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
    let p = f.preview(f.request(&i));
    fs::write(f.root.path().join("source.bin"), b"changed").unwrap();
    assert!(
        f.lib
            .apply_import(&p.preview_id, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
}
#[test]
fn hotcue_selection_is_not_generation_or_replacement_authorization() {
    let f = Fixture::new();
    let mut a = track("A");
    a.cues.push(CueObservation {
        external_id: "cue".into(),
        family: CueFamily::Hot,
        source_slot: Some(4),
        name: Some("Original".into()),
        ..Default::default()
    });
    let i = f.inspect(vec![a.clone()], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    a.cues[0].name = Some("Changed".into());
    let i = f.inspect(vec![a], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.decisions.push(ImportConflictDecision {
        external_id: "a".into(),
        field: "cues.hot".into(),
        decision: ImportDecision::UseSource,
    });
    let p = f.preview(r.clone());
    assert!(p.blocked > 0);
    f.apply(&p);
    let id = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    assert_eq!(
        f.lib.imported_item(&id).unwrap().active_fields["cues.hot"][0]["name"],
        "Original"
    );
    r.policy.replace_hotcues = true;
    let a = f.apply(&f.preview(r));
    assert!(!a.processing_requested);
    assert_eq!(
        f.lib.imported_item(&id).unwrap().active_fields["cues.hot"][0]["name"],
        "Changed"
    );
    assert!(f.lib.jobs().unwrap().is_empty());
}

#[test]
fn source_local_equal_and_divergent_changes_use_three_way_comparison() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    let i = f.inspect(vec![track("B")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.conflicts, 0);
    f.apply(&p);
    assert_eq!(f.title(), "B");
    f.edit_title("Local");
    let i = f.inspect(vec![track("B")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.conflicts, 0);
    assert_eq!(p.fields_changed, 0);
    f.apply(&p);
    assert_eq!(f.title(), "Local");
    let i = f.inspect(vec![track("Local")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.fields_changed, 0);
    assert_eq!(p.baselines_changed, 1);
    f.apply(&p);
    let i = f.inspect(vec![track("Next")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.conflicts, 0);
    f.apply(&p);
    assert_eq!(f.title(), "Next");
}
#[test]
fn null_empty_and_reviewed_deletion_remain_distinct() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    for presence in [Presence::Null, Presence::Empty(json!(""))] {
        let mut t = track("");
        t.metadata.insert("title".into(), presence.clone());
        let i = f.inspect(vec![t], Completeness::WholeLibrary);
        f.apply(&f.preview(f.request(&i)));
        let id = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
            .id
            .clone();
        assert_eq!(
            f.lib.imported_item(&id).unwrap().active_fields["metadata.title"],
            serde_json::to_value(presence).unwrap()
        );
    }
    let mut t = track("");
    t.metadata.insert("title".into(), Presence::Deleted);
    assert!(
        f.try_inspect(vec![t.clone()], Completeness::PartialWithErrors)
            .is_err()
    );
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.decisions.push(ImportConflictDecision {
        external_id: "a".into(),
        field: "metadata.title".into(),
        decision: ImportDecision::UseSource,
    });
    f.apply(&f.preview(r));
    let id = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    assert_eq!(
        f.lib.imported_item(&id).unwrap().active_fields["metadata.title"]["presence"],
        "deleted"
    );
}
#[test]
fn failure_rolls_back_items_baselines_and_receipt_and_allows_retry() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    f.lib.conn().unwrap().execute_batch("CREATE TRIGGER inject_import_failure BEFORE INSERT ON import_applied_baselines BEGIN SELECT RAISE(ABORT,'injected import failure'); END").unwrap();
    assert!(
        f.lib
            .apply_import(&p.preview_id, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
    for table in [
        "import_items",
        "import_bindings",
        "import_operations",
        "import_active_fields",
        "import_applied_baselines",
    ] {
        assert_eq!(
            f.lib
                .conn()
                .unwrap()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
    }
    f.lib
        .conn()
        .unwrap()
        .execute_batch("DROP TRIGGER inject_import_failure")
        .unwrap();
    f.apply(&p);
    assert_eq!(f.title(), "A");
}
#[test]
fn discarded_restarted_and_cancelled_previews_have_no_mutation_authority() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    f.lib.discard_import_preview(&p.preview_id).unwrap();
    assert!(
        f.lib
            .apply_import(&p.preview_id, &CancellationToken::default())
            .is_err()
    );
    let p = f.preview(f.request(&i));
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(f.lib.apply_import(&p.preview_id, &cancel).is_err());
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
    let Fixture {
        lib,
        root: _root,
        _db,
    } = f;
    drop(lib);
    let reopened = Library::open(&_db.path().join("library.db")).unwrap();
    assert!(
        reopened
            .apply_import(&p.preview_id, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(
        reopened
            .import_preview_page(&p.preview_id, None, 10)
            .unwrap()
            .total,
        1
    );
    assert_eq!(reopened.counts().unwrap().tracks, 0);
}
#[test]
fn changed_resolved_media_invalidates_preview_without_decoding() {
    let f = Fixture::new();
    let path = f.root.path().join("media.wav");
    fs::write(&path, b"not decoded").unwrap();
    let mut t = track("A");
    t.locations.platform = Some(std::env::consts::OS.into());
    t.locations.native_path = Some(path.clone());
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    fs::write(&path, b"replacement").unwrap();
    assert!(
        f.lib
            .apply_import(&p.preview_id, &CancellationToken::default())
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
    assert!(f.lib.jobs().unwrap().is_empty());
}
#[test]
fn initial_native_artist_array_conflicts_with_existing_scalar_without_splitting_names() {
    let f = Fixture::new();
    f.lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,artist,source_hash,state,created,updated) VALUES('existing',?1,'A','Local, Band','hash','ready',1,1)",[crate::model::path_bytes(std::path::Path::new("/Synthetic/existing.flac"))]).unwrap();
    let mut t = track("A");
    t.metadata
        .insert("artists".into(), Presence::Value(json!(["Source, Band"])));
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.target_items.insert("a".into(), "existing".into());
    let p = f.preview(r);
    assert_eq!(p.conflicts, 1);
    f.apply(&p);
    assert_eq!(f.lib.track("existing").unwrap().artist, "Local, Band");
}

fn collection(order: &[&str]) -> CollectionObservation {
    CollectionObservation {
        external_id: "playlist".into(),
        kind: CollectionKind::Playlist,
        name: "Source list".into(),
        occurrences: order
            .iter()
            .enumerate()
            .map(|(n, id)| MembershipObservation {
                external_occurrence_id: (*id).into(),
                track_external_id: "a".into(),
                position: Some(n as i64),
                extensions: BTreeMap::new(),
            })
            .collect(),
        ..Default::default()
    }
}
#[test]
fn collection_refresh_preserves_duplicate_members_and_reviews_local_order_conflicts() {
    let f = Fixture::new();
    let inspect = |c| {
        f.try_inspect_records(
            vec![track("A")],
            vec![c],
            vec![],
            Completeness::WholeLibrary,
        )
        .unwrap()
    };
    let i = inspect(collection(&["first", "middle", "last"]));
    f.apply(&f.preview(f.request(&i)));
    let id: String = f
        .lib
        .conn()
        .unwrap()
        .query_row("SELECT id FROM import_collections", [], |r| r.get(0))
        .unwrap();
    let page = f.lib.imported_collection_page(&id, None, 2).unwrap();
    assert_eq!(
        page.rows
            .iter()
            .map(|r| r.occurrence_id.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "middle"]
    );
    assert_eq!(
        f.lib
            .imported_collection_page(&id, page.next_cursor.as_deref(), 2)
            .unwrap()
            .rows[0]
            .occurrence_id,
        "last"
    );
    let revision = f.lib.revision().unwrap();
    let i = inspect(collection(&["first", "middle", "last"]));
    let p = f.preview(f.request(&i));
    assert_eq!(p.collections_changed, 0);
    f.apply(&p);
    assert_eq!(f.lib.revision().unwrap(), revision);
    let i = inspect(collection(&["last", "middle", "first"]));
    let p = f.preview(f.request(&i));
    assert_eq!(p.collections_changed, 1);
    f.apply(&p);
    assert_eq!(
        f.lib.imported_collection_page(&id, None, 3).unwrap().rows[0].occurrence_id,
        "last"
    );
    let mut c = collection(&["last", "middle", "first"]);
    c.name = "Local list".into();
    let conn = f.lib.conn().unwrap();
    conn.execute(
        "UPDATE import_collections SET json=?1,name='Local list',revision=revision+1 WHERE id=?2",
        rusqlite::params![serde_json::to_string(&c).unwrap(), id],
    )
    .unwrap();
    Library::bump(&conn).unwrap();
    drop(conn);
    let i = inspect(collection(&["first", "middle", "last"]));
    let p = f.preview(f.request(&i));
    assert_eq!(p.conflicts, 1);
    assert_eq!(p.collections_changed, 0);
    f.apply(&p);
    assert_eq!(
        f.lib.imported_collection_page(&id, None, 3).unwrap().rows[0].occurrence_id,
        "last"
    );
}
#[test]
fn observation_pages_are_bounded_by_bytes_as_well_as_record_count() {
    let f = Fixture::new();
    let mut a = track("A");
    a.metadata.insert(
        "comment".into(),
        Presence::Value(json!("x".repeat(9 * 1024 * 1024))),
    );
    let mut b = a.clone();
    b.external_id = "b".into();
    let i = f.inspect(vec![a, b], Completeness::WholeLibrary);
    let p = f
        .lib
        .import_page(
            &i.inspection_id,
            ImportPageQuery {
                limit: 100,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(p.records.len(), 1);
    let next = f
        .lib
        .import_page(
            &i.inspection_id,
            ImportPageQuery {
                limit: 100,
                cursor: p.next_cursor,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(next.records.len(), 1);
    assert!(next.next_cursor.is_none());
}

#[test]
fn reused_source_id_with_changed_location_requires_identity_review() {
    let f = Fixture::new();
    let mut a = track("Original");
    a.locations.original = Some("/Foreign/old.flac".into());
    let i = f.inspect(vec![a.clone()], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    a.metadata
        .insert("title".into(), Presence::Value(json!("Replacement")));
    a.locations.original = Some("/Foreign/new.flac".into());
    let i = f.inspect(vec![a], Completeness::WholeLibrary);
    let r = f.request(&i);
    let p = f.preview(r.clone());
    assert!(p.blocked > 0);
    f.apply(&p);
    assert_eq!(f.title(), "Original");
    let mut r = r;
    r.decisions.push(ImportConflictDecision {
        external_id: "a".into(),
        field: "identity".into(),
        decision: ImportDecision::UseSource,
    });
    f.apply(&f.preview(r));
    assert_eq!(f.title(), "Replacement");
}
#[test]
fn linked_native_key_representation_does_not_become_a_false_local_edit() {
    let f = Fixture::new();
    f.lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('existing',?1,'A','hash','ready',1,1)",[crate::model::path_bytes(std::path::Path::new("/Synthetic/existing.flac"))]).unwrap();
    let mut t = track("A");
    t.key = Some(MusicalKey {
        presence: FieldPresence::Value,
        raw: Some("Am".into()),
        camelot: Some("8A".into()),
        ..Default::default()
    });
    let i = f.inspect(vec![t.clone()], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.target_items.insert("a".into(), "existing".into());
    f.apply(&f.preview(r));
    let rev = f.lib.revision().unwrap();
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.conflicts, 0);
    assert_eq!(p.fields_changed, 0);
    assert_eq!(p.baselines_changed, 0);
    f.apply(&p);
    assert_eq!(f.lib.revision().unwrap(), rev);
}

#[test]
fn streamed_collection_memberships_remain_paged_and_outside_preview_json() {
    let f = Fixture::new();
    let c = collection(&[]);
    let members = (0..3000)
        .map(|n| {
            (
                "playlist".into(),
                MembershipObservation {
                    external_occurrence_id: format!("row-{n}"),
                    track_external_id: "a".into(),
                    position: Some(n),
                    extensions: BTreeMap::new(),
                },
            )
        })
        .collect();
    let i = f
        .try_inspect_records(
            vec![track("A")],
            vec![c],
            members,
            Completeness::WholeLibrary,
        )
        .unwrap();
    let p = f.preview(f.request(&i));
    let max:u64=f.lib.conn().unwrap().query_row("SELECT MAX(length(proposal_json)) FROM import_preview_effects WHERE preview_id=?1 AND kind='collection'",[&p.preview_id],|r|r.get(0)).unwrap();
    assert!(max < 10000);
    f.apply(&p);
    let id: String = f
        .lib
        .conn()
        .unwrap()
        .query_row("SELECT id FROM import_collections", [], |r| r.get(0))
        .unwrap();
    let p = f.lib.imported_collection_page(&id, None, 1000).unwrap();
    assert_eq!(p.total, 3000);
    assert_eq!(p.rows.len(), 1000);
    let p = f
        .lib
        .imported_collection_page(&id, p.next_cursor.as_deref(), 1000)
        .unwrap();
    assert_eq!(p.rows[0].occurrence_id, "row-1000");
}
#[test]
fn custom_collection_membership_is_applied_and_cycles_are_rejected_atomically() {
    let f = Fixture::new();
    let i = f
        .try_inspect_records(
            vec![track("A")],
            vec![collection(&["first", "second"])],
            vec![],
            Completeness::WholeLibrary,
        )
        .unwrap();
    let mut r = f.request(&i);
    let mut custom = collection(&["second", "first"]);
    custom.name = "Custom list".into();
    r.decisions.push(ImportConflictDecision {
        external_id: "playlist".into(),
        field: "collection".into(),
        decision: ImportDecision::Custom(serde_json::to_value(&custom).unwrap()),
    });
    let p = f.preview(r.clone());
    f.apply(&p);
    let id: String = f
        .lib
        .conn()
        .unwrap()
        .query_row("SELECT id FROM import_collections", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        f.lib.imported_collection_page(&id, None, 10).unwrap().rows[0].occurrence_id,
        "second"
    );
    custom.parent_id = Some("playlist".into());
    r.decisions[0].decision = ImportDecision::Custom(serde_json::to_value(custom).unwrap());
    assert!(
        f.lib
            .preview_import(r, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(
        f.lib.imported_collection_page(&id, None, 10).unwrap().rows[0].occurrence_id,
        "second"
    );
}
#[test]
fn failed_source_field_stays_unapplied_while_other_fields_can_refresh() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("A")], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    let mut t = track("B");
    t.metadata
        .insert("album".into(), Presence::Value(json!("Valid album")));
    t.issues.push(ImportIssue {
        code: "failed_field".into(),
        record_id: Some("a".into()),
        field: Some("metadata.title".into()),
        message: "Title could not be parsed".into(),
    });
    let i = f.inspect(vec![t], Completeness::PartialWithErrors);
    f.apply(&f.preview(f.request(&i)));
    assert_eq!(f.title(), "A");
    assert_eq!(
        f.lib.tracks(TrackQuery::default()).unwrap().rows[0].album,
        "Valid album"
    );
}

#[test]
fn later_legacy_metadata_edits_and_scalar_clears_remain_local_changes() {
    let f = Fixture::new();
    f.lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,metadata_json,source_hash,state,created,updated) VALUES('existing',?1,'A','{}','hash','ready',1,1)",[crate::model::path_bytes(std::path::Path::new("/Synthetic/existing.flac"))]).unwrap();
    let mut t = track("A");
    t.metadata
        .insert("comment".into(), Presence::Value(json!("Source comment")));
    t.key = Some(MusicalKey {
        presence: FieldPresence::Value,
        raw: Some("8A".into()),
        camelot: Some("8A".into()),
        ..Default::default()
    });
    let i = f.inspect(vec![t.clone()], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.target_items.insert("a".into(), "existing".into());
    f.apply(&f.preview(r));
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET metadata_json=?1,musical_key=NULL WHERE id='existing'",
            [json!({"comment":"Local comment"}).to_string()],
        )
        .unwrap();
    Library::bump(&f.lib.conn().unwrap()).unwrap();
    t.metadata.insert(
        "comment".into(),
        Presence::Value(json!("New source comment")),
    );
    t.key = Some(MusicalKey {
        presence: FieldPresence::Value,
        raw: Some("9A".into()),
        camelot: Some("9A".into()),
        ..Default::default()
    });
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    assert_eq!(p.conflicts, 2);
    f.apply(&p);
    assert_eq!(f.lib.track("existing").unwrap().key, None);
    assert_eq!(
        f.lib.imported_item("existing").unwrap().active_fields["metadata.comment"]["value"],
        "Local comment"
    );
    assert_eq!(
        f.lib.track("existing").unwrap().metadata["comment"],
        "Local comment"
    );
}

#[test]
fn captured_history_and_queue_collections_still_require_adoption_flags() {
    let f = Fixture::new();
    let history = CollectionObservation {
        external_id: "history".into(),
        kind: CollectionKind::History,
        name: "History".into(),
        ..Default::default()
    };
    let queue = CollectionObservation {
        external_id: "queue".into(),
        kind: CollectionKind::Queue,
        name: "Queue".into(),
        ..Default::default()
    };
    let i = f
        .lib
        .inspect_import_with_reader(
            ImportInspectRequest {
                provider: SourceProvider::Mixxx,
                selection: ImportSourceSelection {
                    root: f.root.path().into(),
                    artifacts: vec![ArtifactRequest::stable("source.bin")],
                },
                binding: SourceBindingChoice::Automatic,
                origin: None,
                path_mappings: vec![],
                capture_file_tags: false,
                include_history: true,
                include_queue: true,
            },
            &Reader {
                tracks: vec![],
                collections: vec![history, queue],
                memberships: vec![],
                completeness: Completeness::WholeLibrary,
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let mut r = f.request(&i);
    let p = f.preview(r.clone());
    assert_eq!(p.collections_changed, 0);
    f.apply(&p);
    r.policy.include_history = true;
    r.policy.include_queue = true;
    let p = f.preview(r);
    assert_eq!(p.collections_changed, 2);
    f.apply(&p);
}

use super::reconcile::{Fixture, track};
use crate::*;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::params;
use serde_json::json;

fn physical(f: &Fixture) -> String {
    let id = "existing".to_owned();
    f.lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES(?1,?2,'Local','invented','ready',1,1)",params![id,crate::model::path_bytes(&f.root.path().join("not-present.flac"))]).unwrap();
    id
}
fn hot() -> CueObservation {
    CueObservation {
        external_id: "native-hot".into(),
        family: CueFamily::Hot,
        source_slot: Some(0),
        native_start_seconds: Some(1.0),
        name: Some("Native hot".into()),
        ..Default::default()
    }
}

#[test]
fn observed_duration_and_descriptive_aliases_do_not_require_analysis() {
    let f = Fixture::new();
    let mut t = track("Imported");
    for (name, value) in [
        ("producer", "Invented producer"),
        ("style", "Invented style"),
        ("version", "Invented version"),
        ("catalog-number", "SYN001"),
    ] {
        t.metadata
            .insert(name.into(), Presence::Value(json!(value)));
    }
    t.extensions
        .insert("duration_seconds".into(), json!(123.25));
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut request = f.request(&i);
    request.policy.selected_fields = [FieldGroup::DescriptiveTags].into();
    f.apply(&f.preview(request));
    let item = f.lib.tracks(TrackQuery::default()).unwrap().rows.remove(0);
    let detail = f.lib.imported_item(&item.id).unwrap();
    for name in ["producer", "style", "version", "catalog_number"] {
        assert!(
            detail
                .active_fields
                .contains_key(&format!("metadata.{name}")),
            "missing {name}"
        );
    }
    assert_eq!(item.duration, Some(123.25));
    assert!(f.lib.analysis(&item.id).unwrap().is_none());
    assert!(f.lib.jobs().unwrap().is_empty());
    assert!(
        detail.observations[0]
            .track
            .metadata
            .contains_key("catalog-number")
    );
}
#[test]
fn imported_physical_metadata_advances_the_editor_revision() {
    let f = Fixture::new();
    let id = physical(&f);
    let before: u64 = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT metadata_revision FROM tracks WHERE id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    let i = f.inspect(vec![track("Source title")], Completeness::WholeLibrary);
    let mut request = f.request(&i);
    request.target_items.insert("a".into(), id.clone());
    request.decisions.push(ImportConflictDecision {
        external_id: "a".into(),
        field: "metadata.title".into(),
        decision: ImportDecision::UseSource,
    });
    f.apply(&f.preview(request));
    let after: u64 = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT metadata_revision FROM tracks WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(after > before);
}
#[test]
fn an_empty_later_cue_revision_is_a_local_edit() {
    let f = Fixture::new();
    let id = physical(&f);
    let mut t = track("Source");
    t.cues.push(hot());
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut request = f.request(&i);
    request.target_items.insert("a".into(), id.clone());
    f.apply(&f.preview(request));
    f.lib
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO cue_revisions(id,track_id,json,created) VALUES('later-clear',?1,'[]',2)",
            [&id],
        )
        .unwrap();
    let fields = super::super::reconcile::local_fields(&f.lib.conn().unwrap(), &id).unwrap();
    assert_eq!(fields.get("cues.hot"), Some(&json!([])));
}
#[test]
fn unknown_native_occupancy_blocks_generation_without_blocking_key() {
    let f = Fixture::new();
    let mut t = track("Unknown native slots");
    t.extensions
        .insert("hotcue_occupancy".into(), json!("unknown"));
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut request = f.request(&i);
    request.policy.cue_generation = CueGeneration::Generate;
    request
        .policy
        .analysis_fields
        .insert(AnalysisField::Key, AnalysisAdoption::FillMissing);
    f.apply(&f.preview(request.clone()));
    let item = f.lib.tracks(TrackQuery::default()).unwrap().rows.remove(0);
    let fields = super::super::reconcile::local_fields(&f.lib.conn().unwrap(), &item.id).unwrap();
    let public = f.lib.imported_item(&item.id).unwrap();
    assert!(
        public
            .active_fields
            .keys()
            .all(|field| !field.starts_with('_'))
    );
    assert!(
        public
            .provenance
            .keys()
            .all(|field| !field.starts_with('_'))
    );
    let stages = super::super::processing::planned_stage_blockers(&request.policy, &fields);
    assert!(
        stages
            .iter()
            .find(|s| s.0 == "generation.cues")
            .unwrap()
            .1
            .as_deref()
            .is_some_and(|s| s.contains("Unknown"))
    );
    assert!(
        stages
            .iter()
            .find(|s| s.0 == "analysis.key")
            .unwrap()
            .1
            .is_none()
    );
}
#[test]
fn hot_slot_loops_protect_pads_while_saved_loops_remain_independent() {
    let mut policy = ImportPolicy {
        cue_generation: CueGeneration::Generate,
        ..Default::default()
    };
    let mut fields = std::collections::BTreeMap::from([(
        "cues.loop".into(),
        json!([{"source_slot":0,"flags":{"source_slot_role":"saved_loop"}}]),
    )]);
    assert!(
        super::super::processing::planned_stage_blockers(&policy, &fields)[0]
            .1
            .is_none()
    );
    fields.insert(
        "cues.loop".into(),
        json!([{"source_slot":0,"flags":{"source_slot_role":"hot"}}]),
    );
    assert!(
        super::super::processing::planned_stage_blockers(&policy, &fields)[0]
            .1
            .is_some()
    );
    policy.replace_hotcues = true;
    assert!(
        super::super::processing::planned_stage_blockers(&policy, &fields)[0]
            .1
            .is_none()
    );
}
#[test]
fn unqualified_native_slot_mapping_remains_unknown_occupancy() {
    let mut t = track("Unqualified slot");
    let mut cue = hot();
    cue.flags
        .insert("slot_mapping_qualified".into(), json!(false));
    t.cues.push(cue);
    assert!(super::super::performance::unknown_hotcue_occupancy(&t));
}
#[test]
fn source_holes_and_removed_native_entries_never_become_active_hotcues() {
    let f = Fixture::new();
    let mut t = track("Native arrays");
    for (id, flag) in [("hole", "placeholder"), ("removed", "removed")] {
        let mut cue = hot();
        cue.external_id = id.into();
        cue.flags.insert(flag.into(), json!(true));
        t.cues.push(cue);
    }
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    let item = f.lib.tracks(TrackQuery::default()).unwrap().rows.remove(0);
    let detail = f.lib.imported_item(&item.id).unwrap();
    assert_eq!(detail.active_fields.get("cues.hot"), Some(&json!([])));
    assert_eq!(detail.observations[0].track.cues.len(), 2);
}
#[test]
fn backend_candidate_choice_uses_only_an_immutable_observed_value() {
    let f = Fixture::new();
    let mut t = track("unused");
    t.metadata.remove("title");
    t.extensions.insert("field_candidates".into(),json!({"title":[{"id":"sql:title","surface":"sql","value":"SQL title","normalized_field":"metadata.title","normalized_value":{"presence":"value","value":"SQL title"}},{"id":"json:title","surface":"json","value":"JSON title","normalized_field":"metadata.title","normalized_value":{"presence":"value","value":"JSON title"}}]}));
    t.issues.push(ImportIssue {
        code: "djstudio_surface_disagreement".into(),
        field: Some("title".into()),
        message: "Choose an observed surface".into(),
        ..Default::default()
    });
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut request = f.request(&i);
    request.candidate_choices.push(ImportCandidateChoice {
        external_id: "a".into(),
        field: "title".into(),
        candidate_id: "invented-choice".into(),
    });
    assert!(
        f.lib
            .preview_import(request.clone(), &CancellationToken::default())
            .is_err()
    );
    request.candidate_choices[0].candidate_id = "json:title".into();
    f.apply(&f.preview(request));
    assert_eq!(
        f.lib.tracks(TrackQuery::default()).unwrap().rows[0].title,
        "JSON title"
    );
}

#[test]
fn unknown_occupancy_protects_an_existing_prepared_binding_from_export() {
    let f = Fixture::new();
    let id = physical(&f);
    f.lib
        .conn()
        .unwrap()
        .execute("UPDATE tracks SET analysis_json='{}' WHERE id=?1", [&id])
        .unwrap();
    let mut t = track("Unreadable native pads");
    t.extensions
        .insert("hotcue_occupancy".into(), json!("unknown"));
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut request = f.request(&i);
    request.target_items.insert("a".into(), id.clone());
    f.apply(&f.preview(request));
    let readiness = f.lib.import_export_readiness(&id).unwrap();
    assert!(!readiness.ready);
    assert!(
        readiness
            .reasons
            .iter()
            .any(|r| r.contains("Unknown native hotcue occupancy"))
    );
}

#[test]
fn a_known_empty_source_cannot_clear_prepared_hotcues_by_default() {
    let f = Fixture::new();
    let id = physical(&f);
    let publication = json!({"grid":{"bpm":120.,"startTime":0.},"cues":[{"name":"Keep prepared","startTime":2.,"type":"1","position":0,"color":"blue"}],"implementation_sha256":"invented","inputs_sha256":"invented"});
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET analysis_json=?1 WHERE id=?2",
            params![json!({"publication":publication}).to_string(), id],
        )
        .unwrap();
    let i = f.inspect(
        vec![track("Empty source family")],
        Completeness::WholeLibrary,
    );
    let mut request = f.request(&i);
    request.target_items.insert("a".into(), id.clone());
    let preview = f.preview(request);
    assert!(preview.conflicts > 0 || preview.blocked > 0);
    f.apply(&preview);
    let fields = super::super::reconcile::local_fields(&f.lib.conn().unwrap(), &id).unwrap();
    assert_eq!(fields["cues.hot"]["cues"][0]["name"], "Keep prepared");
}

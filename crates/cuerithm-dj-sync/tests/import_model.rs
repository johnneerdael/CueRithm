use cuerithm_dj_sync::import::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/research/dj-library-import-handover")
}
fn example() -> ObservationEnvelope {
    serde_json::from_slice(
        &std::fs::read(fixture_root().join("fixtures/normalized-import-example.json")).unwrap(),
    )
    .unwrap()
}
fn validate(envelope: &ObservationEnvelope) -> Result<ValidationReport, String> {
    validate_observation(
        &envelope.source,
        &mut envelope.reader(),
        &ImportLimits::default(),
    )
    .map_err(|error| error.to_string())
}

#[test]
fn policy_defaults_do_not_authorize_processing() {
    let policy = ImportPolicy::default();
    assert!(!policy.requires_processing());
    assert!(policy.analysis_fields.is_empty());
    for field in [
        AnalysisField::Key,
        AnalysisField::Energy,
        AnalysisField::Bpm,
        AnalysisField::Beatgrid,
    ] {
        let mut selected = policy.clone();
        selected
            .analysis_fields
            .insert(field, AnalysisAdoption::FillMissing);
        assert!(selected.requires_processing());
        assert!(!selected.metadata_enrichment.enabled);
        assert!(!selected.cleanup.enabled);
        assert_eq!(selected.cue_generation, CueGeneration::Off);
        assert!(!selected.replace_hotcues);
        assert!(selected.write_audio_tags.is_empty());
        assert_eq!(selected.relocation, RelocationPolicy::Off);
        let restored: ImportPolicy =
            serde_json::from_value(serde_json::to_value(&selected).unwrap()).unwrap();
        assert_eq!(selected, restored);
    }
    let mut overwrite = policy;
    overwrite.replace_hotcues = true;
    assert!(
        !overwrite.requires_processing(),
        "replacement permission is not a generation request"
    );
    assert!(!overwrite.include_history && !overwrite.include_queue);
}

#[test]
fn presence_preserves_absent_null_empty_and_deletion() {
    for (input, expected) in [
        (json!({"presence":"absent"}), Presence::Absent),
        (json!({"presence":"null"}), Presence::Null),
        (
            json!({"presence":"empty","value":""}),
            Presence::Empty(json!("")),
        ),
        (
            json!({"presence":"value","value":0}),
            Presence::Value(json!(0)),
        ),
        (json!({"presence":"deleted"}), Presence::Deleted),
    ] {
        let value: Presence<serde_json::Value> = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(value, expected);
        assert_eq!(serde_json::to_value(value).unwrap(), input);
    }
}

#[test]
fn normalized_fixture_is_valid_and_preserves_a_b_a_membership() {
    let mut input = example();
    let mut second = input.tracks[0].clone();
    second.external_id = "202".into();
    input.tracks.push(second);
    let collection = input
        .collections
        .iter_mut()
        .find(|c| !c.occurrences.is_empty())
        .unwrap();
    collection.occurrences.insert(
        1,
        MembershipObservation {
            external_occurrence_id: "902".into(),
            track_external_id: "202".into(),
            position: Some(2),
            ..Default::default()
        },
    );
    let first = collection.occurrences[0].clone();
    let mut repeated = first;
    repeated.external_occurrence_id = "repeat-101".into();
    collection.occurrences.push(repeated);
    let report = validate(&input).unwrap();
    assert_eq!(report.tracks, 2);
    assert!(report.memberships >= 2);
    let cue = &input.tracks[0].cues[0];
    assert_eq!(cue.native_start_seconds, Some(20.0));
    assert_eq!(cue.source_slot, Some(0));
}

#[test]
fn semantic_validation_rejects_duplicate_ids_orphan_and_cycles() {
    let mut input = example();
    input.tracks.push(input.tracks[0].clone());
    assert!(
        validate(&input)
            .unwrap_err()
            .to_string()
            .contains("duplicate track")
    );
    let mut input = example();
    input.collections[1].parent_id = Some("not-a-parent".into());
    assert!(validate(&input).unwrap_err().to_string().contains("parent"));
    let mut input = example();
    input.collections[0].parent_id = Some(input.collections[1].external_id.clone());
    assert!(validate(&input).unwrap_err().to_string().contains("cycle"));
    let mut input = example();
    let collection = input
        .collections
        .iter_mut()
        .find(|c| !c.occurrences.is_empty())
        .unwrap();
    collection
        .occurrences
        .push(collection.occurrences[0].clone());
    assert!(
        validate(&input)
            .unwrap_err()
            .to_string()
            .contains("occurrence")
    );
}

#[test]
fn timing_rejects_guessed_canonical_values_and_preserves_end_only_preroll() {
    let mut input = example();
    let cue = &mut input.tracks[0].cues[0];
    cue.clock_status = ClockStatus::UnresolvedNative;
    assert!(
        validate(&input)
            .unwrap_err()
            .to_string()
            .contains("unresolved")
    );
    let cue = &mut input.tracks[0].cues[0];
    cue.canonical_start_seconds = None;
    cue.canonical_end_seconds = None;
    cue.native_start_seconds = Some(-0.005);
    validate(&input).unwrap();
    let cue = &mut input.tracks[0].cues[0];
    cue.native_start_seconds = None;
    cue.native_end_seconds = Some(4.0);
    validate(&input).unwrap();
    input.tracks[0].bpm.as_mut().unwrap().value = Some(f64::NAN);
    assert!(validate(&input).unwrap_err().to_string().contains("BPM"));
}

#[test]
fn hierarchy_depth_128_is_valid_129_is_rejected() {
    let mut input = example();
    input.collections.clear();
    for n in 0..128 {
        input.collections.push(CollectionObservation {
            external_id: n.to_string(),
            parent_id: (n > 0).then(|| (n - 1).to_string()),
            kind: CollectionKind::Folder,
            name: format!("Folder {n}"),
            ..Default::default()
        });
    }
    validate(&input).unwrap();
    input.collections.push(CollectionObservation {
        external_id: "128".into(),
        parent_id: Some("127".into()),
        ..Default::default()
    });
    assert!(validate(&input).unwrap_err().to_string().contains("depth"));
}

#[test]
fn all_author_manifest_entries_match_original_bytes() {
    let root = fixture_root();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("pack-manifest.json")).unwrap()).unwrap();
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(files.len(), 22);
    let mut paths = BTreeSet::new();
    for entry in files {
        let path = entry["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let bytes = std::fs::read(root.join(path)).unwrap();
        assert_eq!(
            bytes.len() as u64,
            entry["bytes"].as_u64().unwrap(),
            "{path}"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            entry["sha256"].as_str().unwrap(),
            "{path}"
        );
    }
}

#[test]
fn invalid_unicode_normalized_key_returns_an_error() {
    let mut input = example();
    input.tracks[0].key.as_mut().unwrap().camelot = Some("🍎".into());
    assert!(validate(&input).is_err());
}

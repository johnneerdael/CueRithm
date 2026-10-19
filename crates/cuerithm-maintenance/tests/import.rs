use cuerithm_maintenance::{ImportRecord, import_record};
use serde_json::Value;
#[test]
fn approved_metadata_and_collection_gates_match_shipped_import_methods() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-import-v1.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 24);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let target: ImportRecord = serde_json::from_value(case["target"].clone()).unwrap();
        let donor: ImportRecord = serde_json::from_value(case["donor"].clone()).unwrap();
        let unchanged = donor.clone();
        assert_eq!(
            serde_json::to_value(import_record(&target, &donor).unwrap()).unwrap(),
            case["expected"],
            "import case {index}"
        );
        assert_eq!(donor, unchanged);
    }
}
#[test]
fn malformed_collection_and_oversized_imports_are_rejected_without_changing_either_record() {
    let target = ImportRecord::default();
    let mut donor = ImportRecord {
        tempo_markers: vec![serde_json::json!({"bpm":-1,"startTime":0})],
        ..Default::default()
    };
    assert!(import_record(&target, &donor).is_err());
    assert!(target.tempo_markers.is_empty());
    donor.tempo_markers.clear();
    donor.cues = vec![
        serde_json::json!({"type":"1","position":0,"startTime":0,"name":"x".repeat(1_048_577)}),
    ];
    assert!(import_record(&target, &donor).is_err());
}

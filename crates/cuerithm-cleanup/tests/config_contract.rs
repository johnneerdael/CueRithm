use cuerithm_cleanup::CleanupConfig;
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Matrix {
    schema: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    config: serde_json::Value,
    valid: bool,
}

#[test]
fn desktop_cleanup_contract_matches_native_deserialization_and_validation() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/cleanup-contract.json");
    let matrix: Matrix = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(matrix.schema, "cleanup-config-contract-v1");
    assert!(!matrix.cases.is_empty());
    let mut names = BTreeSet::new();
    for case in matrix.cases {
        assert!(
            names.insert(case.name.clone()),
            "Duplicate case: {}",
            case.name
        );
        let result = serde_json::from_value::<CleanupConfig>(case.config)
            .map_err(|error| error.to_string())
            .and_then(|config| config.validate().map_err(|error| error.to_string()));
        assert_eq!(
            result.is_ok(),
            case.valid,
            "Native cleanup contract {}: {result:?}",
            case.name
        );
    }
}

use cuerithm_cleanup::{CleanupConfig, ConfiguredFix, Metadata, MetadataField, SmartFix, apply};
use serde_json::Value;
use std::collections::BTreeSet;
fn metadata(value: &Value) -> Metadata {
    let mut metadata = Metadata::default();
    for (field, value) in value.as_object().unwrap() {
        if let Some(text) = value.as_str().filter(|s| !s.is_empty()) {
            metadata
                .fields
                .insert(field.parse::<MetadataField>().unwrap(), vec![text.into()]);
        }
    }
    metadata
}
#[test]
fn all_eleven_families_match_recorded_reference_or_explicit_non_loss_policy() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/lexicon-v1.json")).unwrap();
    let rows = fixture["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 131);
    let mut families = BTreeSet::new();
    for row in rows {
        let id = row["id"].as_str().unwrap();
        let family = row["family"].as_str().unwrap();
        families.insert(family);
        let mut options = row["options"].as_object().unwrap().clone();
        let ignored_values = options
            .remove("ignore_values")
            .map(|v| serde_json::from_value(v).unwrap())
            .unwrap_or_default();
        if family == "remove_common_text"
            && let Some(kind) = options.remove("kind") {
                options.insert("pattern".into(), kind);
            }
        options.insert("kind".into(), family.into());
        let rule: SmartFix = serde_json::from_value(options.into()).unwrap();
        let config = CleanupConfig {
            fixes: vec![ConfiguredFix {
                id: id.into(),
                enabled: true,
                ignored_values,
                rule,
            }],
            ..Default::default()
        };
        let report = apply(&config, &metadata(&row["input"])).unwrap();
        assert_eq!(report.metadata, metadata(&row["expected"]), "{id}");
        if row["expected"] != row["source_output"] {
            assert!(
                row["policy_deviation"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()),
                "unexplained deviation {id}"
            );
        }
    }
    assert_eq!(families.len(), 11);
}

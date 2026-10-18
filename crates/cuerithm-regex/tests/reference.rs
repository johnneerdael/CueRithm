use cuerithm_regex::Pattern;

#[test]
fn source_helper_regex_outputs_match_with_explicit_strict_validation_policy() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/lexicon-regex-v1.json")).unwrap();
    let mut differences = vec![];
    for case in fixture["cases"].as_array().unwrap() {
        let source = case["source"].as_str().unwrap();
        let text = case["text"].as_str().unwrap();
        let pattern = Pattern::compile(source);
        if case["expect_compile_error"].as_bool().unwrap() {
            assert!(
                pattern.is_err(),
                "Expected strict validation for {source:?}"
            );
        } else {
            let actual = pattern
                .unwrap_or_else(|e| panic!("{source:?}: {e}"))
                .is_match(text)
                .unwrap_or_else(|e| panic!("{source:?} on {text:?}: {e}"));
            if actual != case["source_result"].as_bool().unwrap() {
                differences.push(format!(
                    "{source:?} on {text:?}: actual {actual}, reference {}",
                    case["source_result"]
                ));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

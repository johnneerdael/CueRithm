use cuerithm_library::backup::parse_strict_json_with_budget;

#[test]
fn compact_integer_arrays_stop_at_the_allocation_budget() {
    let bytes = format!("[{}]", vec!["0"; 100_000].join(","));
    let error = parse_strict_json_with_budget(bytes.as_bytes(), 32, 1024 * 1024).unwrap_err();
    assert!(error.to_string().contains("allocation budget"));
    assert!(parse_strict_json_with_budget(b"[0,1,2]", 32, 1024).is_ok());
}

#[test]
fn keys_strings_and_nested_containers_share_one_budget() {
    let bytes = serde_json::to_vec(
        &serde_json::json!({"org.example":{"a":"x".repeat(1024),"b":"y".repeat(1024)}}),
    )
    .unwrap();
    assert!(parse_strict_json_with_budget(&bytes, 32, 4096).is_err());
    assert!(parse_strict_json_with_budget(&bytes, 32, 16384).is_ok());
    assert!(parse_strict_json_with_budget(br#"{"key":1,"key":2}"#, 32, 16384).is_err());
}

use cuerithm_regex::{Pattern, PatternError};
use std::sync::Arc;

#[test]
fn catastrophic_backtracking_and_negative_assertions_report_exhaustion() {
    let input = "a".repeat(512) + "!";
    for pattern in ["(a+)+$", "^(?!(a+)+$)", "^(?!(?!(a+)+$))"] {
        assert!(
            matches!(
                Pattern::compile(pattern).unwrap().is_match(&input),
                Err(PatternError::BudgetExceeded)
            ),
            "{pattern}"
        );
    }
}

#[test]
fn repeated_character_scans_consume_work_instead_of_bypassing_the_limit() {
    let input = "a".repeat(1_048_576);
    assert!(matches!(
        Pattern::compile("^a+$").unwrap().is_match(&input),
        Err(PatternError::BudgetExceeded)
    ));
}

#[test]
fn backreference_comparisons_are_charged_by_captured_length() {
    let regex = regress::Regex::new(r"^(a{100})\1$").unwrap();
    let text: Vec<u16> = "a".repeat(200).encode_utf16().collect();
    assert_eq!(regex.is_match_utf16_bounded(&text, 1_000, false), Ok(true));
    assert!(regex.is_match_utf16_bounded(&text, 150, false).is_err());
}

#[test]
fn concurrent_predicates_have_independent_meters_and_recover_after_errors() {
    let pattern = Arc::new(Pattern::compile("^(a+)+$").unwrap());
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let pattern = pattern.clone();
            std::thread::spawn(move || {
                assert!(matches!(
                    pattern.is_match(&("a".repeat(512) + "!")),
                    Err(PatternError::BudgetExceeded)
                ));
                assert!(pattern.is_match("aaaa").unwrap());
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert!(pattern.is_match("a").unwrap());
}

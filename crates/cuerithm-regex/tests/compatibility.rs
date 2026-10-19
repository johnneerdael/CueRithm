use cuerithm_regex::{Pattern, PatternError};

#[test]
fn first_boolean_test_preserves_ecmascript_flags_and_character_units() {
    for (pattern, text, expected) in [
        ("mix", "Signal mix", true),
        ("mix", "Signal MIX", false),
        ("/mix/i", "Signal MIX", true),
        ("/^drop$/im", "intro\nDROP\noutro", true),
        ("/^intro.drop$/s", "intro\ndrop", true),
        ("/a/gd", "ba", true),
        ("/a/y", "ba", false),
        ("/a/y", "ab", true),
        ("/^.$/", "🎵", false),
        ("/^.$/u", "🎵", true),
        ("/^..$/", "🎵", true),
        ("/^..$/u", "🎵", false),
        (r"/^\uD83C\uDFB5$/", "🎵", true),
        ("/^(?i:mix)$/", "MIX", true),
        ("/🎵/", "A 🎵 B", true),
        ("/🎵/u", "A 🎵 B", true),
        ("/^\\w+$/i", "é", false),
        ("/^\\w+$/iu", "K", true),
        ("/^\\w+$/i", "K", false),
        ("/^[a-z]+$/i", "K", false),
        ("/^[a-z]+$/iu", "K", true),
        ("/^s$/i", "ſ", false),
        ("/^s$/iu", "ſ", true),
        ("/^i$/i", "ı", false),
        ("/^ß$/i", "ẞ", false),
        ("/^ß$/iu", "ẞ", true),
        ("/^µ$/i", "μ", true),
        ("/^(.)\\1$/i", "ſS", false),
        ("/^(.)\\1$/iu", "ſS", true),
        ("/^\\d+$/u", "١٢", false),
        ("/\\p{Letter}+/u", "Δίσκος", true),
        ("/^[\\p{ASCII}&&\\p{Letter}]+$/v", "abc", true),
        ("/^[\\p{ASCII}&&\\p{Letter}]+$/v", "Δίσκος", false),
        ("/(?<=Drop )A\\d+/", "Drop A12", true),
        ("/^(\\w+) \\1$/", "Mix Mix", true),
        ("/^(\\w+) \\1$/", "Mix Other", false),
        ("/^(?!Bootleg)(?=.*Mix).+$/", "Signal Mix", true),
        ("/^(?!Bootleg)(?=.*Mix).+$/", "Bootleg Mix", false),
        (r"/a\/b/i", "A/B", true),
        (r"a\/b", "a/b", true),
        ("", "any", true),
        ("/", "any", true),
        ("\u{feff} /mix/i \u{feff}", "MIX", true),
        ("\u{85}mix\u{85}", "mix", false),
    ] {
        let regex = Pattern::compile(pattern).unwrap_or_else(|e| panic!("{pattern}: {e}"));
        assert_eq!(
            regex.is_match(text).unwrap(),
            expected,
            "{pattern:?} on {text:?}"
        );
        // Source constructs a fresh RegExp per predicate, so g/y never retain
        // lastIndex across different tracks or a second evaluation.
        assert_eq!(
            regex.is_match(text).unwrap(),
            expected,
            "repeated {pattern:?}"
        );
    }
}

#[test]
fn unicode_set_casefolding_precedes_set_operations_and_complement() {
    for (source, text, want) in [
        (r"/^[\p{L}--[A-Z]]$/iv", "a", false),
        (r"/^[[A-Z]&&[a-z]]$/iv", "a", true),
        (r"/^[\P{Lowercase_Letter}]$/iv", "a", false),
        (r"/^[^a]$/iv", "A", false),
        (r"/^[[^a]&&[A]]$/iv", "A", false),
        (r"/^[\q{ab|cd}--\q{AB}]$/iv", "ab", false),
        (r"/^[\q{ab|cd}&&\q{AB}]$/iv", "aB", true),
        (r"/^[\W]$/iv", "K", false),
        (r"/^[\w]$/iv", "K", true),
        (r"/(?<\u{10400}>a)\k<\u{10400}>/", "aa", true),
        ("/(?<𐐀>a)\\k<𐐀>/", "aa", true),
    ] {
        assert_eq!(
            Pattern::compile(source).unwrap().is_match(text).unwrap(),
            want,
            "{source}"
        );
    }
}

#[test]
fn malformed_flags_syntax_and_oversize_inputs_fail_explicitly() {
    for pattern in ["/a/gg", "/a/z", "/a/uv", "/a/ii", "[", "("] {
        assert!(Pattern::compile(pattern).is_err(), "{pattern}");
    }
    assert!(Pattern::compile(&"a".repeat(100)).is_ok());
    assert!(matches!(
        Pattern::compile(&"a".repeat(101)),
        Err(PatternError::PatternTooLong)
    ));
    assert!(
        Pattern::compile(&"🎵".repeat(50))
            .unwrap()
            .is_match(&"🎵".repeat(50))
            .unwrap()
    );
    assert!(matches!(
        Pattern::compile(&"🎵".repeat(51)),
        Err(PatternError::PatternTooLong)
    ));
    assert!(matches!(
        Pattern::compile("a")
            .unwrap()
            .is_match(&"a".repeat(1_048_577)),
        Err(PatternError::InputTooLong)
    ));
}

#[test]
fn sticky_matching_does_not_search_the_remaining_field() {
    let input = "x".repeat(1_000_000) + "a";
    assert!(!Pattern::compile("/a/y").unwrap().is_match(&input).unwrap());
}

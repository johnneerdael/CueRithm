use cuerithm_organize::{
    paths::{PathRules, validate_relative},
    template::*,
};
fn context() -> NamingContext {
    NamingContext {
        artists: vec!["Artist / DJ".into(), "Guest".into()],
        title: Some("A: Title".into()),
        track: Some(7),
        ..Default::default()
    }
}
#[test]
fn typed_plural_tokens_padding_and_optional_segments() {
    let t = Template::parse("{artists|upper}/{track|pad:3} - {title}[ ({version})]").unwrap();
    assert_eq!(
        t.render(
            &context(),
            MissingPolicy::OmitOptional,
            false,
            PathRules::Portable
        )
        .unwrap(),
        Rendered::Path("ARTIST _ DJ, GUEST/007 - A_ Title".into())
    );
}
#[test]
fn missing_policy_never_silently_omits_required_fields() {
    let t = Template::parse("{album}/{title}").unwrap();
    assert!(
        t.render(
            &context(),
            MissingPolicy::OmitOptional,
            false,
            PathRules::Portable
        )
        .is_err()
    );
    assert!(
        t.render(
            &context(),
            MissingPolicy::SkipOptional,
            false,
            PathRules::Portable
        )
        .is_err()
    );
    assert!(matches!(
        t.render(
            &context(),
            MissingPolicy::SkipOptional,
            true,
            PathRules::Portable
        )
        .unwrap(),
        Rendered::Skipped { .. }
    ));
}
#[test]
fn literals_and_functions_cannot_escape_or_execute() {
    for value in [
        "../{title}",
        "/{title}",
        "C:/{title}",
        "a//{title}",
        "CON/{title}",
        "a./{title}",
        "{unknown}",
        "{title|exec}",
        "[[{title}]]",
        "{track|pad:99}",
        "{artists|pad:2}",
    ] {
        assert!(Template::parse(value).is_err(), "accepted {value}");
    }
    for value in [
        ".",
        "..",
        "file. ",
        "LPT1.wav",
        "x\\y",
        "\\\\?\\C:",
        "x/../y",
    ] {
        assert!(
            validate_relative(value, PathRules::Portable).is_err(),
            "accepted {value}"
        );
    }
}

#[test]
fn blank_multivalue_entries_are_missing_in_optional_segments() {
    let mut c = context();
    c.artists = vec![" ".into(), "Guest".into(), "".into()];
    let t = Template::parse("{artist} - {artists}").unwrap();
    assert_eq!(
        t.render(&c, MissingPolicy::Block, false, PathRules::Portable)
            .unwrap(),
        Rendered::Path("Guest - Guest".into())
    );
}
#[test]
fn empty_final_name_stays_blocked_even_when_organization_optional() {
    let t = Template::parse("[{version}]").unwrap();
    assert!(
        t.render(
            &context(),
            MissingPolicy::OmitOptional,
            true,
            PathRules::Portable
        )
        .is_err()
    );
}

#[test]
fn measured_analysis_and_release_year_tokens_are_supported() {
    for token in ["bpm", "energy", "year"] {
        assert!(
            Template::parse(&format!("{{{token}}} - {{title}}")).is_ok(),
            "missing approved naming token: {token}"
        );
    }
    let mut c = context();
    c.bpm_hundredths = Some(12405);
    c.energy = Some(7);
    c.release_date = Some("2026".into());
    assert_eq!(
        Template::parse("{year}/{bpm} - {key}[ {energy}] {title}")
            .unwrap()
            .render(
                &NamingContext {
                    key: Some("8A".into()),
                    ..c.clone()
                },
                MissingPolicy::Block,
                false,
                PathRules::Portable
            )
            .unwrap(),
        Rendered::Path("2026/124.05 - 8A 7 A_ Title".into())
    );
    c.energy = Some(11);
    assert!(
        Template::parse("{title}")
            .unwrap()
            .render(&c, MissingPolicy::SkipOptional, true, PathRules::Portable)
            .is_err()
    );
    c.energy = None;
    c.bpm_hundredths = Some(0);
    assert!(
        Template::parse("[{bpm}] {title}")
            .unwrap()
            .render(&c, MissingPolicy::SkipOptional, true, PathRules::Portable)
            .is_err()
    );
}

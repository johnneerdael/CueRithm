use cuerithm_cleanup::*;
use std::collections::BTreeSet;
fn metadata(fields: &[(MetadataField, &[&str])]) -> Metadata {
    Metadata {
        fields: fields
            .iter()
            .map(|(f, v)| (*f, v.iter().map(|s| s.to_string()).collect()))
            .collect(),
        ..Default::default()
    }
}
fn mapping(from: &str, to: &str) -> CanonicalMapping {
    CanonicalMapping {
        from: from.into(),
        to: Some(to.into()),
    }
}
fn fix(id: &str, rule: SmartFix) -> ConfiguredFix {
    ConfiguredFix {
        id: id.into(),
        enabled: true,
        ignored_values: BTreeSet::new(),
        rule,
    }
}
#[test]
fn aliases_preserve_multiple_contributors_and_only_selected_roles() {
    let input = metadata(&[
        (
            MetadataField::Artist,
            &["Alias", "Earth, Wind & Fire", "LOCKED"],
        ),
        (MetadataField::Remixer, &["Alias"]),
        (MetadataField::Producer, &["Alias"]),
    ]);
    let config = CleanupConfig {
        artists: ArtistRules {
            enabled: true,
            roles: [ContributorRole::Artist, ContributorRole::Remixer].into(),
            mappings: vec![
                mapping("Alias", "Canonical"),
                mapping("Canonical", "Final"),
                mapping("LOCKED", "Wrong"),
            ],
            protected_values: ["LOCKED".into()].into(),
        },
        ..Default::default()
    };
    let report = apply(&config, &input).unwrap();
    assert_eq!(
        report.metadata.fields[&MetadataField::Artist],
        vec!["Final", "Earth, Wind & Fire", "LOCKED"]
    );
    assert_eq!(
        report.metadata.fields[&MetadataField::Remixer],
        vec!["Final"]
    );
    assert_eq!(
        report.metadata.fields[&MetadataField::Producer],
        vec!["Alias"]
    );
    assert_eq!(report.original, input);
}
#[test]
fn genres_map_deduplicate_and_keep_selected_original_detail_as_tags() {
    let input = metadata(&[(
        MetadataField::Genre,
        &["Deep House", "House", "LOCKED", "Deep House"],
    )]);
    let config = CleanupConfig {
        genres: GenreRules {
            enabled: true,
            mappings: vec![mapping("Deep House", "House"), mapping("LOCKED", "Wrong")],
            protected_values: ["LOCKED".into()].into(),
            conversion: Some(GenreConversion {
                category: "Subgenres".into(),
                genres: ["Deep House".into()].into(),
                remove_genre: false,
            }),
        },
        ..Default::default()
    };
    let report = apply(&config, &input).unwrap();
    assert_eq!(
        report.metadata.fields[&MetadataField::Genre],
        vec!["House", "LOCKED"]
    );
    assert_eq!(
        report.metadata.custom_tags,
        vec![CustomTag {
            category: "Subgenres".into(),
            value: "Deep House".into()
        }]
    );
    assert_eq!(report.original, input);
}
#[test]
fn conversion_can_remove_a_genre_without_losing_its_custom_tag() {
    let input = metadata(&[(MetadataField::Genre, &["House", "Techno"])]);
    let mut config = CleanupConfig::default();
    config.genres.enabled = true;
    config.genres.conversion = Some(GenreConversion {
        category: "Genres".into(),
        genres: ["House".into()].into(),
        remove_genre: true,
    });
    let report = apply(&config, &input).unwrap();
    assert_eq!(
        report.metadata.fields[&MetadataField::Genre],
        vec!["Techno"]
    );
    assert_eq!(report.metadata.custom_tags.len(), 1);
}
#[test]
fn canonical_targets_survive_casing_and_repeat_application() {
    let input = metadata(&[(MetadataField::Artist, &["alias"])]);
    let config = CleanupConfig {
        artists: ArtistRules {
            enabled: true,
            mappings: vec![mapping("alias", "DJ BRAND")],
            ..Default::default()
        },
        fixes: vec![fix(
            "case",
            SmartFix::TitleCase {
                field: MetadataField::Artist,
                uppercase: true,
                lowercase: true,
            },
        )],
        ..Default::default()
    };
    let once = apply(&config, &input).unwrap().metadata;
    assert_eq!(once.fields[&MetadataField::Artist], vec!["DJ BRAND"]);
    assert_eq!(apply(&config, &once).unwrap().metadata, once);
}
#[test]
fn rule_order_ignores_and_empty_configuration_are_explicit() {
    let input = metadata(&[(MetadataField::Title, &["signal__track"])]);
    assert_eq!(
        apply(&CleanupConfig::default(), &input).unwrap().metadata,
        input
    );
    let spaces = fix(
        "spaces",
        SmartFix::ReplaceCharacters {
            characters: "_".into(),
        },
    );
    let garbage = fix(
        "garbage",
        SmartFix::RemoveGarbage {
            field: MetadataField::Title,
        },
    );
    let first = CleanupConfig {
        fixes: vec![spaces.clone(), garbage.clone()],
        ..Default::default()
    };
    assert_eq!(
        apply(&first, &input).unwrap().metadata.fields[&MetadataField::Title],
        vec!["signal track"]
    );
    let reverse = CleanupConfig {
        fixes: vec![garbage, spaces],
        ..Default::default()
    };
    assert_eq!(
        apply(&reverse, &input).unwrap().metadata.fields[&MetadataField::Title],
        vec!["signal  track"]
    );
    let input = metadata(&[(MetadataField::Title, &["01. Signal"])]);
    let mut ignored = fix("prefix", SmartFix::RemoveNumberPrefix);
    ignored.ignored_values.insert("01. Signal".into());
    assert_eq!(
        apply(
            &CleanupConfig {
                fixes: vec![ignored],
                ..Default::default()
            },
            &input
        )
        .unwrap()
        .metadata,
        input
    );
}
#[test]
fn invalid_configuration_rejects_cycles_empty_extractors_and_non_text_targets() {
    let mut config = CleanupConfig::default();
    config.artists.enabled = true;
    config.artists.mappings = vec![mapping("A", "B"), mapping("B", "A")];
    assert!(apply(&config, &Metadata::default()).is_err());
    for rule in [
        SmartFix::ExtractArtist {
            separator: String::new(),
            part: 1,
        },
        SmartFix::ExtractArtist {
            separator: " - ".into(),
            part: 0,
        },
        SmartFix::ExtractRemixer {
            word: String::new(),
            remove_title: true,
        },
    ] {
        assert!(
            apply(
                &CleanupConfig {
                    fixes: vec![fix("bad", rule)],
                    ..Default::default()
                },
                &Metadata::default()
            )
            .is_err()
        );
    }
    for field in [
        "location",
        "bpm",
        "key",
        "energy",
        "cue_points",
        "SERATO_MARKERS2",
    ] {
        assert!(field.parse::<MetadataField>().is_err());
    }
    let mut bad = fix("ignored", SmartFix::RemoveNumberPrefix);
    bad.ignored_values.insert(String::new());
    assert!(
        apply(
            &CleanupConfig {
                fixes: vec![bad],
                ..Default::default()
            },
            &Metadata::default()
        )
        .is_err()
    );
}

#[test]
fn inactive_contributor_roles_do_not_inherit_mapping_source_protection() {
    let input = metadata(&[
        (MetadataField::Artist, &["alias"]),
        (MetadataField::Producer, &["alias"]),
    ]);
    let config = CleanupConfig {
        artists: ArtistRules {
            enabled: true,
            mappings: vec![mapping("alias", "Canonical")],
            ..Default::default()
        },
        fixes: vec![fix(
            "producer-case",
            SmartFix::TitleCase {
                field: MetadataField::Producer,
                uppercase: true,
                lowercase: true,
            },
        )],
        ..Default::default()
    };
    let report = apply(&config, &input).unwrap();
    assert_eq!(
        report.metadata.fields[&MetadataField::Artist],
        vec!["Canonical"]
    );
    assert_eq!(
        report.metadata.fields[&MetadataField::Producer],
        vec!["Alias"]
    );
}
#[test]
fn mapping_provenance_names_the_applied_operations() {
    let input = metadata(&[
        (MetadataField::Genre, &["Deep House"]),
        (MetadataField::Artist, &["alias"]),
    ]);
    let config = CleanupConfig {
        genres: GenreRules {
            enabled: true,
            mappings: vec![mapping("Deep House", "House")],
            ..Default::default()
        },
        artists: ArtistRules {
            enabled: true,
            mappings: vec![mapping("alias", "Canonical")],
            ..Default::default()
        },
        ..Default::default()
    };
    let report = apply(&config, &input).unwrap();
    assert!(report.applied_rules.contains(&"genres".to_owned()));
    assert!(report.applied_rules.contains(&"artists".to_owned()));
}
#[test]
fn cleanup_does_not_infer_a_remixer_from_an_unnamed_version() {
    let input = metadata(&[
        (MetadataField::Title, &["Signal ( Remix)"]),
        (MetadataField::Artist, &["Existing Artist"]),
    ]);
    let config = CleanupConfig {
        fixes: vec![fix(
            "remixer",
            SmartFix::ExtractRemixer {
                word: "remix".into(),
                remove_title: true,
            },
        )],
        ..Default::default()
    };
    assert_eq!(apply(&config, &input).unwrap().metadata, input);
}
#[test]
fn garbage_cleanup_is_idempotent_and_preserves_real_text() {
    for title in [
        "Signal () Sound",
        "(( ))",
        "--- Signal --",
        "C++",
        "+Signal",
        "Signal+",
        "(Real Version)",
        "Signal [ ] -",
    ] {
        let input = metadata(&[(MetadataField::Title, &[title])]);
        let config = CleanupConfig {
            fixes: vec![fix(
                "garbage",
                SmartFix::RemoveGarbage {
                    field: MetadataField::Title,
                },
            )],
            ..Default::default()
        };
        let once = apply(&config, &input).unwrap().metadata;
        assert_eq!(apply(&config, &once).unwrap().metadata, once, "{title}");
    }
}

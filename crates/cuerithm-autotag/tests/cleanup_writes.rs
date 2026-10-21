use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, CatalogMatch, CatalogTrack, FieldChange, FrameName, MatchStatus,
    Metadata, MetadataField, TagInput, TagMatchOutcome, TagWritePlan, read_metadata,
};
use cuerithm_cleanup::{
    ArtistRules, CanonicalMapping, CleanupConfig, ConfiguredFix, ContributorRole, GenreConversion,
    GenreRules, SmartFix,
};
use cuerithm_media::{MediaRuntime, file_sha256};
use std::{collections::BTreeSet, path::Path};
fn no_match() -> TagMatchOutcome {
    TagMatchOutcome {
        status: MatchStatus::NoMatch,
        matches: vec![],
        attempted: vec![],
        warnings: vec![],
    }
}
fn config() -> AutotagConfig {
    let mut c = AutotagConfig::default();
    c.options.tags.clear();
    c.write_energy = false;
    c
}
fn input(runtime: &MediaRuntime, path: &Path, cache: &Path) -> TagInput {
    let audio = runtime.decode(path, cache).unwrap();
    TagInput {
        path: path.to_owned(),
        source_sha256: audio.source_sha256,
        media: audio.info,
        duration_seconds: audio.clock.duration_seconds(),
    }
}
fn change(metadata: &Metadata, field: MetadataField, values: &[&str]) -> FieldChange {
    FieldChange {
        field,
        before: metadata.fields.get(&field).cloned().unwrap_or_default(),
        after: values.iter().map(|s| s.to_string()).collect(),
    }
}
fn rule(id: &str, rule: SmartFix) -> ConfiguredFix {
    ConfiguredFix {
        id: id.into(),
        enabled: true,
        ignored_values: BTreeSet::new(),
        rule,
    }
}
fn mapping(from: &str, to: &str) -> CanonicalMapping {
    CanonicalMapping {
        from: from.into(),
        to: Some(to.into()),
    }
}
#[test]
fn all_formats_cleanup_native_values_without_changing_pcm_or_unselected_metadata() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let rows: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let image = std::fs::read(root.join("apps/desktop/src-tauri/icons/32x32.png")).unwrap();
    let cache = d.path().join("缓存");
    let renamed = d.path().join("演奏");
    std::fs::create_dir(&renamed).unwrap();
    for (index, row) in rows.as_array().unwrap().iter().enumerate() {
        let source = d.path().join(row["file"].as_str().unwrap());
        let seed_input = input(&runtime, &source, &cache);
        let before = read_metadata(&source, &seed_input.media).unwrap();
        let edits = vec![
            change(&before, MetadataField::Title, &["signal__track"]),
            change(
                &before,
                MetadataField::Artist,
                &["alias", "Earth, Wind & Fire"],
            ),
            change(&before, MetadataField::Genre, &["Deep House", "House"]),
            change(&before, MetadataField::Album, &["Unchanged Album"]),
            change(&before, MetadataField::Remixer, &["RemixerAlias"]),
            change(&before, MetadataField::Producer, &["ProducerAlias"]),
            change(&before, MetadataField::Composer, &["ComposerAlias"]),
            change(&before, MetadataField::Lyricist, &["LyricistAlias"]),
            change(
                &before,
                MetadataField::Comment,
                &["Contact https://example.com"],
            ),
            change(&before, MetadataField::Grouping, &["Unchanged Group"]),
        ];
        let mut cfg = config();
        cfg.write_energy = true;
        cfg.options.tags = vec![
            cuerithm_autotag::SupportedTag::OtherTags,
            cuerithm_autotag::SupportedTag::AlbumArt,
        ];
        let outcome = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: vec![CatalogMatch::new(
                1.,
                CatalogTrack {
                    platform: "beatport".into(),
                    other: vec![(FrameName::same("CUSTOM_KEEP"), vec!["Untouched".into()])],
                    ..Default::default()
                },
            )],
            attempted: vec![],
            warnings: vec![],
        };
        let staged = TagWritePlan::build(
            seed_input,
            &outcome,
            &cfg,
            &AnalysisTags {
                energy: Some(7),
                ..Default::default()
            },
        )
        .unwrap()
        .with_metadata_edits(edits)
        .unwrap()
        .with_artwork(image.clone())
        .unwrap()
        .stage(&runtime, &cache)
        .unwrap();
        std::fs::copy(&staged.path, &source).unwrap();
        let wrong = renamed.join(format!("track-{index}.bin"));
        std::fs::rename(source, &wrong).unwrap();
        let incoming = input(&runtime, &wrong, &cache);
        let hash = file_sha256(&wrong).unwrap();
        let read = read_metadata(&wrong, &incoming.media)
            .unwrap_or_else(|e| panic!("{}: {e:#}", row["file"]));
        assert_eq!(
            read.fields[&MetadataField::Artist],
            vec!["alias", "Earth, Wind & Fire"],
            "{}",
            row["file"]
        );
        assert_eq!(
            file_sha256(&wrong).unwrap(),
            hash,
            "metadata read mutated input"
        );
        let cleanup = CleanupConfig {
            fixes: vec![
                rule(
                    "spaces",
                    SmartFix::ReplaceCharacters {
                        characters: "_".into(),
                    },
                ),
                rule(
                    "garbage",
                    SmartFix::RemoveGarbage {
                        field: MetadataField::Title,
                    },
                ),
                rule(
                    "urls",
                    SmartFix::RemoveUrls {
                        field: MetadataField::Comment,
                        clear_field: false,
                    },
                ),
            ],
            genres: GenreRules {
                enabled: true,
                mappings: vec![mapping("Deep House", "House")],
                conversion: Some(GenreConversion {
                    category: "Subgenres".into(),
                    genres: ["Deep House".into()].into(),
                    remove_genre: false,
                }),
                ..Default::default()
            },
            artists: ArtistRules {
                enabled: true,
                roles: [
                    ContributorRole::Artist,
                    ContributorRole::Remixer,
                    ContributorRole::Producer,
                    ContributorRole::Composer,
                    ContributorRole::Lyricist,
                ]
                .into(),
                mappings: vec![
                    mapping("alias", "Canonical"),
                    mapping("RemixerAlias", "Remixer"),
                    mapping("ProducerAlias", "Producer"),
                    mapping("ComposerAlias", "Composer"),
                    mapping("LyricistAlias", "Lyricist"),
                ],
                ..Default::default()
            },
        };
        let stage = TagWritePlan::build(incoming, &no_match(), &config(), &AnalysisTags::default())
            .unwrap()
            .with_cleanup(cleanup)
            .unwrap()
            .stage(&runtime, &cache)
            .unwrap();
        let after = read_metadata(&stage.path, &stage.metadata).unwrap();
        assert_eq!(
            runtime.artwork(&stage.path).unwrap().unwrap().bytes,
            image,
            "{} artwork",
            row["file"]
        );
        assert_eq!(
            stage.metadata.tags.get("energy").map(String::as_str),
            Some("7")
        );
        assert_eq!(after.fields[&MetadataField::Title], vec!["signal track"]);
        assert_eq!(
            after.fields[&MetadataField::Artist],
            vec!["Canonical", "Earth, Wind & Fire"]
        );
        assert_eq!(after.fields[&MetadataField::Genre], vec!["House"]);
        assert_eq!(after.fields[&MetadataField::Comment], vec!["Contact"]);
        for (field, value) in [
            (MetadataField::Producer, "Producer"),
            (MetadataField::Composer, "Composer"),
            (MetadataField::Lyricist, "Lyricist"),
            (MetadataField::Remixer, "Remixer"),
            (MetadataField::Album, "Unchanged Album"),
            (MetadataField::Grouping, "Unchanged Group"),
        ] {
            assert_eq!(after.fields[&field], vec![value]);
        }
        assert_eq!(
            stage.cleanup.as_ref().unwrap().metadata.custom_tags.len(),
            1
        );
        assert_eq!(file_sha256(&wrong).unwrap(), hash);
        let source_audio = runtime.decode(&wrong, &cache).unwrap();
        let staged_audio = runtime.decode(&stage.path, &cache).unwrap();
        assert_eq!(
            source_audio.pcm_sha256().unwrap(),
            staged_audio.pcm_sha256().unwrap()
        );
        assert_eq!(source_audio.frames, staged_audio.frames);
        assert_eq!(
            stage.metadata.tags.get("custom_keep").map(String::as_str),
            Some("Untouched")
        );
    }
}
#[test]
fn cleanup_uses_effective_catalog_fields_after_selected_overwrite_policy() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = d.path().join("source.flac");
    std::fs::copy(
        root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &source,
    )
    .unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let mut cfg = config();
    cfg.options.tags = vec![cuerithm_autotag::SupportedTag::Title];
    let outcome = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                title: "CATALOG_TITLE".into(),
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    };
    let cleanup = CleanupConfig {
        fixes: vec![rule(
            "spaces",
            SmartFix::ReplaceCharacters {
                characters: "_".into(),
            },
        )],
        ..Default::default()
    };
    let staged = TagWritePlan::build(
        input(&runtime, &source, d.path()),
        &outcome,
        &cfg,
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_cleanup(cleanup)
    .unwrap()
    .stage(&runtime, d.path())
    .unwrap();
    assert_eq!(
        read_metadata(&staged.path, &staged.metadata)
            .unwrap()
            .fields[&MetadataField::Title],
        vec!["CATALOG TITLE"]
    );
    assert_eq!(
        staged.provenance["matches"][0]["track"]["title"],
        "CATALOG_TITLE"
    );
}
#[test]
fn stale_manual_metadata_patch_is_rejected_before_any_original_write() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = d.path().join("source.flac");
    std::fs::copy(
        root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &source,
    )
    .unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let hash = file_sha256(&source).unwrap();
    let plan = TagWritePlan::build(
        input(&runtime, &source, d.path()),
        &no_match(),
        &config(),
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_metadata_edits(vec![FieldChange {
        field: MetadataField::Title,
        before: vec!["Wrong baseline".into()],
        after: vec!["New title".into()],
    }])
    .unwrap();
    assert!(plan.stage(&runtime, d.path()).is_err());
    assert_eq!(file_sha256(&source).unwrap(), hash);
}

#[test]
fn unchanged_cleanup_preserves_exact_file_bytes_and_id3_comments() {
    use id3::{TagLike, Version, frame::Comment};
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    for row in cases.as_array().unwrap() {
        let source = d.path().join(row["file"].as_str().unwrap());
        let incoming = input(&runtime, &source, d.path());
        let stage = TagWritePlan::build(incoming, &no_match(), &config(), &AnalysisTags::default())
            .unwrap()
            .with_cleanup(CleanupConfig::default())
            .unwrap()
            .stage(&runtime, d.path())
            .unwrap();
        assert_eq!(
            stage.source_before_sha256, stage.source_after_sha256,
            "{} unchanged cleanup rewrote file",
            row["file"]
        );
    }
    // Only the unqualified comment belongs to the cleanup field; other comments
    // and v2.3 itself survive an actual manual edit.
    let source = d.path().join("comments.mp3");
    let mp3 = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["file"].as_str().unwrap().ends_with(".mp3"))
        .unwrap();
    std::fs::copy(d.path().join(mp3["file"].as_str().unwrap()), &source).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("Existing");
    tag.set_artist("Earth, Wind & Fire");
    tag.add_frame(Comment {
        lang: "nld".into(),
        description: "".into(),
        text: "Remove https://example.com".into(),
    });
    tag.add_frame(Comment {
        lang: "eng".into(),
        description: "private".into(),
        text: "Leave https://private.example".into(),
    });
    tag.write_to_path(&source, Version::Id3v23).unwrap();
    let cleanup = CleanupConfig {
        fixes: vec![rule(
            "urls",
            SmartFix::RemoveUrls {
                field: MetadataField::Comment,
                clear_field: false,
            },
        )],
        ..Default::default()
    };
    let stage = TagWritePlan::build(
        input(&runtime, &source, d.path()),
        &no_match(),
        &config(),
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_cleanup(cleanup)
    .unwrap()
    .stage(&runtime, d.path())
    .unwrap();
    let after = id3::Tag::read_from_path(&stage.path).unwrap();
    assert_eq!(after.version(), Version::Id3v23);
    assert_eq!(after.artist(), Some("Earth, Wind & Fire"));
    let comments: Vec<_> = after
        .comments()
        .map(|c| (c.lang.as_str(), c.description.as_str(), c.text.as_str()))
        .collect();
    assert!(comments.contains(&("nld", "", "Remove")));
    assert!(comments.contains(&("eng", "private", "Leave https://private.example")));
}
#[test]
fn cleanup_removes_only_selected_fields_and_respects_catalog_overwrite() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = d.path().join("source.flac");
    std::fs::copy(
        root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &source,
    )
    .unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let incoming = input(&runtime, &source, d.path());
    let before = read_metadata(&source, &incoming.media).unwrap();
    let original = before.fields[&MetadataField::Title].clone();
    let mut cfg = config();
    cfg.options.tags = vec![cuerithm_autotag::SupportedTag::Title];
    cfg.options.overwrite = false;
    cfg.options.overwrite_tags.clear();
    let outcome = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                title: "CATALOG_TITLE".into(),
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    };
    let cleanup = CleanupConfig {
        genres: GenreRules {
            enabled: true,
            mappings: before
                .fields
                .get(&MetadataField::Genre)
                .into_iter()
                .flatten()
                .map(|v| CanonicalMapping {
                    from: v.clone(),
                    to: None,
                })
                .collect(),
            ..Default::default()
        },
        ..Default::default()
    };
    let stage = TagWritePlan::build(incoming, &outcome, &cfg, &AnalysisTags::default())
        .unwrap()
        .with_cleanup(cleanup)
        .unwrap()
        .stage(&runtime, d.path())
        .unwrap();
    let after = read_metadata(&stage.path, &stage.metadata).unwrap();
    assert_eq!(after.fields[&MetadataField::Title], original);
    assert!(!after.fields.contains_key(&MetadataField::Genre));
}

#[test]
fn unrepresentable_native_edit_is_rejected_instead_of_silently_joined() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let mp3 = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["file"].as_str().unwrap().ends_with(".mp3"))
        .unwrap();
    let source = d.path().join(mp3["file"].as_str().unwrap());
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let incoming = input(&runtime, &source, d.path());
    let before = read_metadata(&source, &incoming.media).unwrap();
    let hash = file_sha256(&source).unwrap();
    let plan = TagWritePlan::build(incoming, &no_match(), &config(), &AnalysisTags::default())
        .unwrap()
        .with_metadata_edits(vec![change(
            &before,
            MetadataField::Comment,
            &["first comment", "second comment"],
        )])
        .unwrap();
    assert!(
        plan.stage(&runtime, d.path()).is_err(),
        "scalar comments must not silently merge distinct values"
    );
    assert_eq!(file_sha256(&source).unwrap(), hash);
}

#[test]
fn catalog_artist_replaces_existing_native_asf_contributors_before_cleanup() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let row = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["file"].as_str().unwrap().ends_with(".wma"))
        .unwrap();
    let source = d.path().join(row["file"].as_str().unwrap());
    let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let incoming = input(&rt, &source, d.path());
    let before = read_metadata(&source, &incoming.media).unwrap();
    let seed = TagWritePlan::build(incoming, &no_match(), &config(), &AnalysisTags::default())
        .unwrap()
        .with_metadata_edits(vec![change(
            &before,
            MetadataField::Artist,
            &["Old Artist"],
        )])
        .unwrap()
        .stage(&rt, d.path())
        .unwrap();
    std::fs::copy(&seed.path, &source).unwrap();
    let mut cfg = config();
    cfg.options.tags = vec![cuerithm_autotag::SupportedTag::Artist];
    let outcome = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                artists: vec!["New Artist".into(), "Earth, Wind & Fire".into()],
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    };
    let stage = TagWritePlan::build(
        input(&rt, &source, d.path()),
        &outcome,
        &cfg,
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_cleanup(CleanupConfig::default())
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    assert_eq!(
        read_metadata(&stage.path, &stage.metadata).unwrap().fields[&MetadataField::Artist],
        vec!["New Artist", "Earth, Wind & Fire"]
    );
}

#[test]
fn configured_id3_upgrade_preserves_slashes_in_unselected_contributor_names() {
    use id3::{TagLike, Version};
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let row = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["file"].as_str().unwrap().ends_with(".mp3"))
        .unwrap();
    let path = d.path().join(row["file"].as_str().unwrap());
    let mut tag = id3::Tag::new();
    tag.set_title("Existing");
    tag.set_artist("AC/DC");
    tag.set_text("TCOM", "Composer/Name");
    tag.write_to_path(&path, Version::Id3v23).unwrap();
    let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let mut cfg = config();
    cfg.options.tags = vec![cuerithm_autotag::SupportedTag::Title];
    cfg.options.id3v24 = true;
    let outcome = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                title: "Catalog title".into(),
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    };
    let staged = TagWritePlan::build(
        input(&rt, &path, d.path()),
        &outcome,
        &cfg,
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_cleanup(CleanupConfig::default())
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    let after = read_metadata(&staged.path, &staged.metadata).unwrap();
    assert_eq!(after.fields[&MetadataField::Artist], vec!["AC/DC"]);
    assert_eq!(
        after.fields[&MetadataField::Composer],
        vec!["Composer/Name"]
    );
    assert_eq!(
        id3::Tag::read_from_path(&staged.path).unwrap().version(),
        Version::Id3v24
    );
}

#[test]
fn cleanup_never_rewrites_a_value_the_user_reviewed() {
    use cuerithm_autotag::{MetadataPatch, MetadataTextField, TextEdit};
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = d.path().join("source.flac");
    std::fs::copy(
        root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &source,
    )
    .unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let input = input(&runtime, &source, d.path());
    let before = read_metadata(&source, &input.media).unwrap();
    let mut patch = MetadataPatch::default();
    for (field, value) in [
        (MetadataTextField::Title, "Reviewed_Title"),
        (MetadataTextField::Genre, "reviewed genre"),
    ] {
        patch.text.insert(
            field,
            TextEdit::Set {
                values: vec![value.into()],
            },
        );
    }
    let title_case = |field| SmartFix::TitleCase {
        field,
        uppercase: true,
        lowercase: true,
    };
    let cleanup = CleanupConfig {
        fixes: vec![
            rule(
                "underscores",
                SmartFix::ReplaceCharacters {
                    characters: "_".into(),
                },
            ),
            rule("genre case", title_case(MetadataField::Genre)),
            rule("album case", title_case(MetadataField::Album)),
        ],
        ..Default::default()
    };
    let staged = TagWritePlan::build(input, &no_match(), &config(), &AnalysisTags::default())
        .unwrap()
        .with_metadata_edits(vec![change(
            &before,
            MetadataField::Album,
            &["unreviewed album"],
        )])
        .unwrap()
        .with_editor_patch(patch)
        .unwrap()
        .with_cleanup(cleanup)
        .unwrap()
        .stage(&runtime, d.path())
        .unwrap();
    let after = read_metadata(&staged.path, &staged.metadata).unwrap();
    assert_eq!(after.fields[&MetadataField::Title], vec!["Reviewed_Title"]);
    assert_eq!(after.fields[&MetadataField::Genre], vec!["reviewed genre"]);
    assert_eq!(
        after.fields[&MetadataField::Album],
        vec!["Unreviewed Album"],
        "cleanup still normalizes every field the user did not review"
    );
}

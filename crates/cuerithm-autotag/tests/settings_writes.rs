use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, CatalogMatch, CatalogTrack, MatchStatus, SupportedTag, TagInput,
    TagMatchOutcome, TagWritePlan,
};
use cuerithm_media::MediaRuntime;
use onetagger_tag::{Field, Tag};
use onetagger_tagger::{StylesOptions, TrackNumber};
use std::path::{Path, PathBuf};
fn fixture(extension: &str) -> (tempfile::TempDir, PathBuf, MediaRuntime) {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let path = cases
        .as_array()
        .unwrap()
        .iter()
        .map(|c| d.path().join(c["file"].as_str().unwrap()))
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some(extension))
        .unwrap();
    let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    (d, path, rt)
}
fn source(rt: &MediaRuntime, path: &Path, cache: &Path) -> TagInput {
    let a = rt.decode(path, cache).unwrap();
    TagInput {
        path: path.to_owned(),
        source_sha256: a.source_sha256,
        media: a.info,
        duration_seconds: a.clock.duration_seconds(),
    }
}
fn result() -> TagMatchOutcome {
    TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                genres: vec!["house".into()],
                styles: vec!["techno".into()],
                release_date: chrono::NaiveDate::from_ymd_opt(2024, 2, 29),
                track_number: Some(TrackNumber::Number(3)),
                track_total: Some(12),
                other: vec![(
                    cuerithm_autotag::FrameName::same("CUSTOM_MOOD"),
                    vec!["Warm".into()],
                )],
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    }
}
#[test]
fn genre_style_routes_and_custom_tags_follow_saved_options() {
    let (d, path, rt) = fixture("flac");
    let input = source(&rt, &path, d.path());
    for (option, genre, style) in [
        (StylesOptions::Default, Some("house"), Some("techno")),
        (StylesOptions::OnlyGenres, Some("house"), None),
        (StylesOptions::OnlyStyles, None, Some("techno")),
        (StylesOptions::MergeToGenres, Some("house|techno"), None),
        (StylesOptions::MergeToStyles, None, Some("techno|house")),
        (StylesOptions::StylesToGenre, Some("techno"), None),
        (StylesOptions::GenresToStyle, None, Some("house")),
    ] {
        let mut config = AutotagConfig::default();
        config.options.tags = vec![SupportedTag::Genre, SupportedTag::Style];
        config.options.styles_options = option;
        let staged =
            TagWritePlan::build(input.clone(), &result(), &config, &AnalysisTags::default())
                .unwrap()
                .stage(&rt, d.path())
                .unwrap();
        let wrapper = Tag::load_file(&staged.path, false).unwrap();
        let tag = wrapper.tag();
        assert_eq!(
            tag.get_field(Field::Genre).map(|v| v.join("|")),
            genre.map(str::to_owned)
        );
        assert_eq!(
            tag.get_field(Field::Style).map(|v| v.join("|")),
            style.map(str::to_owned)
        );
    }
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Style, SupportedTag::OtherTags];
    config.options.styles_options = StylesOptions::CustomTag;
    config.options.styles_custom_tag = Some(cuerithm_autotag::FrameName::same("CUSTOM_STYLE"));
    let staged = TagWritePlan::build(input, &result(), &config, &AnalysisTags::default())
        .unwrap()
        .stage(&rt, d.path())
        .unwrap();
    let wrapper = Tag::load_file(&staged.path, false).unwrap();
    assert_eq!(
        wrapper.tag().get_raw("CUSTOM_STYLE"),
        Some(vec!["techno".into()])
    );
    assert_eq!(
        wrapper.tag().get_raw("CUSTOM_MOOD"),
        Some(vec!["Warm".into()])
    );
}
#[test]
fn id3_versions_dates_and_number_padding_are_applied() {
    let (d, path, rt) = fixture("mp3");
    let input = source(&rt, &path, d.path());
    for version in [true, false] {
        let mut config = AutotagConfig::default();
        config.options.tags = vec![
            SupportedTag::ReleaseDate,
            SupportedTag::TrackNumber,
            SupportedTag::TrackTotal,
            SupportedTag::OtherTags,
        ];
        config.options.id3v24 = version;
        config.options.track_number_leading_zeroes = 3;
        let staged =
            TagWritePlan::build(input.clone(), &result(), &config, &AnalysisTags::default())
                .unwrap()
                .stage(&rt, d.path())
                .unwrap();
        let wrapper = Tag::load_file(&staged.path, false).unwrap();
        let Tag::ID3(tag) = wrapper else {
            panic!("Not ID3")
        };
        assert_eq!(tag.id3v24, version);
        use onetagger_tag::TagImpl;
        let date = tag.get_date().unwrap();
        assert_eq!((date.year, date.month, date.day), (2024, Some(2), Some(29)));
        assert_eq!(tag.get_raw("TRCK"), Some(vec!["003/12".into()]));
    }
}
#[test]
fn wav_metadata_is_inside_the_riff_container() {
    let (d, path, rt) = fixture("wav");
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::OtherTags];
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &result(),
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    let bytes = std::fs::read(&staged.path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8,
        bytes.len()
    );
}
#[test]
fn mp4_dates_read_plain_iso_and_publish_date_is_not_silently_ignored() {
    let (d, path, rt) = fixture("m4a");
    {
        let mut tag = Tag::load_file(&path, true).unwrap();
        let t = tag.tag_mut();
        t.set_raw("©day", vec!["2020-01-02".into()], true);
        t.save_file(&path).unwrap();
    }
    let tag = Tag::load_file(&path, false).unwrap();
    let date = tag.tag().get_date().unwrap();
    assert_eq!((date.year, date.month, date.day), (2020, Some(1), Some(2)));
    drop(tag);
    let mut result = result();
    result.matches[0].track.publish_date = chrono::NaiveDate::from_ymd_opt(2025, 3, 4);
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::PublishDate];
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &result,
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    assert!(
        staged
            .metadata
            .tags
            .values()
            .any(|v| v.contains("2025-03-04"))
    );
}
#[test]
fn setting_total_alone_preserves_number_and_meta_marking_is_optional() {
    let (d, path, rt) = fixture("mp3");
    {
        let mut t = Tag::load_file(&path, true).unwrap();
        let tag = t.tag_mut();
        tag.set_track_number("007", None, true);
        tag.save_file(&path).unwrap();
    }
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::TrackTotal];
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &result(),
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    let t = Tag::load_file(&staged.path, false).unwrap();
    assert_eq!(t.tag().get_raw("TRCK"), Some(vec!["007/12".into()]));
    assert!(t.tag().get_raw("CUERITHM_TAGGEDDATE").is_none());
}
#[test]
fn repeated_raw_tag_overwrite_keeps_one_value() {
    let (d, path, rt) = fixture("mp3");
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::OtherTags];
    for text in ["Warm", "Cold", "Bright"] {
        let mut out = result();
        out.matches[0].track.other[0].1 = vec![text.into()];
        let staged = TagWritePlan::build(
            source(&rt, &path, d.path()),
            &out,
            &config,
            &AnalysisTags::default(),
        )
        .unwrap()
        .stage(&rt, d.path())
        .unwrap();
        let t = Tag::load_file(&staged.path, false).unwrap();
        assert_eq!(t.tag().get_raw("CUSTOM_MOOD"), Some(vec![text.into()]));
        std::fs::copy(&staged.path, &path).unwrap();
    }
    let raw = id3::Tag::read_from_path(&path).unwrap();
    assert_eq!(
        raw.extended_texts()
            .filter(|t| t.description == "CUSTOM_MOOD")
            .count(),
        1
    );
}

#[test]
fn id3_duration_uses_one_seconds_to_milliseconds_conversion() {
    for extension in ["mp3", "aac", "aiff", "wav"] {
        let (d, path, rt) = fixture(extension);
        let input = source(&rt, &path, d.path());
        if extension == "aac" {
            assert!(input.duration_seconds.fract() > 0.);
        }
        let expected = (input.duration_seconds * 1000.).round().to_string();
        let mut config = AutotagConfig::default();
        config.options.tags = vec![SupportedTag::Duration];
        let staged = TagWritePlan::build(input, &result(), &config, &AnalysisTags::default())
            .unwrap()
            .stage(&rt, d.path())
            .unwrap();
        let wrapper = Tag::load_file(&staged.path, false).unwrap();
        assert_eq!(
            wrapper.tag().get_raw("TLEN"),
            Some(vec![expected]),
            "{extension}"
        );
    }
}
#[test]
fn track_number_and_total_obey_independent_selection_and_overwrite() {
    for extension in ["mp3", "m4a", "flac"] {
        let (d, path, rt) = fixture(extension);
        {
            let mut wrapper = Tag::load_file(&path, true).unwrap();
            let tag = wrapper.tag_mut();
            tag.set_track_number("7", Some(20), true);
            tag.save_file(&path).unwrap();
        }
        let input = source(&rt, &path, d.path());
        for (tags, overwrite, exceptions, number, total) in [
            (vec![SupportedTag::TrackNumber], true, vec![], "3", "20"),
            (
                vec![SupportedTag::TrackNumber, SupportedTag::TrackTotal],
                false,
                vec![SupportedTag::TrackNumber],
                "3",
                "20",
            ),
            (
                vec![SupportedTag::TrackNumber, SupportedTag::TrackTotal],
                false,
                vec![SupportedTag::TrackTotal],
                "7",
                "12",
            ),
            (
                vec![SupportedTag::TrackNumber, SupportedTag::TrackTotal],
                false,
                vec![],
                "7",
                "20",
            ),
            (vec![SupportedTag::TrackTotal], true, vec![], "7", "12"),
        ] {
            let mut config = AutotagConfig::default();
            config.options.tags = tags;
            config.options.overwrite = overwrite;
            config.options.overwrite_tags = exceptions;
            let staged =
                TagWritePlan::build(input.clone(), &result(), &config, &AnalysisTags::default())
                    .unwrap()
                    .stage(&rt, d.path())
                    .unwrap();
            let wrapper = Tag::load_file(&staged.path, false).unwrap();
            assert_eq!(
                wrapper.tag().get_field(Field::TrackNumber),
                Some(vec![number.into()]),
                "{extension} number"
            );
            assert_eq!(
                wrapper.tag().get_field(Field::TrackTotal),
                Some(vec![total.into()]),
                "{extension} total"
            );
        }
    }
}

#[test]
fn id3_upgrade_preserves_slashes_in_unselected_contributors() {
    use id3::{TagLike, Version};
    let (d, path, rt) = fixture("mp3");
    let mut tag = id3::Tag::new();
    tag.set_title("Original");
    tag.set_artist("AC/DC");
    tag.set_text("TCOM", "Composer/Name");
    tag.write_to_path(&path, Version::Id3v23).unwrap();
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Title];
    config.options.id3v24 = true;
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &result(),
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    let actual = id3::Tag::read_from_path(&staged.path).unwrap();
    assert_eq!(actual.version(), Version::Id3v24);
    assert_eq!(actual.artist(), Some("AC/DC"));
    assert_eq!(
        actual.get("TCOM").unwrap().content().text(),
        Some("Composer/Name")
    );
}

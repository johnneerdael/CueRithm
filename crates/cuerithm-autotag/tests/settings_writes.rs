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

#[test]
fn explicit_overwrite_respects_global_and_selected_field_policy() {
    for extension in ["mp3", "m4a", "flac", "ogg", "opus"] {
        let (d, path, rt) = fixture(extension);
        {
            let mut wrapper = Tag::load_file(&path, true).unwrap_or_else(|_| panic!("{extension}"));
            let tag = wrapper.tag_mut();
            tag.set_explicit(true);
            tag.save_file(&path).unwrap();
        }
        let before = source(&rt, &path, d.path());
        let mut output = result();
        output.matches[0].track.explicit = Some(false);
        let mut config = AutotagConfig::default();
        config.options.tags = vec![SupportedTag::Explicit];
        config.options.overwrite = false;
        let staged = TagWritePlan::build(before, &output, &config, &AnalysisTags::default())
            .unwrap()
            .stage(&rt, d.path())
            .unwrap();
        let original = Tag::load_file(&path, false).unwrap();
        let actual = Tag::load_file(&staged.path, false).unwrap();
        let key = if extension == "mp3" {
            "ITUNESADVISORY"
        } else if extension == "m4a" {
            "rtng"
        } else {
            "COMMENT"
        };
        if extension == "m4a" {
            assert_eq!(
                mp4ameta::Tag::read_from_path(&staged.path)
                    .unwrap()
                    .advisory_rating(),
                Some(mp4ameta::AdvisoryRating::Explicit)
            );
        }
        assert_eq!(
            actual.tag().all_tags().get(key),
            original.tag().all_tags().get(key),
            "{extension}"
        );
    }
}

#[test]
fn unrelated_id3_native_records_survive_selected_title_edit_independently() {
    use id3::frame::{Comment, EncapsulatedObject, Popularimeter, Private, Unknown};
    use id3::{Content, Frame, TagLike, Version};
    let (d, path, rt) = fixture("mp3");
    let mut native = id3::Tag::read_from_path(&path).unwrap_or_default();
    native.add_frame(EncapsulatedObject {
        mime_type: "application/octet-stream".into(),
        filename: String::new(),
        description: "Serato Markers2".into(),
        data: vec![0, 255, 7, 19, 0, 8],
    });
    native.add_frame(Popularimeter {
        user: "DJ fixture".into(),
        rating: 196,
        counter: 237,
    });
    native.add_frame(Private {
        owner_identifier: "Native DJ fixture".into(),
        private_data: vec![255, 0, 19, 27],
    });
    native.add_frame(Comment {
        lang: "eng".into(),
        description: "DJ notes".into(),
        text: "Preserve this exact / note".into(),
    });
    native.add_frame(Comment {
        lang: "fra".into(),
        description: "different owner".into(),
        text: "Deuxième commentaire".into(),
    });
    native.add_frame(Frame::with_content(
        "XDJX",
        Content::Unknown(Unknown {
            data: vec![9, 8, 7, 0, 255],
            version: Version::Id3v24,
        }),
    ));
    native.write_to_path(&path, Version::Id3v24).unwrap();
    let before = id3::Tag::read_from_path(&path).unwrap();
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Title];
    config.options.id3v24 = true;
    let mut output = result();
    output.matches[0].track.title = "New title".into();
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &output,
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    let after = id3::Tag::read_from_path(&staged.path).unwrap();
    for id in ["GEOB", "PRIV", "COMM", "POPM", "XDJX"] {
        assert_eq!(
            after.frames().filter(|f| f.id() == id).collect::<Vec<_>>(),
            before.frames().filter(|f| f.id() == id).collect::<Vec<_>>(),
            "{id}"
        );
    }
    assert_eq!(after.title(), Some("New title"));
}

#[test]
fn malformed_id3_is_refused_without_reconstructing_an_empty_tag() {
    let (d, path, rt) = fixture("mp3");
    let audio = std::fs::read(&path).unwrap();
    // A claimed ID3 body with an invalid frame ID; keep real playable audio behind it.
    let mut bytes = b"ID3\x04\x00\x00\x00\x00\x00\x0b!!!!\x00\x00\x00\x01\x00\x00\xff".to_vec();
    bytes.extend(audio);
    std::fs::write(&path, &bytes).unwrap();
    let input = source(&rt, &path, d.path());
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Title];
    let error = TagWritePlan::build(input, &result(), &config, &AnalysisTags::default())
        .unwrap()
        .stage(&rt, d.path());
    assert!(error.is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn required_selected_field_absence_is_not_reported_as_success() {
    let (d, path, rt) = fixture("flac");
    {
        let mut wrapper = Tag::load_file(&path, true).unwrap();
        wrapper.tag_mut().remove_raw("MOOD");
        wrapper.tag_mut().save_file(&path).unwrap();
    }
    let input = source(&rt, &path, d.path());
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Mood];
    let staged = TagWritePlan::build(input.clone(), &result(), &config, &AnalysisTags::default())
        .unwrap()
        .stage(&rt, d.path())
        .unwrap();
    assert_eq!(staged.provenance["fields"][0]["status"], "unavailable");
    config.required_tags = vec![SupportedTag::Mood];
    let error = TagWritePlan::build(input, &result(), &config, &AnalysisTags::default())
        .unwrap()
        .stage(&rt, d.path())
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("Required selected field unavailable")
    );
}

fn flac_applications(bytes: &[u8]) -> Vec<Vec<u8>> {
    assert_eq!(&bytes[..4], b"fLaC");
    let mut offset = 4;
    let mut applications = vec![];
    loop {
        let header = bytes[offset];
        let len = u32::from_be_bytes([0, bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
            as usize;
        if header & 127 == 2 {
            applications.push(bytes[offset + 4..offset + 4 + len].to_vec());
        }
        offset += 4 + len;
        if header & 128 != 0 {
            break;
        }
    }
    applications
}
#[test]
fn opaque_flac_application_block_survives_selected_title_write() {
    let (d, path, rt) = fixture("flac");
    let bytes = std::fs::read(&path).unwrap();
    let mut offset = 4;
    loop {
        let header = bytes[offset];
        let len = u32::from_be_bytes([0, bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
            as usize;
        if header & 128 != 0 {
            break;
        }
        offset += 4 + len;
    }
    let last_len =
        u32::from_be_bytes([0, bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]) as usize;
    let end = offset + 4 + last_len;
    let mut changed = bytes[..end].to_vec();
    changed[offset] &= 127;
    let payload = b"DJXX\x00\xffopaque native DJ record\x00";
    changed.extend([130, 0, 0, payload.len() as u8]);
    changed.extend(payload);
    changed.extend(&bytes[end..]);
    std::fs::write(&path, &changed).unwrap();
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Title];
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &result(),
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    assert_eq!(
        flac_applications(&std::fs::read(staged.path).unwrap()),
        flac_applications(&changed)
    );
}
fn riff_unknown(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut pos = 12;
    let mut found = vec![];
    while pos + 8 <= bytes.len() {
        let n = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if &bytes[pos..pos + 4] == b"DJXX" {
            found.push(bytes[pos + 8..pos + 8 + n].to_vec());
        }
        pos += 8 + n + (n % 2);
    }
    found
}
#[test]
fn unknown_riff_chunk_survives_selected_title_write() {
    let (d, path, rt) = fixture("wav");
    let mut bytes = std::fs::read(&path).unwrap();
    let payload = b"opaque\x00DJ\xff\x00\x19";
    bytes.extend(b"DJXX");
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    if !payload.len().is_multiple_of(2) {
        bytes.push(0);
    }
    let size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Title];
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &result(),
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    assert_eq!(
        riff_unknown(&std::fs::read(staged.path).unwrap()),
        riff_unknown(&bytes)
    );
}

#[test]
fn per_field_explicit_replacement_keeps_unrelated_comments_and_rating() {
    use id3::{TagLike, Version};
    let (d, path, rt) = fixture("mp3");
    let mut native = id3::Tag::read_from_path(&path).unwrap_or_default();
    native.set_title("Protected title");
    native.add_frame(id3::frame::Comment {
        lang: "eng".into(),
        description: "DJ notes".into(),
        text: "Preserve".into(),
    });
    native.write_to_path(&path, Version::Id3v24).unwrap();
    {
        let mut wrapper = Tag::load_file(&path, true).unwrap();
        let tag = wrapper.tag_mut();
        tag.set_explicit(true);
        tag.set_rating(4, true);
        tag.save_file(&path).unwrap();
    }
    let before = id3::Tag::read_from_path(&path).unwrap();
    let mut output = result();
    output.matches[0].track.explicit = Some(false);
    output.matches[0].track.title = "Replace?".into();
    let mut config = AutotagConfig::default();
    config.options.tags = vec![
        SupportedTag::Title,
        SupportedTag::Genre,
        SupportedTag::Explicit,
    ];
    config.options.overwrite = false;
    config.options.overwrite_tags = vec![SupportedTag::Genre, SupportedTag::Explicit];
    let staged = TagWritePlan::build(
        source(&rt, &path, d.path()),
        &output,
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    let after = id3::Tag::read_from_path(staged.path).unwrap();
    assert_eq!(after.title(), Some("Protected title"));
    assert_eq!(after.genre(), Some("house"));
    assert_eq!(
        after
            .extended_texts()
            .find(|t| t.description == "ITUNESADVISORY")
            .unwrap()
            .value,
        "2"
    );
    for id in ["COMM", "POPM"] {
        assert_eq!(
            after.frames().filter(|f| f.id() == id).collect::<Vec<_>>(),
            before.frames().filter(|f| f.id() == id).collect::<Vec<_>>()
        );
    }
}

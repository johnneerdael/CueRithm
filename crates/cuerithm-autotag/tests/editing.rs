use cuerithm_autotag::*;
use cuerithm_media::MediaRuntime;
use id3::TagLike;
use std::path::Path;
fn fixtures() -> (tempfile::TempDir, MediaRuntime, Vec<std::path::PathBuf>) {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let paths = cases
        .as_array()
        .unwrap()
        .iter()
        .map(|r| d.path().join(r["file"].as_str().unwrap()))
        .collect();
    (
        d,
        MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap(),
        paths,
    )
}
fn stage(
    runtime: &MediaRuntime,
    path: &Path,
    patch: MetadataPatch,
    cache: &Path,
) -> StagedTagWrite {
    let audio = runtime.decode(path, cache).unwrap();
    let input = TagInput {
        path: path.into(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let mut cfg = AutotagConfig::default();
    cfg.options.tags.clear();
    cfg.write_energy = false;
    TagWritePlan::build(
        input,
        &TagMatchOutcome {
            status: MatchStatus::NoMatch,
            matches: vec![],
            attempted: vec![],
            warnings: vec![],
        },
        &cfg,
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_editor_patch(patch)
    .unwrap()
    .stage_prepared(runtime, &audio, cache)
    .unwrap()
}
#[test]
fn typed_editor_preserves_audio_and_date_precision_and_independent_totals_all_formats() {
    let (d, runtime, paths) = fixtures();
    assert_eq!(paths.len(), 14);
    for path in paths {
        let before = runtime.decode(&path, d.path()).unwrap();
        let mut patch = MetadataPatch::default();
        patch.text.insert(
            MetadataTextField::Title,
            TextEdit::Set {
                values: vec!["Reviewed title".into()],
            },
        );
        patch.text.insert(
            MetadataTextField::CatalogNumber,
            TextEdit::Set {
                values: vec!["CAT-123".into()],
            },
        );
        patch.text.insert(
            MetadataTextField::Isrc,
            TextEdit::Set {
                values: vec!["GBTEST1234567".into()],
            },
        );
        patch.release_date = ValueEdit::Set {
            value: TransferDate {
                year: 2026,
                month: Some(10),
                day: None,
            },
        };
        patch.original_date = ValueEdit::Set {
            value: TransferDate {
                year: 2020,
                month: Some(2),
                day: Some(29),
            },
        };
        patch.track_number = ValueEdit::Set { value: 7 };
        patch.track_total = ValueEdit::Set { value: 19 };
        patch.disc_number = ValueEdit::Set { value: 2 };
        patch.disc_total = ValueEdit::Set { value: 3 };
        let staged = stage(&runtime, &path, patch, d.path());
        let snapshot = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
        assert_eq!(
            snapshot.text[&MetadataTextField::Title],
            vec!["Reviewed title"],
            "{}",
            path.display()
        );
        assert_eq!(
            snapshot.release_date,
            Some(TransferDate {
                year: 2026,
                month: Some(10),
                day: None
            }),
            "{}",
            path.display()
        );
        assert_eq!(
            snapshot.original_date,
            Some(TransferDate {
                year: 2020,
                month: Some(2),
                day: Some(29)
            }),
            "{}",
            path.display()
        );
        assert_eq!(
            (
                snapshot.track_number,
                snapshot.track_total,
                snapshot.disc_number,
                snapshot.disc_total
            ),
            (Some(7), Some(19), Some(2), Some(3)),
            "{}",
            path.display()
        );
        assert_eq!(before.pcm_sha256().unwrap(), staged.pcm_sha256);
        assert_eq!(before.frames, staged.frames);
        let mut clear = MetadataPatch::default();
        clear.track_number = ValueEdit::Clear;
        clear.disc_number = ValueEdit::Clear;
        let cleared = stage(&runtime, &staged.path, clear, d.path());
        let final_snapshot = read_metadata_snapshot(&cleared.path, &cleared.metadata).unwrap();
        assert_eq!(
            (
                final_snapshot.track_number,
                final_snapshot.track_total,
                final_snapshot.disc_number,
                final_snapshot.disc_total
            ),
            (None, Some(19), None, Some(3)),
            "{}",
            path.display()
        );
    }
}
#[test]
fn id3_editor_targets_owner_language_and_exact_picture_without_erasing_other_records() {
    use id3::{
        TagLike,
        frame::{Comment, Popularimeter},
    };
    let (d, runtime, paths) = fixtures();
    let path = paths
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "mp3"))
        .unwrap();
    let mut tag = id3::Tag::read_from_path(path).unwrap_or_default();
    tag.add_frame(Popularimeter {
        user: "one".into(),
        rating: 51,
        counter: 99,
    });
    tag.add_frame(Popularimeter {
        user: "two".into(),
        rating: 255,
        counter: 987654321,
    });
    tag.add_frame(Comment {
        lang: "eng".into(),
        description: String::new(),
        text: "English".into(),
    });
    tag.add_frame(Comment {
        lang: "nld".into(),
        description: String::new(),
        text: "Dutch".into(),
    });
    tag.add_frame(id3::frame::ExtendedText {
        description: "OPAQUE_KEEP".into(),
        value: "native value".into(),
    });
    tag.write_to_path(path, id3::Version::Id3v24).unwrap();
    let info = runtime.probe(path).unwrap();
    let before = read_metadata_snapshot(path, &info).unwrap();
    let mut patch = MetadataPatch::default();
    patch.ratings.push(RatingEdit {
        identity: RatingIdentity {
            owner: "one".into(),
        },
        value: ValueEdit::Clear,
    });
    patch.comments.push(CommentEdit {
        identity: before
            .comments
            .iter()
            .find(|c| c.identity.language == "eng")
            .unwrap()
            .identity
            .clone(),
        value: TextEdit::Set {
            values: vec!["Edited English".into()],
        },
    });
    let staged = stage(&runtime, path, patch, d.path());
    let tag = id3::Tag::read_from_path(&staged.path).unwrap();
    let popms = tag
        .frames()
        .filter_map(|f| {
            if let id3::Content::Popularimeter(p) = f.content() {
                Some(p)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(popms.len(), 2);
    assert!(
        popms
            .iter()
            .any(|p| p.user == "one" && p.rating == 0 && p.counter == 99)
    );
    assert!(
        popms
            .iter()
            .any(|p| p.user == "two" && p.rating == 255 && p.counter == 987654321)
    );
    assert!(
        tag.comments()
            .any(|c| c.lang == "eng" && c.text == "Edited English")
    );
    assert!(tag.comments().any(|c| c.lang == "nld" && c.text == "Dutch"));
    assert!(
        tag.extended_texts()
            .any(|v| v.description == "OPAQUE_KEEP" && v.value == "native value")
    );
}

fn picture(seed: u8) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([seed, 10, 20, 255]));
    let mut output = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut output, image::ImageFormat::Png)
        .unwrap();
    output.into_inner()
}
#[test]
fn selected_artwork_edits_compose_by_original_identity_all_formats() {
    let (d, runtime, paths) = fixtures();
    for path in paths {
        let mut add = MetadataPatch::default();
        for seed in [1, 2, 3] {
            add.artwork.push(ArtworkEdit {
                target: None,
                replacement: Some(ArtworkValue {
                    kind: None,
                    description: None,
                    bytes: picture(seed),
                }),
            });
        }
        let staged = stage(&runtime, &path, add, d.path());
        let before = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
        assert_eq!(before.artwork.len(), 3, "{}", path.display());
        let mut edit = MetadataPatch::default();
        edit.artwork.push(ArtworkEdit {
            target: Some(before.artwork[0].identity.clone()),
            replacement: None,
        });
        edit.artwork.push(ArtworkEdit {
            target: Some(before.artwork[1].identity.clone()),
            replacement: Some(ArtworkValue {
                kind: None,
                description: None,
                bytes: picture(9),
            }),
        });
        let edited = stage(&runtime, &staged.path, edit, d.path());
        let after = read_metadata_snapshot(&edited.path, &edited.metadata).unwrap();
        assert_eq!(after.artwork.len(), 2, "{}", path.display());
        assert_ne!(
            after.artwork[0].identity.sha256,
            before.artwork[1].identity.sha256
        );
        assert_eq!(
            after.artwork[1].identity.sha256,
            before.artwork[2].identity.sha256
        );
        assert_eq!(
            metadata_artwork_bytes(&edited.path, &edited.metadata, &after.artwork[1].identity)
                .unwrap(),
            picture(3)
        );
    }
}

#[test]
fn duplicate_native_popm_owners_are_visible_and_block_ambiguous_edits() {
    let (d, runtime, paths) = fixtures();
    let path = paths
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "mp3"))
        .unwrap();
    let mut tag = id3::Tag::read_from_path(path).unwrap_or_default();
    for raw in [51, 255] {
        tag.frames_vec_mut().push(id3::Frame::with_content(
            "POPM",
            id3::Content::Popularimeter(id3::frame::Popularimeter {
                user: "duplicated owner".into(),
                rating: raw,
                counter: u64::from(raw),
            }),
        ));
    }
    tag.write_to_path(path, id3::Version::Id3v24).unwrap();
    let native = read_metadata_snapshot(path, &runtime.probe(path).unwrap()).unwrap();
    assert_eq!(
        native
            .ratings
            .iter()
            .filter(|r| r.identity.owner == "duplicated owner")
            .count(),
        2,
        "native duplicates must remain visible to ambiguity guards"
    );
    let audio = runtime.decode(path, d.path()).unwrap();
    let mut patch = MetadataPatch::default();
    patch.ratings.push(RatingEdit {
        identity: RatingIdentity {
            owner: "duplicated owner".into(),
        },
        value: ValueEdit::Clear,
    });
    let input = TagInput {
        path: path.into(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let mut cfg = AutotagConfig::default();
    cfg.options.tags.clear();
    let result = TagWritePlan::build(
        input,
        &TagMatchOutcome {
            status: MatchStatus::NoMatch,
            matches: vec![],
            attempted: vec![],
            warnings: vec![],
        },
        &cfg,
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_editor_patch(patch)
    .unwrap()
    .stage_prepared(&runtime, &audio, d.path());
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("Duplicate rating owners")
    );
}

#[test]
fn typed_contributors_do_not_flatten_when_editing_id3v23() {
    let (d, runtime, paths) = fixtures();
    let path = paths
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "mp3"))
        .unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("untouched title with / literal slash");
    tag.write_to_path(path, id3::Version::Id3v23).unwrap();
    let mut patch = MetadataPatch::default();
    patch.text.insert(
        MetadataTextField::Artist,
        TextEdit::Set {
            values: vec!["First artist".into(), "Second artist".into()],
        },
    );
    let audio = runtime.decode(path, d.path()).unwrap();
    let input = TagInput {
        path: path.into(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let mut cfg = AutotagConfig::default();
    cfg.options.tags.clear();
    let blocked = TagWritePlan::build(
        input,
        &TagMatchOutcome {
            status: MatchStatus::NoMatch,
            matches: vec![],
            attempted: vec![],
            warnings: vec![],
        },
        &cfg,
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_editor_patch(patch.clone())
    .unwrap()
    .stage_prepared(&runtime, &audio, d.path());
    assert!(
        blocked
            .err()
            .unwrap()
            .to_string()
            .contains("explicitly allow ID3 conversion")
    );
    patch.allow_id3_upgrade = true;
    let staged = stage(&runtime, path, patch, d.path());
    let after = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
    assert_eq!(
        after.text[&MetadataTextField::Artist],
        vec!["First artist", "Second artist"]
    );
    assert_eq!(
        after.text[&MetadataTextField::Title],
        vec!["untouched title with / literal slash"]
    );
}
#[test]
fn unselected_duplicate_opaque_frames_and_flags_survive_a_typed_edit() {
    let (d, runtime, paths) = fixtures();
    let path = paths
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "mp3"))
        .unwrap();
    let mut tag = id3::Tag::new();
    for value in [vec![0, 255, 1, 0], vec![2, 0, 255, 3]] {
        let mut frame = id3::Frame::with_content(
            "XABC",
            id3::Content::Unknown(id3::frame::Unknown {
                data: value,
                version: id3::Version::Id3v24,
            }),
        );
        frame.set_file_alter_preservation(true);
        tag.frames_vec_mut().push(frame);
    }
    tag.write_to_path(path, id3::Version::Id3v24).unwrap();
    let before = id3::Tag::read_from_path(path)
        .unwrap()
        .frames()
        .filter(|f| f.id() == "XABC")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(|f| f.file_alter_preservation()));
    let mut patch = MetadataPatch::default();
    patch.text.insert(
        MetadataTextField::Title,
        TextEdit::Set {
            values: vec!["edited".into()],
        },
    );
    let staged = stage(&runtime, path, patch, d.path());
    let after = id3::Tag::read_from_path(&staged.path)
        .unwrap()
        .frames()
        .filter(|f| f.id() == "XABC")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(before, after);
    assert!(after.iter().all(|f| f.file_alter_preservation()));
}

#[test]
fn serialized_editor_input_cannot_request_internal_undo_record_restoration() {
    let patch:MetadataPatch=serde_json::from_value(serde_json::json!({"restored_ratings":[[{"owner":"other application"},null]],"artwork_order":[1,0]})).unwrap();
    assert!(
        patch.is_noop(),
        "serialized editor input cannot remove rating owners or reset their counters through private Undo policy"
    );
    let encoded = serde_json::to_value(&patch).unwrap();
    assert!(encoded.get("restored_ratings").is_none());
    assert!(encoded.get("artwork_order").is_none());
}

fn auto_tagged_date(path: &Path) -> Option<onetagger_tag::TagDate> {
    onetagger_tag::Tag::load_file(path, false)
        .unwrap()
        .tag()
        .get_date()
}
#[test]
fn release_date_edits_use_the_field_automatic_tagging_reads_and_writes() {
    let (d, runtime, paths) = fixtures();
    for path in paths {
        let wma = path.extension().is_some_and(|e| e == "wma");
        if !wma {
            // The date automatic tagging stored is the date the editor shows.
            let mut tag = onetagger_tag::Tag::load_file(&path, true).unwrap();
            tag.tag_mut().set_date(
                &onetagger_tag::TagDate {
                    year: 2019,
                    month: Some(3),
                    day: Some(8),
                },
                true,
            );
            tag.tag_mut().save_file(&path).unwrap();
            let info = runtime.probe(&path).unwrap();
            assert_eq!(
                read_metadata_snapshot(&path, &info).unwrap().release_date,
                Some(TransferDate {
                    year: 2019,
                    month: Some(3),
                    day: Some(8)
                }),
                "{}",
                path.display()
            );
        }
        let mut patch = MetadataPatch::default();
        patch.allow_id3_upgrade = true;
        patch.release_date = ValueEdit::Set {
            value: TransferDate {
                year: 2026,
                month: Some(10),
                day: Some(5),
            },
        };
        let staged = stage(&runtime, &path, patch, d.path());
        let edited = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
        assert_eq!(
            edited.release_date,
            Some(TransferDate {
                year: 2026,
                month: Some(10),
                day: Some(5)
            }),
            "{}",
            path.display()
        );
        if !wma {
            // Other software reading the automatic-tagging field sees the edit, and no second
            // release field is left behind to contradict it.
            let date = auto_tagged_date(&staged.path).unwrap();
            assert_eq!(
                (date.year, date.month, date.day),
                (2026, Some(10), Some(5)),
                "{}",
                path.display()
            );
            let tags = onetagger_tag::Tag::load_file(&staged.path, false)
                .unwrap()
                .tag()
                .all_tags();
            for key in tags.keys() {
                assert!(
                    !matches!(
                        key.to_ascii_uppercase().as_str(),
                        "TDRL" | "RELEASETIME" | "RELEASEDATE" | "RELEASE_DATE"
                    ),
                    "{} gained {key}",
                    path.display()
                );
            }
        }
        let mut clear = MetadataPatch::default();
        clear.release_date = ValueEdit::Clear;
        let cleared = stage(&runtime, &staged.path, clear, d.path());
        assert_eq!(
            read_metadata_snapshot(&cleared.path, &cleared.metadata)
                .unwrap()
                .release_date,
            None,
            "{}",
            path.display()
        );
        if !wma {
            assert!(
                auto_tagged_date(&cleared.path).is_none(),
                "{}",
                path.display()
            );
        }
    }
}
#[test]
fn id3v23_release_dates_split_across_year_and_day_month_are_shown_whole() {
    let (d, runtime, paths) = fixtures();
    let path = paths
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "mp3"))
        .unwrap();
    let mut tag = id3::Tag::new();
    tag.set_text("TYER", "2019");
    tag.set_text("TDAT", "0803");
    tag.write_to_path(path, id3::Version::Id3v23).unwrap();
    let info = runtime.probe(path).unwrap();
    assert_eq!(
        read_metadata_snapshot(path, &info).unwrap().release_date,
        Some(TransferDate {
            year: 2019,
            month: Some(3),
            day: Some(8)
        })
    );
    let mut patch = MetadataPatch::default();
    patch.allow_id3_upgrade = true;
    patch.release_date = ValueEdit::Set {
        value: TransferDate {
            year: 2026,
            month: None,
            day: None,
        },
    };
    let staged = stage(&runtime, path, patch, d.path());
    let written = id3::Tag::read_from_path(&staged.path).unwrap();
    assert!(
        written.get("TYER").is_none() && written.get("TDAT").is_none(),
        "the old split date must not survive next to the new one"
    );
    assert_eq!(
        read_metadata_snapshot(&staged.path, &staged.metadata)
            .unwrap()
            .release_date,
        Some(TransferDate {
            year: 2026,
            month: None,
            day: None
        })
    );
}

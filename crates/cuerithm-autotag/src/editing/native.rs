use super::*;
use crate::{TransferDate, asf::AsfTag, input::audio_format};
use anyhow::{Context, Result, bail, ensure};
use cuerithm_media::MediaInfo;
use id3::{Content, Frame, TagLike};
use onetagger_tag::{AudioFileFormat as F, Field, Tag, TagImpl};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn text_key(field: MetadataTextField, format: &F) -> &'static str {
    use MetadataTextField::*;
    let (id3, vorbis, mp4, asf) = match field {
        Title => ("TIT2", "TITLE", "©nam", "Title"),
        Artist => ("TPE1", "ARTIST", "©ART", "WM/Artist"),
        Album => ("TALB", "ALBUM", "©alb", "WM/AlbumTitle"),
        AlbumArtist => ("TPE2", "ALBUMARTIST", "aART", "WM/AlbumArtist"),
        Genre => ("TCON", "GENRE", "©gen", "WM/Genre"),
        Style => ("STYLE", "STYLE", "STYLE", "STYLE"),
        Label => ("TPUB", "LABEL", "LABEL", "WM/Publisher"),
        Version => ("TIT3", "SUBTITLE", "desc", "WM/SubTitle"),
        Remixer => ("TPE4", "REMIXER", "REMIXER", "WM/ModifiedBy"),
        Producer => ("PRODUCER", "PRODUCER", "PRODUCER", "WM/Producer"),
        Composer => ("TCOM", "COMPOSER", "©wrt", "WM/Composer"),
        Lyricist => ("TEXT", "LYRICIST", "LYRICIST", "WM/Writer"),
        Grouping => ("TIT1", "GROUPING", "©grp", "WM/ContentGroupDescription"),
        Comment => ("COMM", "COMMENT", "©cmt", "Description"),
        CatalogNumber => (
            "CATALOGNUMBER",
            "CATALOGNUMBER",
            "CATALOGNUMBER",
            "WM/CatalogNo",
        ),
        Isrc => ("TSRC", "ISRC", "ISRC", "WM/ISRC"),
        Key => ("TKEY", "INITIALKEY", "INITIALKEY", "WM/InitialKey"),
    };
    match format {
        F::FLAC | F::OGG => vorbis,
        F::MP4 => mp4,
        F::WMA => asf,
        _ => id3,
    }
}
pub(super) const TEXT_FIELDS: &[MetadataTextField] = &[
    MetadataTextField::Title,
    MetadataTextField::Artist,
    MetadataTextField::Album,
    MetadataTextField::AlbumArtist,
    MetadataTextField::Genre,
    MetadataTextField::Style,
    MetadataTextField::Label,
    MetadataTextField::Version,
    MetadataTextField::Remixer,
    MetadataTextField::Producer,
    MetadataTextField::Composer,
    MetadataTextField::Lyricist,
    MetadataTextField::Grouping,
    MetadataTextField::Comment,
    MetadataTextField::CatalogNumber,
    MetadataTextField::Isrc,
    MetadataTextField::Key,
];
/// Fields that hold a full date. The release date is the one `TagImpl::get_date`/`set_date` use
/// for automatic tagging, so edits and automatic tagging always agree on where it lives.
fn date_keys(format: &F, original: bool) -> &'static [&'static str] {
    match (format, original) {
        (F::FLAC | F::OGG, false) => &["DATE"],
        (F::FLAC | F::OGG, true) => &["ORIGINALDATE"],
        (F::MP4, false) => &["©day"],
        (F::MP4, true) => &["----:com.apple.iTunes:ORIGINALDATE"],
        (F::WMA, false) => &["RELEASETIME", "WM/Year"],
        (F::WMA, true) => &["WM/OriginalReleaseTime"],
        (_, false) => &["TDRC"],
        (_, true) => &["TDOR", "TORY"],
    }
}
/// Every field one edit of a date replaces; ID3v2.3 keeps the release date in `TYER` and `TDAT`.
fn date_owned(format: &F, original: bool) -> &'static [&'static str] {
    match (format, original) {
        (F::FLAC | F::OGG | F::MP4 | F::WMA, _) => date_keys(format, original),
        (_, false) => &["TDRC", "TYER", "TDAT"],
        (_, true) => date_keys(format, original),
    }
}
fn parse_date(value: &str) -> Option<TransferDate> {
    let parts = value.split('T').next()?.split('-').collect::<Vec<_>>();
    if !(1..=3).contains(&parts.len()) {
        return None;
    }
    let year = parts[0].parse().ok()?;
    let month = parts.get(1).map(|v| v.parse::<u8>()).transpose().ok()?;
    let day = parts.get(2).map(|v| v.parse::<u8>()).transpose().ok()?;
    let date = TransferDate { year, month, day };
    super::validate_date(&date).ok()?;
    Some(date)
}
fn date_string(date: &TransferDate) -> String {
    match (date.month, date.day) {
        (Some(m), Some(d)) => format!("{:04}-{m:02}-{d:02}", date.year),
        (Some(m), None) => format!("{:04}-{m:02}", date.year),
        _ => format!("{:04}", date.year),
    }
}
fn first(tag: &dyn TagImpl, key: &str) -> Option<String> {
    tag.get_native(key)?.into_iter().next()
}
fn read_date(tag: &dyn TagImpl, format: &F, original: bool) -> Option<TransferDate> {
    date_keys(format, original)
        .iter()
        .find_map(|k| first(tag, k).and_then(|v| parse_date(&v)))
        .or_else(|| {
            // ID3v2.3 splits the release date across TYER and TDAT; the tag layer knows how to join them.
            let date = if original { None } else { tag.get_date() }?;
            let date = TransferDate {
                year: date.year,
                month: date.month,
                day: date.day,
            };
            super::validate_date(&date).ok()?;
            Some(date)
        })
}
fn number(tag: &dyn TagImpl, key: &str) -> Option<u16> {
    first(tag, key)?
        .split('/')
        .next()?
        .parse::<u16>()
        .ok()
        .filter(|v| *v > 0)
}
fn numbers(tag: &dyn TagImpl, format: &F, disc: bool) -> (Option<u16>, Option<u16>) {
    let (key, total) = match (format, disc) {
        (F::FLAC | F::OGG, false) => ("TRACKNUMBER", "TRACKTOTAL"),
        (F::FLAC | F::OGG, true) => ("DISCNUMBER", "DISCTOTAL"),
        (F::MP4, false) => ("trkn", ""),
        (F::MP4, true) => ("disk", ""),
        (F::WMA, false) => ("WM/TrackNumber", "TRACKTOTAL"),
        (F::WMA, true) => ("WM/PartOfSet", "DISCTOTAL"),
        (_, false) => ("TRCK", ""),
        (_, true) => ("TPOS", ""),
    };
    let values = first(tag, key);
    let n = values
        .as_ref()
        .and_then(|s| s.split('/').next()?.parse::<u16>().ok())
        .filter(|v| *v > 0);
    let t = if total.is_empty() {
        values
            .and_then(|s| s.split('/').nth(1)?.parse::<u16>().ok())
            .filter(|v| *v > 0)
    } else {
        number(tag, total)
    };
    (n, t)
}
fn common(tag: &dyn TagImpl, format: &F) -> MetadataSnapshot {
    let mut text = BTreeMap::new();
    for field in TEXT_FIELDS {
        let values = text_values(tag, *field, format);
        if !values.is_empty() {
            text.insert(*field, values);
        }
    }
    let (track_number, track_total) = numbers(tag, format, false);
    let (disc_number, disc_total) = numbers(tag, format, true);
    let comments = tag
        .get_native(text_key(MetadataTextField::Comment, format))
        .filter(|v| !v.is_empty())
        .map(|values| NativeComment {
            identity: CommentIdentity {
                language: String::new(),
                description: String::new(),
                sha256: digest(&serde_json::to_vec(&values).expect("strings")),
            },
            values,
        })
        .into_iter()
        .collect();
    let rating = tag
        .get_rating()
        .filter(|v| *v > 0)
        .map(|stars| NativeRating {
            identity: RatingIdentity {
                owner: "native".into(),
            },
            raw: first(tag, rating_key(format))
                .and_then(|v| v.parse().ok())
                .unwrap_or(stars),
            counter: 0,
            stars: Some(stars),
        })
        .into_iter()
        .collect();
    MetadataSnapshot {
        text,
        release_date: read_date(tag, format, false),
        original_date: read_date(tag, format, true),
        track_number,
        track_total,
        disc_number,
        disc_total,
        bpm: first(tag, Field::BPM.by_format(format)).and_then(|s| s.parse::<f64>().ok()),
        ratings: rating,
        comments,
        capabilities: BTreeMap::from([(
            "rating_owner".into(),
            if matches!(format, F::MP3 | F::AAC | F::WAV | F::AIFF) {
                "CueRithm"
            } else {
                "native"
            }
            .into(),
        )]),
        ..Default::default()
    }
}
fn comment_sha(lang: &str, desc: &str, text: &str) -> String {
    digest(&serde_json::to_vec(&(lang, desc, text)).expect("serialize strings"))
}
fn art_identity(
    kind: Option<u32>,
    desc: Option<String>,
    bytes: &[u8],
    ordinal: usize,
) -> ArtworkIdentity {
    ArtworkIdentity {
        kind,
        description: desc,
        sha256: digest(bytes),
        ordinal,
    }
}
pub(crate) fn snapshot_tag(wrapper: &Tag, format: &F) -> Result<MetadataSnapshot> {
    let mut snap = common(*wrapper.tag(), format);
    match wrapper {
        Tag::ID3(id3) => {
            snap.capabilities.insert(
                "id3_version".into(),
                if id3.id3v24 {
                    "2.4"
                } else if id3.editor_tag().version() == id3::Version::Id3v22 {
                    "2.2"
                } else {
                    "2.3"
                }
                .into(),
            );
            snap.ratings = id3
                .editor_tag()
                .frames()
                .filter_map(|f| {
                    if let Content::Popularimeter(p) = f.content() {
                        Some(NativeRating {
                            identity: RatingIdentity {
                                owner: p.user.clone(),
                            },
                            raw: p.rating,
                            counter: p.counter,
                            stars: (p.rating > 0).then_some(p.rating.div_ceil(51).min(5)),
                        })
                    } else {
                        None
                    }
                })
                .collect();
            snap.comments = id3
                .editor_tag()
                .comments()
                .map(|c| NativeComment {
                    identity: CommentIdentity {
                        language: c.lang.clone(),
                        description: c.description.clone(),
                        sha256: comment_sha(&c.lang, &c.description, &c.text),
                    },
                    values: vec![c.text.clone()],
                })
                .collect();
            snap.artwork = id3
                .editor_tag()
                .pictures()
                .enumerate()
                .map(|(i, p)| NativeArtwork {
                    identity: art_identity(
                        Some(u8::from(p.picture_type).into()),
                        Some(p.description.clone()),
                        &p.data,
                        i,
                    ),
                    mime: p.mime_type.clone(),
                    bytes: p.data.len() as u64,
                })
                .collect();
            if snap
                .comments
                .iter()
                .filter(|c| c.identity.description.is_empty())
                .count()
                > 1
            {
                snap.capabilities.insert(
                    "comment".into(),
                    "Choose a language and description for this comment".into(),
                );
            }
        }
        Tag::MP4(mp4) => {
            let tag = mp4.editor_tag();
            snap.track_number = tag.track_number().filter(|n| *n > 0);
            snap.track_total = tag.total_tracks().filter(|n| *n > 0);
            snap.disc_number = tag.disc_number().filter(|n| *n > 0);
            snap.disc_total = tag.total_discs().filter(|n| *n > 0);
            snap.artwork = tag
                .artworks()
                .enumerate()
                .map(|(i, p)| NativeArtwork {
                    identity: art_identity(None, None, p.data, i),
                    mime: match p.fmt {
                        mp4ameta::ImgFmt::Jpeg => "image/jpeg",
                        mp4ameta::ImgFmt::Png => "image/png",
                        mp4ameta::ImgFmt::Bmp => "image/bmp",
                    }
                    .into(),
                    bytes: p.data.len() as u64,
                })
                .collect();
        }
        _ => {
            snap.artwork = wrapper
                .tag()
                .get_art()
                .iter()
                .enumerate()
                .map(|(i, p)| NativeArtwork {
                    identity: art_identity(
                        Some(cover_kind(&p.kind)),
                        Some(p.description.clone()),
                        &p.data,
                        i,
                    ),
                    mime: p.mime.clone(),
                    bytes: p.data.len() as u64,
                })
                .collect();
        }
    }
    Ok(snap)
}
pub(crate) fn snapshot_asf(tag: &AsfTag) -> Result<MetadataSnapshot> {
    let mut s = common(tag, &F::WMA);
    s.artwork = tag
        .get_art()
        .iter()
        .enumerate()
        .map(|(i, p)| NativeArtwork {
            identity: art_identity(
                Some(cover_kind(&p.kind)),
                Some(p.description.clone()),
                &p.data,
                i,
            ),
            mime: p.mime.clone(),
            bytes: p.data.len() as u64,
        })
        .collect();
    Ok(s)
}
fn cover_kind(kind: &onetagger_tag::CoverType) -> u32 {
    crate::asf::kind(kind)
}
pub fn read_metadata_snapshot(path: &Path, info: &MediaInfo) -> Result<MetadataSnapshot> {
    let f = audio_format(info)?;
    if f == F::WMA {
        snapshot_asf(&AsfTag::open(path)?)
    } else {
        snapshot_tag(&Tag::load_format(path, &f, true)?, &f)
    }
}
fn text_after(before: Vec<String>, edit: &TextEdit) -> Vec<String> {
    match edit {
        TextEdit::Keep => before,
        TextEdit::Set { values } => values.clone(),
        TextEdit::Clear => vec![],
        TextEdit::Append { values } => {
            let mut out = before;
            for value in values {
                if !out.contains(value) {
                    out.push(value.clone());
                }
            }
            out
        }
    }
}
fn value_after<T: Clone>(before: Option<T>, edit: &ValueEdit<T>) -> Option<T> {
    match edit {
        ValueEdit::Keep => before,
        ValueEdit::Clear => None,
        ValueEdit::Set { value } => Some(value.clone()),
    }
}
fn common_apply(tag: &mut dyn TagImpl, f: &F, patch: &MetadataPatch) -> Result<()> {
    for (field, edit) in &patch.text {
        if matches!(edit, TextEdit::Keep) {
            continue;
        }
        let key = text_key(*field, f);
        let values = text_after(text_values(tag, *field, f), edit);
        tag.set_native(key, values.clone());
        if *field != MetadataTextField::Comment {
            ensure!(
                tag.get_native(key).unwrap_or_default() == values,
                "Container cannot preserve the requested ordered text values for {:?}",
                field
            );
        }

        if *field == MetadataTextField::Artist && *f == F::WMA {
            tag.set_native(
                "Author",
                if values.is_empty() {
                    vec![]
                } else {
                    vec![values.join("; ")]
                },
            );
        }
        if matches!(f, F::FLAC | F::OGG) {
            let alias = match field {
                MetadataTextField::Label => Some("PUBLISHER"),
                MetadataTextField::Version => Some("VERSION"),
                _ => None,
            };
            if let Some(a) = alias
                && tag.get_native(a).is_some()
            {
                tag.set_native(a, values);
            }
        }
    }
    for (original, edit) in [(false, &patch.release_date), (true, &patch.original_date)] {
        if matches!(edit, ValueEdit::Keep) {
            continue;
        }
        for key in date_owned(f, original) {
            tag.remove_raw(key);
        }
        if let ValueEdit::Set { value } = edit {
            let text = date_string(value);
            // Windows reads the year from WM/Year, so a WMA release date keeps both fields.
            if *f == F::WMA && !original {
                tag.set_native("WM/Year", vec![value.year.to_string()]);
                if value.month.is_some() {
                    tag.set_native("RELEASETIME", vec![text]);
                }
            } else {
                tag.set_native(date_keys(f, original)[0], vec![text]);
            }
        }
    }
    if !matches!(patch.bpm, ValueEdit::Keep) {
        tag.set_native(
            Field::BPM.by_format(f),
            match patch.bpm {
                ValueEdit::Set { value } => vec![value.to_string()],
                _ => vec![],
            },
        );
    }
    for (disc, n, t) in [
        (false, &patch.track_number, &patch.track_total),
        (true, &patch.disc_number, &patch.disc_total),
    ] {
        if matches!(n, ValueEdit::Keep) && matches!(t, ValueEdit::Keep) {
            continue;
        }
        let (before_n, before_t) = numbers(tag, f, disc);
        let number = value_after(before_n, n);
        let total = value_after(before_t, t);
        let (key, total_key) = match (f, disc) {
            (F::FLAC | F::OGG, false) => ("TRACKNUMBER", Some("TRACKTOTAL")),
            (F::FLAC | F::OGG, true) => ("DISCNUMBER", Some("DISCTOTAL")),
            (F::WMA, false) => ("WM/TrackNumber", Some("TRACKTOTAL")),
            (F::WMA, true) => ("WM/PartOfSet", Some("DISCTOTAL")),
            (_, false) => ("TRCK", None),
            (_, true) => ("TPOS", None),
        };
        if let Some(tk) = total_key {
            tag.set_native(key, number.map(|v| vec![v.to_string()]).unwrap_or_default());
            tag.set_native(tk, total.map(|v| vec![v.to_string()]).unwrap_or_default());
        } else if *f != F::MP4 {
            tag.set_native(
                key,
                if number.is_none() && total.is_none() {
                    vec![]
                } else {
                    vec![match total {
                        Some(t) => format!("{}/{t}", number.unwrap_or(0)),
                        None => number.unwrap_or(0).to_string(),
                    }]
                },
            );
        }
    }
    Ok(())
}
fn replacement_frame(old: &Frame, content: Content) -> Frame {
    {
        let mut frame = Frame::with_content(old.id(), content).set_encoding(old.encoding());
        frame.set_tag_alter_preservation(old.tag_alter_preservation());
        frame.set_file_alter_preservation(old.file_alter_preservation());
        frame
    }
}
fn id3_apply(id3: &mut onetagger_tag::id3::ID3Tag, patch: &MetadataPatch) -> Result<()> {
    let tag = id3.editor_tag_mut();
    for (identity, record) in &patch.restored_ratings {
        let positions = tag
            .frames_vec()
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                matches!(f.content(),Content::Popularimeter(p)if p.user==identity.owner)
                    .then_some(i)
            })
            .collect::<Vec<_>>();
        ensure!(positions.len() <= 1, "Ambiguous rating owner during Undo");
        if let Some(i) = positions.first() {
            if let Some(record) = record {
                let old = &tag.frames_vec()[*i];
                tag.frames_vec_mut()[*i] = replacement_frame(
                    old,
                    Content::Popularimeter(id3::frame::Popularimeter {
                        user: identity.owner.clone(),
                        rating: record.raw,
                        counter: record.counter,
                    }),
                );
            } else {
                tag.frames_vec_mut().remove(*i);
            }
        } else if let Some(record) = record {
            tag.frames_vec_mut().push(Frame::with_content(
                "POPM",
                Content::Popularimeter(id3::frame::Popularimeter {
                    user: identity.owner.clone(),
                    rating: record.raw,
                    counter: record.counter,
                }),
            ));
        }
    }
    for edit in &patch.ratings {
        let positions = tag
            .frames_vec()
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                matches!(f.content(),Content::Popularimeter(p)if p.user==edit.identity.owner)
                    .then_some(i)
            })
            .collect::<Vec<_>>();
        ensure!(
            positions.len() <= 1,
            "Duplicate rating owners require native repair before editing"
        );
        let raw = match edit.value {
            ValueEdit::Keep => continue,
            ValueEdit::Clear => 0,
            ValueEdit::Set { value } => value * 51,
        };
        if let Some(i) = positions.first() {
            let old = &tag.frames_vec()[*i];
            let Content::Popularimeter(p) = old.content() else {
                unreachable!()
            };
            let mut p = p.clone();
            p.rating = raw;
            let replacement = replacement_frame(old, Content::Popularimeter(p));
            tag.frames_vec_mut()[*i] = replacement;
        } else {
            ensure!(!edit.identity.owner.is_empty(), "Choose a rating owner");
            tag.add_frame(id3::frame::Popularimeter {
                user: edit.identity.owner.clone(),
                rating: raw,
                counter: 0,
            });
        }
    }
    for edit in &patch.comments {
        let positions=tag.frames_vec().iter().enumerate().filter_map(|(i,f)|matches!(f.content(),Content::Comment(c)if c.lang==edit.identity.language&&c.description==edit.identity.description).then_some(i)).collect::<Vec<_>>();
        ensure!(positions.len() <= 1, "Ambiguous native comment identity");
        if matches!(edit.value, TextEdit::Keep) {
            continue;
        }
        if let Some(i) = positions.first() {
            let old = &tag.frames_vec()[*i];
            let Content::Comment(c) = old.content() else {
                unreachable!()
            };
            ensure!(
                comment_sha(&c.lang, &c.description, &c.text) == edit.identity.sha256,
                "Comment changed since preview"
            );
            let values = text_after(vec![c.text.clone()], &edit.value);
            if values.is_empty() {
                tag.frames_vec_mut().remove(*i);
            } else {
                let mut c = c.clone();
                c.text = values.join("; ");
                let frame = replacement_frame(old, Content::Comment(c));
                tag.frames_vec_mut()[*i] = frame;
            }
        } else {
            ensure!(
                edit.identity.sha256.is_empty(),
                "Selected comment is missing"
            );
            let values = text_after(vec![], &edit.value);
            if !values.is_empty() {
                tag.add_frame(id3::frame::Comment {
                    lang: edit.identity.language.clone(),
                    description: edit.identity.description.clone(),
                    text: values.join("; "),
                });
            }
        }
    }
    Ok(())
}
pub(super) fn apply_tag(tag: &mut Tag, f: &F, patch: &MetadataPatch) -> Result<()> {
    patch.validate()?;
    let before = snapshot_tag(tag, f)?;
    if let Tag::ID3(id3) = tag {
        let needs_upgrade = patch.text.iter().any(|(field, edit)| {
            *field != MetadataTextField::Comment
                && text_after(before.text.get(field).cloned().unwrap_or_default(), edit).len() > 1
                && !matches!(edit, TextEdit::Keep)
        }) || matches!(patch.release_date, ValueEdit::Set { .. })
            || matches!(patch.original_date, ValueEdit::Set { .. });
        if !id3.id3v24 && needs_upgrade {
            ensure!(
                patch.allow_id3_upgrade,
                "This edit requires ID3v2.4 for exact ordered values/date precision; explicitly allow ID3 conversion and preview again"
            );
            id3.set_id3v24(true);
        }
    }
    // Common ID3 comments are safe only when there is one unambiguous default.
    let mut patch = patch.clone();
    if let Tag::ID3(_) = tag
        && let Some(edit) = patch.text.remove(&MetadataTextField::Comment)
        && !matches!(edit, TextEdit::Keep)
    {
        let comments = before
            .comments
            .iter()
            .filter(|c| c.identity.description.is_empty())
            .collect::<Vec<_>>();
        ensure!(
            comments.len() <= 1,
            "Choose the exact comment language and description"
        );
        patch.comments.push(CommentEdit {
            identity: comments
                .first()
                .map(|c| c.identity.clone())
                .unwrap_or(CommentIdentity {
                    language: "eng".into(),
                    description: String::new(),
                    sha256: String::new(),
                }),
            value: edit,
        });
    }
    {
        let dyn_tag = tag.tag_mut();
        common_apply(&mut **dyn_tag, f, &patch)?;
    }
    if let Tag::ID3(id3) = tag {
        id3_apply(id3, &patch)?;
    } else {
        let dyn_tag = tag.tag_mut();
        for edit in &patch.ratings {
            ensure!(
                edit.identity.owner == "native",
                "This container has one native rating"
            );
            apply_native_rating(&mut **dyn_tag, f, &edit.value)
        }
        restore_native_ratings(&mut **dyn_tag, f, &patch.restored_ratings)?;
        for edit in &patch.comments {
            ensure!(
                edit.identity.language.is_empty() && edit.identity.description.is_empty(),
                "Named/language comments are ID3-specific"
            );
            let previous = dyn_tag
                .get_native(text_key(MetadataTextField::Comment, f))
                .unwrap_or_default();
            validate_common_comment(&previous, &edit.identity)?;
            let values = text_after(previous, &edit.value);
            dyn_tag.set_native(text_key(MetadataTextField::Comment, f), values);
        }
    }
    if let Tag::MP4(mp4) = tag {
        let m = mp4.editor_tag_mut();
        for (disc, n, t) in [
            (false, &patch.track_number, &patch.track_total),
            (true, &patch.disc_number, &patch.disc_total),
        ] {
            if matches!(n, ValueEdit::Keep) && matches!(t, ValueEdit::Keep) {
                continue;
            }
            let bn = if disc {
                m.disc_number()
            } else {
                m.track_number()
            };
            let bt = if disc {
                m.total_discs()
            } else {
                m.total_tracks()
            };
            let number = value_after(bn, n);
            let total = value_after(bt, t);
            if disc {
                m.remove_disc_number();
                m.remove_total_discs();
                if let Some(n) = number {
                    m.set_disc_number(n)
                }
                if let Some(t) = total {
                    m.set_total_discs(t)
                }
            } else {
                m.remove_track_number();
                m.remove_total_tracks();
                if let Some(n) = number {
                    m.set_track_number(n)
                }
                if let Some(t) = total {
                    m.set_total_tracks(t)
                }
            }
        }
    }
    apply_artwork_tag(tag, f, &patch.artwork)?;
    if let Some(order) = &patch.artwork_order {
        reorder_tag_artwork(tag, order)?;
    }
    Ok(())
}
pub(super) fn apply_asf(tag: &mut AsfTag, patch: &MetadataPatch) -> Result<()> {
    patch.validate()?;
    common_apply(tag, &F::WMA, patch)?;
    for edit in &patch.ratings {
        ensure!(
            edit.identity.owner == "native",
            "ASF exposes one native rating"
        );
        apply_native_rating(tag, &F::WMA, &edit.value)
    }
    restore_native_ratings(tag, &F::WMA, &patch.restored_ratings)?;
    for edit in &patch.comments {
        ensure!(
            edit.identity.language.is_empty() && edit.identity.description.is_empty(),
            "ASF has one common comment"
        );
        let previous = tag.get_native("Description").unwrap_or_default();
        validate_common_comment(&previous, &edit.identity)?;
        let values = text_after(previous, &edit.value);
        tag.set_native("Description", values);
    }
    for edit in ordered_artwork(&patch.artwork) {
        let snapshot = snapshot_asf(tag)?;
        if let Some(target) = &edit.target {
            ensure!(
                snapshot
                    .artwork
                    .get(target.ordinal)
                    .is_some_and(|p| &p.identity == target),
                "Artwork selection changed"
            );
        }
        tag.editor_picture(
            edit.target.as_ref().map(|i| i.ordinal),
            edit.replacement.as_ref(),
        )?;
    }
    if let Some(order) = &patch.artwork_order {
        let images = tag.get_art();
        validate_order(order, images.len())?;
        for (index, original) in order.iter().enumerate() {
            let image = &images[*original];
            tag.editor_picture(
                Some(index),
                Some(&ArtworkValue {
                    kind: Some(cover_kind(&image.kind)),
                    description: Some(image.description.clone()),
                    bytes: image.data.clone(),
                }),
            )?;
        }
    }
    Ok(())
}
fn apply_artwork_tag(tag: &mut Tag, f: &F, edits: &[ArtworkEdit]) -> Result<()> {
    for edit in ordered_artwork(edits) {
        let snapshot = snapshot_tag(tag, f)?;
        if let Some(target) = &edit.target {
            ensure!(
                snapshot
                    .artwork
                    .get(target.ordinal)
                    .is_some_and(|p| &p.identity == target),
                "Artwork changed since preview"
            );
        }
        let replacement = edit
            .replacement
            .as_ref()
            .map(|p| crate::image::Artwork::validate(p.bytes.clone()).map(|image| (p, image)))
            .transpose()?;
        match tag {
            Tag::ID3(t) => {
                let raw = t.editor_tag_mut();
                let positions = raw
                    .frames_vec()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, f)| matches!(f.content(), Content::Picture(_)).then_some(i))
                    .collect::<Vec<_>>();
                let frame = replacement.map(|(p, img)| {
                    Content::Picture(id3::frame::Picture {
                        mime_type: img.mime.into(),
                        picture_type: id3_picture_type(p.kind.unwrap_or(3)),
                        description: p.description.clone().unwrap_or_default(),
                        data: img.bytes,
                    })
                });
                match (edit.target.as_ref(), frame) {
                    (Some(id), Some(content)) => {
                        let index = positions[id.ordinal];
                        let new = replacement_frame(&raw.frames_vec()[index], content);
                        raw.frames_vec_mut()[index] = new;
                    }
                    (Some(id), None) => {
                        raw.frames_vec_mut().remove(positions[id.ordinal]);
                    }
                    (None, Some(content)) => raw
                        .frames_vec_mut()
                        .push(Frame::with_content("APIC", content)),
                    _ => bail!("Artwork change has neither a selection nor an image"),
                }
            }
            Tag::MP4(t) => {
                use mp4ameta::{Img, ImgFmt};
                let raw = t.editor_tag_mut();
                let mut pictures = raw
                    .artworks()
                    .map(|p| Img {
                        fmt: p.fmt,
                        data: p.data.to_vec(),
                    })
                    .collect::<Vec<_>>();
                let new = replacement
                    .map(|(p, img)| {
                        ensure!(
                            p.kind.is_none() && p.description.as_ref().is_none_or(|s| s.is_empty()),
                            "MP4 artwork has no native type or description"
                        );
                        Ok(Img {
                            fmt: match img.mime {
                                "image/jpeg" => ImgFmt::Jpeg,
                                "image/png" => ImgFmt::Png,
                                _ => bail!("MP4 supports PNG or JPEG artwork"),
                            },
                            data: img.bytes,
                        })
                    })
                    .transpose()?;
                match (edit.target.as_ref(), new) {
                    (Some(id), Some(p)) => pictures[id.ordinal] = p,
                    (Some(id), None) => {
                        pictures.remove(id.ordinal);
                    }
                    (None, Some(p)) => pictures.push(p),
                    _ => bail!("Empty artwork edit"),
                };
                raw.set_artworks(pictures);
            }
            Tag::FLAC(t) => {
                use metaflac::block::{Block, BlockType, Picture};
                let raw = t.editor_tag_mut();
                let mut pictures = raw.pictures().cloned().collect::<Vec<_>>();
                let new = replacement
                    .map(|(p, img)| {
                        let mut picture = Picture::new();
                        picture.picture_type = flac_picture_type(p.kind.unwrap_or(3))?;
                        picture.mime_type = img.mime.into();
                        picture.description = p.description.clone().unwrap_or_default();
                        picture.data = img.bytes;
                        Ok::<_, anyhow::Error>(picture)
                    })
                    .transpose()?;
                match (edit.target.as_ref(), new) {
                    (Some(id), Some(p)) => pictures[id.ordinal] = p,
                    (Some(id), None) => {
                        pictures.remove(id.ordinal);
                    }
                    (None, Some(p)) => pictures.push(p),
                    _ => bail!("Empty artwork edit"),
                };
                raw.remove_blocks(BlockType::Picture);
                for p in pictures {
                    raw.push_block(Block::Picture(p));
                }
            }
            Tag::Vorbis(t) => {
                use lofty::ogg::OggPictureStorage;
                use lofty::picture::{MimeType, Picture, PictureInformation, PictureType};
                let raw = t.editor_tag_mut();
                let new = replacement
                    .map(|(p, img)| {
                        let picture = Picture::new_unchecked(
                            PictureType::from_u8(p.kind.unwrap_or(3) as u8),
                            Some(MimeType::from_str(img.mime)),
                            p.description.clone(),
                            img.bytes,
                        );
                        let info = PictureInformation::from_picture(&picture)?;
                        Ok::<_, anyhow::Error>((picture, info))
                    })
                    .transpose()?;
                match (edit.target.as_ref(), new) {
                    (Some(id), Some((p, i))) => raw.set_picture(id.ordinal, p, i),
                    (Some(id), None) => {
                        raw.remove_picture(id.ordinal);
                    }
                    (None, Some((p, i))) => {
                        raw.insert_picture(p, Some(i))?;
                    }
                    _ => bail!("Empty artwork edit"),
                };
            }
        }
    }
    Ok(())
}
pub fn metadata_artwork_bytes(
    path: &Path,
    info: &MediaInfo,
    identity: &ArtworkIdentity,
) -> Result<Vec<u8>> {
    let f = audio_format(info)?;
    let snapshot = read_metadata_snapshot(path, info)?;
    ensure!(
        snapshot
            .artwork
            .get(identity.ordinal)
            .is_some_and(|p| &p.identity == identity),
        "Artwork selection is stale"
    );
    let bytes = if f == F::WMA {
        AsfTag::open(path)?
            .get_art()
            .get(identity.ordinal)
            .context("Missing artwork")?
            .data
            .clone()
    } else {
        let wrapper = Tag::load_format(path, &f, true)?;
        if let Tag::MP4(t) = &wrapper {
            t.editor_tag()
                .artworks()
                .nth(identity.ordinal)
                .context("Missing artwork")?
                .data
                .to_vec()
        } else {
            wrapper
                .tag()
                .get_art()
                .get(identity.ordinal)
                .context("Missing artwork")?
                .data
                .clone()
        }
    };
    ensure!(
        digest(&bytes) == identity.sha256,
        "Artwork changed while reading"
    );
    crate::image::Artwork::validate(bytes.clone())?;
    Ok(bytes)
}

fn id3_picture_type(value: u32) -> id3::frame::PictureType {
    use id3::frame::PictureType::*;
    match value {
        0 => Other,
        1 => Icon,
        2 => OtherIcon,
        3 => CoverFront,
        4 => CoverBack,
        5 => Leaflet,
        6 => Media,
        7 => LeadArtist,
        8 => Artist,
        9 => Conductor,
        10 => Band,
        11 => Composer,
        12 => Lyricist,
        13 => RecordingLocation,
        14 => DuringRecording,
        15 => DuringPerformance,
        16 => ScreenCapture,
        17 => BrightFish,
        18 => Illustration,
        19 => BandLogo,
        20 => PublisherLogo,
        _ => Undefined(value as u8),
    }
}
fn flac_picture_type(value: u32) -> Result<metaflac::block::PictureType> {
    use metaflac::block::PictureType::*;
    Ok(match value {
        0 => Other,
        1 => Icon,
        2 => OtherIcon,
        3 => CoverFront,
        4 => CoverBack,
        5 => Leaflet,
        6 => Media,
        7 => LeadArtist,
        8 => Artist,
        9 => Conductor,
        10 => Band,
        11 => Composer,
        12 => Lyricist,
        13 => RecordingLocation,
        14 => DuringRecording,
        15 => DuringPerformance,
        16 => ScreenCapture,
        17 => BrightFish,
        18 => Illustration,
        19 => BandLogo,
        20 => PublisherLogo,
        _ => bail!("Unknown FLAC picture type"),
    })
}
fn ordered_artwork(edits: &[ArtworkEdit]) -> Vec<&ArtworkEdit> {
    let mut edits = edits.iter().collect::<Vec<_>>();
    edits.sort_by_key(|e| match &e.target {
        Some(t) => (0, std::cmp::Reverse(t.ordinal)),
        None => (1, std::cmp::Reverse(0)),
    });
    edits
}

fn validate_common_comment(values: &[String], identity: &CommentIdentity) -> Result<()> {
    if values.is_empty() {
        ensure!(
            identity.sha256.is_empty(),
            "Selected comment no longer exists"
        );
    } else {
        ensure!(
            digest(&serde_json::to_vec(values)?) == identity.sha256,
            "Selected comment changed since preview"
        );
    }
    Ok(())
}

fn rating_key(format: &F) -> &'static str {
    match format {
        F::WMA => "WM/SharedUserRating",
        F::MP4 => "rate",
        _ => "RATING",
    }
}
fn apply_native_rating(tag: &mut dyn TagImpl, format: &F, value: &ValueEdit<u8>) {
    match value {
        ValueEdit::Keep => {}
        ValueEdit::Clear => tag.remove_raw(rating_key(format)),
        ValueEdit::Set { value } => {
            let raw = if *format == F::WMA {
                [0, 1, 25, 50, 75, 99][*value as usize]
            } else {
                value * 20
            };
            tag.set_native(rating_key(format), vec![raw.to_string()]);
        }
    }
}
fn restore_native_ratings(
    tag: &mut dyn TagImpl,
    format: &F,
    records: &[(RatingIdentity, Option<NativeRating>)],
) -> Result<()> {
    for (identity, record) in records {
        ensure!(
            identity.owner == "native",
            "This container has one native rating"
        );
        if let Some(record) = record {
            tag.set_native(rating_key(format), vec![record.raw.to_string()]);
        } else {
            tag.remove_raw(rating_key(format));
        }
    }
    Ok(())
}

fn validate_order(order: &[usize], len: usize) -> Result<()> {
    ensure!(
        order.len() == len
            && order
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                == (0..len).collect::<std::collections::BTreeSet<_>>(),
        "Invalid guarded artwork restoration order"
    );
    Ok(())
}
fn reorder_tag_artwork(tag: &mut Tag, order: &[usize]) -> Result<()> {
    match tag {
        Tag::ID3(t) => {
            let raw = t.editor_tag_mut();
            let positions = raw
                .frames_vec()
                .iter()
                .enumerate()
                .filter_map(|(i, f)| matches!(f.content(), Content::Picture(_)).then_some(i))
                .collect::<Vec<_>>();
            validate_order(order, positions.len())?;
            let old = positions
                .iter()
                .map(|i| raw.frames_vec()[*i].clone())
                .collect::<Vec<_>>();
            for (i, source) in order.iter().enumerate() {
                raw.frames_vec_mut()[positions[i]] = old[*source].clone();
            }
        }
        Tag::MP4(t) => {
            let raw = t.editor_tag_mut();
            let images = raw
                .artworks()
                .map(|i| mp4ameta::Img {
                    fmt: i.fmt,
                    data: i.data.to_vec(),
                })
                .collect::<Vec<_>>();
            validate_order(order, images.len())?;
            raw.set_artworks(order.iter().map(|i| images[*i].clone()));
        }
        Tag::FLAC(t) => {
            let raw = t.editor_tag_mut();
            let images = raw.pictures().cloned().collect::<Vec<_>>();
            validate_order(order, images.len())?;
            raw.remove_blocks(metaflac::block::BlockType::Picture);
            for i in order {
                raw.push_block(metaflac::block::Block::Picture(images[*i].clone()));
            }
        }
        Tag::Vorbis(t) => {
            use lofty::ogg::OggPictureStorage;
            let raw = t.editor_tag_mut();
            let images = raw.pictures().to_vec();
            validate_order(order, images.len())?;
            for (index, source) in order.iter().enumerate() {
                raw.set_picture(index, images[*source].0.clone(), images[*source].1);
            }
        }
    }
    Ok(())
}

fn text_values(tag: &dyn TagImpl, field: MetadataTextField, format: &F) -> Vec<String> {
    let values = tag.get_native(text_key(field, format)).unwrap_or_default();
    if !values.is_empty() {
        return values;
    }
    let alias = match (field, format) {
        (MetadataTextField::Artist, F::WMA) => Some("Author"),
        (MetadataTextField::Label, F::FLAC | F::OGG) => Some("PUBLISHER"),
        (MetadataTextField::Version, F::FLAC | F::OGG) => Some("VERSION"),
        _ => None,
    };
    alias
        .and_then(|key| tag.get_native(key))
        .unwrap_or_default()
}

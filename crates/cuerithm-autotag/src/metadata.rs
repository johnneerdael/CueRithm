//! Structured human-readable fields; never infer contributors by splitting commas.
use crate::{asf::AsfTag, input::audio_format};
use anyhow::{Result, ensure};
use cuerithm_cleanup::{FieldChange, Metadata, MetadataField};
use cuerithm_media::MediaInfo;
use onetagger_tag::{AudioFileFormat, Tag, TagImpl};
use std::path::Path;

pub fn read_metadata(path: &Path, info: &MediaInfo) -> Result<Metadata> {
    let format = audio_format(info)?;
    if format == AudioFileFormat::WMA {
        read(&AsfTag::open(path)?, &format)
    } else {
        let tag = Tag::load_format(path, &format, true)?;
        read(*tag.tag(), &format)
    }
}
fn binding(field: MetadataField, format: &AudioFileFormat) -> &'static str {
    use MetadataField::*;
    let (id3, vorbis, mp4, asf) = match field {
        Title => ("TIT2", "TITLE", "©nam", "Title"),
        Artist => ("TPE1", "ARTIST", "©ART", "WM/Artist"),
        Album => ("TALB", "ALBUM", "©alb", "WM/AlbumTitle"),
        AlbumArtist => ("TPE2", "ALBUMARTIST", "aART", "WM/AlbumArtist"),
        Genre => ("TCON", "GENRE", "©gen", "WM/Genre"),
        Comment => ("COMM", "COMMENT", "©cmt", "Description"),
        Grouping => ("TIT1", "GROUPING", "©grp", "WM/ContentGroupDescription"),
        Label => ("TPUB", "LABEL", "LABEL", "WM/Publisher"),
        Remixer => ("TPE4", "REMIXER", "REMIXER", "WM/ModifiedBy"),
        Mix => ("TIT3", "VERSION", "desc", "WM/SubTitle"),
        Producer => ("PRODUCER", "PRODUCER", "PRODUCER", "WM/Producer"),
        Composer => ("TCOM", "COMPOSER", "©wrt", "WM/Composer"),
        Lyricist => ("TEXT", "LYRICIST", "LYRICIST", "WM/Writer"),
    };
    match format {
        AudioFileFormat::FLAC | AudioFileFormat::OGG => vorbis,
        AudioFileFormat::MP4 => mp4,
        AudioFileFormat::WMA => asf,
        _ => id3,
    }
}
fn effective_binding(
    field: MetadataField,
    format: &AudioFileFormat,
    tag: &dyn TagImpl,
) -> &'static str {
    let key = binding(field, format);
    if field == MetadataField::Label
        && matches!(format, AudioFileFormat::FLAC | AudioFileFormat::OGG)
        && tag.get_native(key).is_none()
        && tag.get_native("PUBLISHER").is_some()
    {
        return "PUBLISHER";
    }
    if field == MetadataField::Artist
        && *format == AudioFileFormat::WMA
        && tag.get_native(key).is_none()
    {
        return "Author";
    }
    key
}
pub(crate) fn read(tag: &dyn TagImpl, format: &AudioFileFormat) -> Result<Metadata> {
    let mut metadata = Metadata::default();
    for &field in MetadataField::ALL {
        if let Some(values) = tag.get_native(effective_binding(field, format, tag)) {
            metadata.set(field, values);
        }
    }
    metadata.validate()?;
    Ok(metadata)
}
pub(crate) fn edit(
    tag: &mut dyn TagImpl,
    format: &AudioFileFormat,
    changes: &[FieldChange],
) -> Result<()> {
    let before = read(tag, format)?;
    let mut after = before.clone();
    let mut seen = std::collections::BTreeSet::new();
    for change in changes {
        ensure!(seen.insert(change.field), "Duplicate metadata edit");
        ensure!(
            before
                .fields
                .get(&change.field)
                .cloned()
                .unwrap_or_default()
                == change.before,
            "Metadata preview is stale: {:?}",
            change.field
        );
        after.set(change.field, change.after.clone());
    }
    after.validate()?;
    for change in changes {
        if change.before == change.after {
            continue;
        }
        // ASF's content-description Author is scalar. Keep its display form,
        // and retain real contributor boundaries in repeated WM/Artist values.
        if change.field == MetadataField::Artist && *format == AudioFileFormat::WMA {
            tag.set_native("WM/Artist", change.after.clone());
            tag.set_native(
                "Author",
                if change.after.is_empty() {
                    vec![]
                } else {
                    vec![change.after.join("; ")]
                },
            );
        } else {
            let key = effective_binding(change.field, format, tag);
            tag.set_native(key, change.after.clone());
        }
    }
    Ok(())
}

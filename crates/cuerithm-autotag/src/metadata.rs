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
pub fn has_embedded_artwork(path: &Path, info: &MediaInfo) -> Result<bool> {
    let format = audio_format(info)?;
    if format == AudioFileFormat::WMA {
        Ok(AsfTag::open(path)?.has_art())
    } else {
        let tag = Tag::load_format(path, &format, true)?;
        Ok(tag.tag().has_art())
    }
}
/// Native 0–5 stars through the format reader; no normalized rating remains None.
pub fn read_star_rating(path: &Path, info: &MediaInfo) -> Result<Option<u8>> {
    let format = audio_format(info)?;
    let rating = if format == AudioFileFormat::WMA {
        AsfTag::open(path)?.get_rating()
    } else {
        Tag::load_format(path, &format, true)?.tag().get_rating()
    };
    ensure!(
        rating.is_none_or(|n| n <= 5),
        "Native star rating is outside 0–5"
    );
    Ok(rating)
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
        Mix => ("TIT3", "SUBTITLE", "desc", "WM/SubTitle"),
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
    if field == MetadataField::Mix
        && matches!(format, AudioFileFormat::FLAC | AudioFileFormat::OGG)
        && tag.get_native(key).is_none()
        && tag.get_native("VERSION").is_some()
    {
        return "VERSION";
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
        } else if matches!(change.field, MetadataField::Label | MetadataField::Mix)
            && matches!(format, AudioFileFormat::FLAC | AudioFileFormat::OGG)
        {
            // Keep existing aliases synchronized so an edit or clear cannot
            // expose an older value through a different reader.
            let aliases = if change.field == MetadataField::Label {
                ["LABEL", "PUBLISHER"]
            } else {
                ["SUBTITLE", "VERSION"]
            };
            let keys = aliases
                .into_iter()
                .filter(|key| tag.get_native(key).is_some())
                .collect::<Vec<_>>();
            if keys.is_empty() {
                tag.set_native(binding(change.field, format), change.after.clone());
            } else {
                for key in keys {
                    tag.set_native(key, change.after.clone());
                }
            }
        } else {
            let key = effective_binding(change.field, format, tag);
            tag.set_native(key, change.after.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod rating_tests {
    use super::*;
    #[test]
    fn native_stars_round_trip_across_common_layouts_without_changing_pcm() {
        let dir = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
        ))
        .unpack(dir.path())
        .unwrap();
        let cases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("cases.json")).unwrap()).unwrap();
        assert_eq!(cases.as_array().unwrap().len(), 14);
        let runtime =
            cuerithm_media::MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin"))
                .unwrap();
        for row in cases.as_array().unwrap() {
            let path = dir.path().join(row["file"].as_str().unwrap());
            let before = runtime.decode(&path, dir.path()).unwrap();
            let format = audio_format(&before.info).unwrap();
            for stars in [4, 0] {
                if format == AudioFileFormat::WMA {
                    let mut tag = AsfTag::open(&path).unwrap();
                    tag.set_rating(stars, true);
                    tag.save_file(&path).unwrap();
                } else {
                    let mut tag = Tag::load_format(&path, &format, true).unwrap();
                    tag.tag_mut().set_rating(stars, true);
                    tag.tag_mut().save_file(&path).unwrap();
                }
                let info = runtime.probe(&path).unwrap();
                assert_eq!(
                    read_star_rating(&path, &info).unwrap().unwrap_or(0),
                    stars,
                    "{}",
                    path.display()
                );
            }
            let after = runtime.decode(&path, dir.path()).unwrap();
            assert_eq!(
                after.pcm_sha256().unwrap(),
                before.pcm_sha256().unwrap(),
                "{}",
                path.display()
            );
            assert_eq!(after.frames, before.frames);
        }
    }
}

//! Post-write field outcomes distinguish absence from a successfully retained value.
use crate::{SupportedTag, asf::AsfTag, input::audio_format};
use anyhow::Result;
use cuerithm_media::MediaInfo;
use mp4ameta::ident::DataIdent;
use onetagger_tag::{AudioFileFormat, Field, Tag, TagImpl, mp4::MP4Tag};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldWriteStatus {
    Written,
    KeptExisting,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldWriteOutcome {
    pub field: SupportedTag,
    pub status: FieldWriteStatus,
    pub required: bool,
}

pub(crate) fn explicit_present(tag: &dyn TagImpl, format: &AudioFileFormat) -> bool {
    match format {
        AudioFileFormat::MP4 => tag.all_tags().contains_key("rtng"),
        AudioFileFormat::FLAC | AudioFileFormat::OGG => tag
            .get_raw("COMMENT")
            .is_some_and(|v| v.iter().any(|s| s == "Explicit")),
        _ => tag.get_raw("ITUNESADVISORY").is_some(),
    }
}
fn value(
    tag: &dyn TagImpl,
    selected: SupportedTag,
    format: &AudioFileFormat,
    other_bindings: &[String],
) -> Option<Vec<String>> {
    let field = match selected {
        SupportedTag::Title => Some(Field::Title),
        SupportedTag::Artist => Some(Field::Artist),
        SupportedTag::Album => Some(Field::Album),
        SupportedTag::Key => Some(Field::Key),
        SupportedTag::BPM => Some(Field::BPM),
        SupportedTag::Genre => Some(Field::Genre),
        SupportedTag::Style => Some(Field::Style),
        SupportedTag::AlbumArtist => Some(Field::AlbumArtist),
        SupportedTag::Label => Some(Field::Label),
        SupportedTag::ISRC => Some(Field::ISRC),
        SupportedTag::CatalogNumber => Some(Field::CatalogNumber),
        SupportedTag::Version => Some(Field::Version),
        SupportedTag::Duration => Some(Field::Duration),
        SupportedTag::Remixer => Some(Field::Remixer),
        SupportedTag::TrackNumber => Some(Field::TrackNumber),
        SupportedTag::TrackTotal => Some(Field::TrackTotal),
        SupportedTag::DiscNumber => Some(Field::DiscNumber),
        SupportedTag::Mood => Some(Field::Mood),
        _ => None,
    };
    if let Some(field) = field {
        return tag
            .get_field(field)
            .filter(|v| v.iter().any(|s| !s.trim().is_empty()));
    }
    match selected {
        SupportedTag::ReleaseDate => {
            let keys: &[&str] = match format {
                AudioFileFormat::FLAC | AudioFileFormat::OGG => {
                    &["RELEASETIME", "RELEASEDATE", "RELEASE_DATE"]
                }
                AudioFileFormat::MP4 => &["----:com.apple.iTunes:RELEASETIME"],
                AudioFileFormat::WMA => &["RELEASETIME"],
                _ => &["TDRL"],
            };
            keys.iter()
                .find_map(|key| tag.get_native(key))
                .or_else(|| tag.get_date().map(|d| vec![format!("{d:?}")]))
        }
        SupportedTag::PublishDate => tag.get_raw(match format {
            AudioFileFormat::FLAC | AudioFileFormat::OGG => "ORIGINALDATE",
            AudioFileFormat::MP4 => "com.apple.iTunes:PUBLISHTIME",
            AudioFileFormat::WMA => "PUBLISHTIME",
            _ => "TDRL",
        }),
        SupportedTag::AlbumArt => (!tag.get_art().is_empty()).then(|| {
            tag.get_art()
                .iter()
                .map(|p| {
                    use sha2::{Digest, Sha256};
                    format!("{:x}", Sha256::digest(&p.data))
                })
                .collect()
        }),
        SupportedTag::Explicit => explicit_present(tag, format).then(|| {
            vec![format!(
                "{:?}",
                tag.all_tags().get(if *format == AudioFileFormat::MP4 {
                    "rtng"
                } else if matches!(format, AudioFileFormat::FLAC | AudioFileFormat::OGG) {
                    "COMMENT"
                } else {
                    "ITUNESADVISORY"
                })
            )]
        }),
        SupportedTag::URL => tag.get_raw("WWWAUDIOFILE"),
        SupportedTag::TrackId | SupportedTag::ReleaseId | SupportedTag::MetaTags => {
            let suffix = match selected {
                SupportedTag::TrackId => "_TRACK_ID",
                SupportedTag::ReleaseId => "_RELEASE_ID",
                _ => "_TAGGEDDATE",
            };
            let mut values: Vec<_> = tag
                .all_tags()
                .into_iter()
                .filter(|(k, _)| {
                    let canonical = k
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .flat_map(char::to_lowercase)
                        .collect::<String>();
                    k.to_uppercase().ends_with(suffix)
                        || match selected {
                            SupportedTag::TrackId => {
                                canonical.ends_with("musicbrainztrackid")
                                    || canonical.ends_with("musicbrainzreleasetrackid")
                            }
                            SupportedTag::ReleaseId => {
                                canonical.ends_with("musicbrainzalbumid")
                                    || canonical.ends_with("musicbrainzreleasegroupid")
                            }
                            _ => false,
                        }
                })
                .collect();
            values.sort_by(|a, b| a.0.cmp(&b.0));
            (!values.is_empty()).then(|| {
                values
                    .into_iter()
                    .flat_map(|(k, vs)| vs.into_iter().map(move |v| format!("{k}={v}")))
                    .collect()
            })
        }
        // These need individual native bindings rather than claiming any unrelated tag satisfies them.
        SupportedTag::OtherTags => {
            let values: Vec<_> = other_bindings
                .iter()
                .filter_map(|key| tag.get_raw(key).map(|v| format!("{key}={v:?}")))
                .collect();
            (!values.is_empty()).then_some(values)
        }
        SupportedTag::SyncedLyrics | SupportedTag::UnsyncedLyrics => None,
        _ => None,
    }
}
pub(crate) fn values(
    path: &Path,
    info: &MediaInfo,
    selected: &[SupportedTag],
    other_bindings: &[String],
    custom_style: Option<&str>,
) -> Result<Vec<Option<Vec<String>>>> {
    let format = audio_format(info)?;
    if format == AudioFileFormat::WMA {
        let tag = AsfTag::open(path)?;
        Ok(selected
            .iter()
            .map(|s| {
                if *s == SupportedTag::Style
                    && let Some(custom) = custom_style
                {
                    tag.get_raw(custom)
                } else {
                    value(&tag, *s, &format, other_bindings)
                }
            })
            .collect())
    } else {
        let wrapper = Tag::load_format(path, &format, true)?;
        let advisory = if format == AudioFileFormat::MP4 {
            mp4ameta::Tag::read_from_path(path)?
                .advisory_rating()
                .map(|r| vec![format!("{r:?}")])
        } else {
            None
        };
        Ok(selected
            .iter()
            .map(|s| {
                if *s == SupportedTag::Style
                    && let Some(custom) = custom_style
                {
                    wrapper.tag().get_raw(custom)
                } else if *s == SupportedTag::TrackId && matches!(&wrapper, Tag::ID3(_)) {
                    let mut values =
                        value(&**wrapper.tag(), *s, &format, other_bindings).unwrap_or_default();
                    if let Tag::ID3(tag) = &wrapper {
                        for id in tag
                            .editor_tag()
                            .unique_file_identifiers()
                            .filter(|id| id.owner_identifier == "http://musicbrainz.org")
                        {
                            if let Ok(value) = std::str::from_utf8(&id.identifier) {
                                values.push(format!("UFID:http://musicbrainz.org={value}"));
                            }
                        }
                    }
                    (!values.is_empty()).then_some(values)
                } else if format == AudioFileFormat::MP4 && *s == SupportedTag::Explicit {
                    advisory.clone()
                } else {
                    value(*wrapper.tag(), *s, &format, other_bindings)
                }
            })
            .collect())
    }
}

pub(crate) fn external_art_exists(
    path: &Path,
    info: &MediaInfo,
    input: &crate::TagInput,
    config: &crate::AutotagConfig,
) -> Result<bool> {
    if !config.options.album_art_file {
        return Ok(false);
    }
    let format = audio_format(info)?;
    if format == AudioFileFormat::WMA {
        Ok(crate::sidecar::target_name(input, config, &AsfTag::open(path)?)?.is_file())
    } else {
        let tag = Tag::load_format(path, &format, true)?;
        Ok(crate::sidecar::target_name(input, config, *tag.tag())?.is_file())
    }
}

pub(crate) fn explicit_at(
    path: &Path,
    format: &AudioFileFormat,
    tag: &dyn TagImpl,
) -> Result<bool> {
    if *format == AudioFileFormat::MP4 {
        Ok(mp4ameta::Tag::read_from_path(path)?
            .advisory_rating()
            .is_some())
    } else {
        Ok(explicit_present(tag, format))
    }
}

/// The name the native writer addresses for a custom key. MP4 freeform keys are
/// resolved by `MP4Tag::string_to_ident`, which keeps only the first two `:`
/// components, so `com.apple.iTunes:LABEL:ignored` still writes `LABEL`.
fn custom_leaf(key: &str, format: &AudioFileFormat) -> String {
    if *format == AudioFileFormat::MP4
        && let DataIdent::Freeform { name, .. } = MP4Tag::string_to_ident(key)
    {
        return name.to_uppercase();
    }
    key.rsplit(':').next().unwrap_or(key).to_uppercase()
}

fn same_mp4_atom(a: &str, b: &str) -> bool {
    let ident = |key| MP4Tag::ident_to_string(&MP4Tag::string_to_ident(key));
    ident(a).eq_ignore_ascii_case(&ident(b))
}

/// Native MP4 atoms for the comment, lyrics, description, artwork and rating
/// fields that the ID3/Vorbis names in `protected_custom_target` already cover.
fn protected_mp4_atom(key: &str) -> bool {
    use mp4ameta::ident;
    matches!(
        MP4Tag::string_to_ident(key),
        DataIdent::Fourcc(code) if [
            ident::COMMENT,
            ident::LYRICS,
            ident::DESCRIPTION,
            ident::ARTWORK,
            ident::ADVISORY_RATING,
        ]
        .contains(&code)
    )
}

fn dj_namespace(upper: &str) -> bool {
    ["SERATO", "REKORDBOX", "TRAKTOR"]
        .iter()
        .any(|vendor| upper.contains(vendor))
}

/// Second spellings the adapters read and write as the same logical field as its binding:
/// FLAC and Ogg keep legacy `LABEL` and `VERSION` beside `PUBLISHER` and `SUBTITLE`, and
/// WMA writes both `WM/Artist` and `Author` and reads `WM/Artist` first.
fn field_aliases(field: &Field, format: &AudioFileFormat) -> &'static [&'static str] {
    match (field, format) {
        (Field::Label, AudioFileFormat::FLAC | AudioFileFormat::OGG) => &["LABEL"],
        (Field::Version, AudioFileFormat::FLAC | AudioFileFormat::OGG) => &["VERSION"],
        (Field::Artist, AudioFileFormat::WMA) => &["WM/Artist", "Author"],
        _ => &[],
    }
}

pub(crate) fn protected_custom_target(key: &str, format: &AudioFileFormat) -> bool {
    if *format == AudioFileFormat::MP4 {
        if protected_mp4_atom(key) {
            return true;
        }
        // Serato's cue and grid atoms are `com.serato.dj:markersv2` and `:beatgrid`,
        // so the vendor appears only in the freeform mean.
        if let DataIdent::Freeform { mean, .. } = MP4Tag::string_to_ident(key)
            && dj_namespace(&mean.to_uppercase())
        {
            return true;
        }
    }
    let leaf = custom_leaf(key, format);
    [
        "GEOB",
        "UFID",
        "PRIV",
        "POPM",
        "COMM",
        "COMMENT",
        "DESCRIPTION",
        "APIC",
        "PICTURE",
        "COVR",
        "METADATA_BLOCK_PICTURE",
        "COVERART",
        "COVERARTMIME",
        "WM/PICTURE",
        "ITUNESADVISORY",
        "RTNG",
        "RATING",
        "RATE",
        "WM/SHAREDUSERRATING",
        "LYRICS",
        "USLT",
        "SYLT",
    ]
    .contains(&leaf.as_str())
        || dj_namespace(&leaf)
}

pub(crate) fn custom_bypasses_field_policy(
    key: &str,
    format: &AudioFileFormat,
    allow_style: bool,
) -> bool {
    let leaf = custom_leaf(key, format);
    let bound = [
        Field::Title,
        Field::Artist,
        Field::Album,
        Field::AlbumArtist,
        Field::Genre,
        Field::Style,
        Field::Label,
        Field::Remixer,
        Field::Version,
        Field::BPM,
        Field::Key,
        Field::Duration,
        Field::CatalogNumber,
        Field::ISRC,
        Field::TrackNumber,
        Field::TrackTotal,
        Field::DiscNumber,
        Field::Mood,
    ]
    .iter()
    .any(|field| {
        if allow_style && *field == Field::Style {
            return false;
        }
        let binding = field.by_format(format);
        binding.eq_ignore_ascii_case(key)
            || binding.eq_ignore_ascii_case(&leaf)
            || (*format == AudioFileFormat::MP4 && same_mp4_atom(binding, key))
            || field_aliases(field, format)
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(key) || alias.eq_ignore_ascii_case(&leaf))
    });
    bound
        || (leaf
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .contains("MUSICBRAINZ")
            && leaf.ends_with("ID"))
        || leaf.ends_with("_TRACK_ID")
        || leaf.ends_with("_RELEASE_ID")
        || leaf.ends_with("_TAGGEDDATE")
        || [
            "DATE",
            "YEAR",
            "©DAY",
            "ORIGINALDATE",
            "TDRC",
            "TDRL",
            "TYER",
            "TDAT",
            "TIME",
            "WM/YEAR",
            "RELEASETIME",
            "WM/ORIGINALRELEASETIME",
            "PUBLISHTIME",
            "WWWAUDIOFILE",
            "WOAF",
            "WM/URL",
            "ENERGY",
        ]
        .contains(&leaf.as_str())
}

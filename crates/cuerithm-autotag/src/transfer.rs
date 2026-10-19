//! Codec-aware metadata snapshots for reviewed donor transfers. No original writes.
use crate::{FieldChange, Metadata, MetadataField, asf::AsfTag, input::audio_format};
use anyhow::{Result, ensure};
use cuerithm_media::MediaInfo;
use onetagger_tag::{AudioFileFormat, Field, Tag, TagDate, TagImpl};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferText {
    Key,
    Energy,
    Color,
    Extra1,
    Extra2,
}
impl TransferText {
    fn binding(self, format: &AudioFileFormat) -> &'static str {
        match self {
            Self::Key => Field::Key.by_format(format),
            Self::Energy => "ENERGY",
            Self::Color => "COLOR",
            Self::Extra1 => "EXTRA1",
            Self::Extra2 => "EXTRA2",
        }
    }
    const ALL: &'static [Self] = &[
        Self::Key,
        Self::Energy,
        Self::Color,
        Self::Extra1,
        Self::Extra2,
    ];
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferDate {
    pub year: i32,
    pub month: Option<u8>,
    pub day: Option<u8>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TransferSnapshot {
    pub text: BTreeMap<MetadataField, Vec<String>>,
    pub extra: BTreeMap<TransferText, Vec<String>>,
    pub bpm: Option<f64>,
    pub date: Option<TransferDate>,
    pub rating: Option<u8>,
    pub track_number: Option<u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferPatch {
    pub before: TransferSnapshot,
    pub after: TransferSnapshot,
}
impl TransferSnapshot {
    pub fn validate(&self) -> Result<()> {
        Metadata {
            fields: self.text.clone(),
            custom_tags: vec![],
        }
        .validate()?;
        let values = self.text.values().chain(self.extra.values());
        let mut size = 0usize;
        for values in values {
            ensure!(
                !values.is_empty() && values.len() <= 128,
                "Empty or oversized transfer field"
            );
            for value in values {
                size = size.saturating_add(value.len());
            }
        }
        ensure!(size <= 1_048_576, "Transfer metadata exceeds 1 MiB");
        ensure!(
            self.bpm
                .is_none_or(|v| v.is_finite() && v > 0. && v < 1000.),
            "Invalid transfer BPM"
        );
        ensure!(
            self.rating.is_none_or(|v| (1..=5).contains(&v)),
            "Invalid normalized transfer rating"
        );
        ensure!(
            self.track_number.is_none_or(|v| v > 0),
            "Invalid transfer track number"
        );
        if let Some(date) = &self.date {
            ensure!((1..=9999).contains(&date.year), "Invalid transfer year");
            ensure!(
                match (date.month, date.day) {
                    (None, None) => true,
                    (Some(m), Some(d)) =>
                        chrono::NaiveDate::from_ymd_opt(date.year, m.into(), d.into()).is_some(),
                    _ => false,
                },
                "Invalid transfer calendar date"
            );
        }
        Ok(())
    }
}
fn read(tag: &dyn TagImpl, format: &AudioFileFormat) -> Result<TransferSnapshot> {
    let mut extra = BTreeMap::new();
    for field in TransferText::ALL {
        if let Some(values) = tag.get_native(field.binding(format))
            && !values.is_empty()
        {
            extra.insert(*field, values);
        }
    }
    let bpm = (if *format == AudioFileFormat::MP4 {
        tag.get_native("----:com.apple.iTunes:BPM")
    } else {
        None
    })
    .or_else(|| tag.get_field(Field::BPM))
    .and_then(|v| v.first()?.trim().parse::<f64>().ok())
    .filter(|n| *n > 0.);
    let date = tag
        .get_date()
        .or_else(|| {
            tag.get_native("YEAR")
                .and_then(|v| v.first()?.parse().ok())
                .map(|year| TagDate {
                    year,
                    month: None,
                    day: None,
                })
        })
        .map(|d| TransferDate {
            year: d.year,
            month: d.month,
            day: d.day,
        });
    let track_number = tag
        .get_field(Field::TrackNumber)
        .and_then(|v| v.first()?.parse().ok())
        .filter(|v| *v > 0);
    let value = TransferSnapshot {
        text: crate::metadata::read(tag, format)?.fields,
        extra,
        bpm,
        date,
        rating: tag.get_rating().filter(|r| *r > 0),
        track_number,
    };
    value.validate()?;
    Ok(value)
}
pub fn read_transfer_metadata(path: &Path, info: &MediaInfo) -> Result<TransferSnapshot> {
    let format = audio_format(info)?;
    if format == AudioFileFormat::WMA {
        read(&AsfTag::open(path)?, &format)
    } else {
        let wrapper = Tag::load_format(path, &format, true)?;
        read(*wrapper.tag(), &format)
    }
}
impl TransferPatch {
    pub fn validate(&self) -> Result<()> {
        self.before.validate()?;
        self.after.validate()
    }
    pub(crate) fn verify(&self, path: &Path, info: &MediaInfo) -> Result<()> {
        let actual = read_transfer_metadata(path, info)?;
        let mut fields = Vec::new();
        for field in MetadataField::ALL {
            if actual.text.get(field) != self.after.text.get(field) {
                fields.push(format!("{field:?}"));
            }
        }
        for field in TransferText::ALL {
            if actual.extra.get(field) != self.after.extra.get(field) {
                fields.push(format!("{field:?}"));
            }
        }
        if actual.bpm != self.after.bpm {
            fields.push("BPM".into());
        }
        if actual.date != self.after.date {
            fields.push("date".into());
        }
        if actual.rating != self.after.rating {
            fields.push("rating".into());
        }
        if actual.track_number != self.after.track_number {
            fields.push("track number".into());
        }
        ensure!(
            fields.is_empty(),
            "Transferred metadata cannot be represented exactly in this tag format: {}",
            fields.join(", ")
        );
        Ok(())
    }
    pub(crate) fn apply(&self, tag: &mut dyn TagImpl, format: &AudioFileFormat) -> Result<()> {
        ensure!(
            read(tag, format)? == self.before,
            "Native transfer preview is stale"
        );
        let changes = MetadataField::ALL
            .iter()
            .filter_map(|field| {
                let before = self.before.text.get(field).cloned().unwrap_or_default();
                let after = self.after.text.get(field).cloned().unwrap_or_default();
                (before != after).then_some(FieldChange {
                    field: *field,
                    before,
                    after,
                })
            })
            .collect::<Vec<_>>();
        crate::metadata::edit(tag, format, &changes)?;
        for field in TransferText::ALL {
            if self.before.extra.get(field) != self.after.extra.get(field) {
                tag.set_native(
                    field.binding(format),
                    self.after.extra.get(field).cloned().unwrap_or_default(),
                );
            }
        }
        if self.before.bpm != self.after.bpm {
            match self.after.bpm {
                Some(bpm) => {
                    if *format == AudioFileFormat::MP4 {
                        tag.set_field(Field::BPM, vec![bpm.round().to_string()], true);
                        tag.set_native("----:com.apple.iTunes:BPM", vec![bpm.to_string()]);
                    } else {
                        tag.set_field(Field::BPM, vec![bpm.to_string()], true);
                    }
                }
                None => {
                    tag.remove_raw(Field::BPM.by_format(format));
                    if *format == AudioFileFormat::MP4 {
                        tag.remove_raw("----:com.apple.iTunes:BPM");
                    }
                }
            }
        }
        if self.before.date != self.after.date {
            let keys: &[&str] = match format {
                AudioFileFormat::FLAC | AudioFileFormat::OGG => &["DATE", "YEAR"],
                AudioFileFormat::MP4 => &["©day", "YEAR", "DATE"],
                AudioFileFormat::WMA => &["WM/Year", "RELEASETIME", "YEAR"],
                _ => &["TDRC", "TYER", "TDAT", "TIME"],
            };
            for key in keys {
                tag.remove_raw(key);
            }
            if let Some(d) = &self.after.date {
                tag.set_date(
                    &TagDate {
                        year: d.year,
                        month: d.month,
                        day: d.day,
                    },
                    true,
                );
            }
        }
        if self.before.rating != self.after.rating {
            tag.set_rating(self.after.rating.unwrap_or(0), true);
        }
        if self.before.track_number != self.after.track_number {
            let total = tag
                .get_field(Field::TrackTotal)
                .and_then(|v| v.first()?.parse::<u16>().ok());
            if let Some(number) = self.after.track_number {
                tag.set_track_number(&number.to_string(), total, true);
            } else if total.is_some()
                && matches!(
                    format,
                    AudioFileFormat::MP4
                        | AudioFileFormat::MP3
                        | AudioFileFormat::AAC
                        | AudioFileFormat::AIFF
                        | AudioFileFormat::WAV
                )
            {
                tag.set_track_number("0", total, true);
            } else {
                tag.remove_raw(Field::TrackNumber.by_format(format));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnalysisTags, AutotagConfig, MatchStatus, TagInput, TagMatchOutcome, TagWritePlan,
    };
    use cuerithm_media::MediaRuntime;
    #[test]
    fn label_aliases_are_updated_and_cleared_as_one_logical_field() {
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
        let cache = d.path().join("cache");
        let mut count = 0;
        for row in rows.as_array().unwrap() {
            let path = d.path().join(row["file"].as_str().unwrap());
            let info = runtime.probe(&path).unwrap();
            let format = audio_format(&info).unwrap();
            if !matches!(format, AudioFileFormat::FLAC | AudioFileFormat::OGG) {
                continue;
            }
            count += 1;
            let mut tag = Tag::load_format(&path, &format, true).unwrap();
            tag.tag_mut().set_native("LABEL", vec!["Primary".into()]);
            tag.tag_mut().set_native("PUBLISHER", vec!["Shadow".into()]);
            tag.tag_mut().save_file(&path).unwrap();
            let before = read_transfer_metadata(&path, &info).unwrap();
            let mut after = before.clone();
            after
                .text
                .insert(MetadataField::Label, vec!["Donor".into()]);
            let audio = runtime.decode(&path, &cache).unwrap();
            let mut cfg = AutotagConfig::default();
            cfg.options.tags.clear();
            cfg.write_energy = false;
            let input = TagInput {
                path: path.clone(),
                source_sha256: audio.source_sha256.clone(),
                media: audio.info.clone(),
                duration_seconds: audio.clock.duration_seconds(),
            };
            let outcome = TagMatchOutcome {
                status: MatchStatus::NoMatch,
                matches: vec![],
                attempted: vec![],
                warnings: vec![],
            };
            let staged = TagWritePlan::build(input, &outcome, &cfg, &AnalysisTags::default())
                .unwrap()
                .with_transfer(TransferPatch {
                    before,
                    after: after.clone(),
                })
                .unwrap()
                .stage_prepared(&runtime, &audio, &cache)
                .unwrap();
            let read = Tag::load_format(&staged.path, &format, true).unwrap();
            assert_eq!(
                read.tag().get_native("PUBLISHER"),
                Some(vec!["Donor".into()])
            );
            let mut cleared = after.clone();
            cleared.text.remove(&MetadataField::Label);
            let audio = runtime.decode(&staged.path, &cache).unwrap();
            let input = TagInput {
                path: staged.path.clone(),
                source_sha256: audio.source_sha256.clone(),
                media: audio.info.clone(),
                duration_seconds: audio.clock.duration_seconds(),
            };
            let result = TagWritePlan::build(input, &outcome, &cfg, &AnalysisTags::default())
                .unwrap()
                .with_transfer(TransferPatch {
                    before: after,
                    after: cleared.clone(),
                })
                .unwrap()
                .stage_prepared(&runtime, &audio, &cache)
                .unwrap();
            assert_eq!(
                read_transfer_metadata(&result.path, &result.metadata).unwrap(),
                cleared
            );
        }
        assert_eq!(count, 3);
    }
    #[test]
    fn transfers_keep_opaque_tags_artwork_and_independent_track_totals_across_all_layouts() {
        use crate::{CatalogMatch, CatalogTrack, FrameName, SupportedTag};
        fn inspect<T>(path: &Path, info: &MediaInfo, f: impl FnOnce(&dyn TagImpl) -> T) -> T {
            let format = audio_format(info).unwrap();
            if format == AudioFileFormat::WMA {
                f(&AsfTag::open(path).unwrap())
            } else {
                let wrapper = Tag::load_format(path, &format, true).unwrap();
                f(*wrapper.tag())
            }
        }
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
        let cache = d.path().join("cache");
        let image = std::fs::read(root.join("apps/desktop/src-tauri/icons/32x32.png")).unwrap();
        let base = |audio: &cuerithm_media::PreparedAudio| TagInput {
            path: audio.source_path.clone(),
            source_sha256: audio.source_sha256.clone(),
            media: audio.info.clone(),
            duration_seconds: audio.clock.duration_seconds(),
        };
        let mut cfg = AutotagConfig {
            write_energy: false,
            ..Default::default()
        };
        for row in rows.as_array().unwrap() {
            let path = d.path().join(row["file"].as_str().unwrap());
            let audio = runtime.decode(&path, &cache).unwrap();
            cfg.options.tags = vec![
                SupportedTag::OtherTags,
                SupportedTag::AlbumArt,
                SupportedTag::TrackTotal,
            ];
            let outcome = TagMatchOutcome {
                status: MatchStatus::Matched,
                matches: vec![CatalogMatch::new(
                    1.,
                    CatalogTrack {
                        platform: "beatport".into(),
                        other: vec![(FrameName::same("CUSTOM_KEEP"), vec!["Opaque value".into()])],
                        track_total: Some(20),
                        ..Default::default()
                    },
                )],
                attempted: vec![],
                warnings: vec![],
            };
            let seed = TagWritePlan::build(base(&audio), &outcome, &cfg, &AnalysisTags::default())
                .unwrap()
                .with_artwork(image.clone())
                .unwrap()
                .stage_prepared(&runtime, &audio, &cache)
                .unwrap();
            let original_art = runtime.artwork(&seed.path).unwrap().unwrap().bytes;
            let before = read_transfer_metadata(&seed.path, &seed.metadata).unwrap();
            let mut after = before.clone();
            after
                .text
                .insert(MetadataField::Title, vec!["Transferred title".into()]);
            after.track_number = Some(7);
            let audio = runtime.decode(&seed.path, &cache).unwrap();
            cfg.options.tags.clear();
            let no_match = TagMatchOutcome {
                status: MatchStatus::NoMatch,
                matches: vec![],
                attempted: vec![],
                warnings: vec![],
            };
            let changed =
                TagWritePlan::build(base(&audio), &no_match, &cfg, &AnalysisTags::default())
                    .unwrap()
                    .with_transfer(TransferPatch {
                        before,
                        after: after.clone(),
                    })
                    .unwrap()
                    .stage_prepared(&runtime, &audio, &cache)
                    .unwrap();
            assert_eq!(
                inspect(&changed.path, &changed.metadata, |t| t
                    .get_native("CUSTOM_KEEP")),
                Some(vec!["Opaque value".into()])
            );
            assert_eq!(
                inspect(&changed.path, &changed.metadata, |t| t
                    .get_field(Field::TrackTotal)),
                Some(vec!["20".into()])
            );
            assert_eq!(
                runtime.artwork(&changed.path).unwrap().unwrap().bytes,
                original_art
            );
            let before = after.clone();
            after.track_number = None;
            let audio = runtime.decode(&changed.path, &cache).unwrap();
            let cleared =
                TagWritePlan::build(base(&audio), &no_match, &cfg, &AnalysisTags::default())
                    .unwrap()
                    .with_transfer(TransferPatch { before, after })
                    .unwrap()
                    .stage_prepared(&runtime, &audio, &cache)
                    .unwrap();
            assert_eq!(
                inspect(&cleared.path, &cleared.metadata, |t| t
                    .get_field(Field::TrackTotal)),
                Some(vec!["20".into()])
            );
            assert_eq!(
                runtime.artwork(&cleared.path).unwrap().unwrap().bytes,
                original_art
            );
        }
    }
}

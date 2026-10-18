use crate::{AutotagConfig, config::filename_template};
use anyhow::{Context, Result, ensure};
use cuerithm_media::MediaInfo;
use onetagger_tagger::{AudioFileFormat, AudioFileInfo, FileTaggedStatus};
use std::{collections::HashMap, path::PathBuf, time::Duration};
#[derive(Debug, Clone)]
pub struct TagInput {
    pub path: PathBuf,
    pub source_sha256: String,
    pub media: MediaInfo,
    pub duration_seconds: f64,
}
impl TagInput {
    pub fn audio_info(&self, config: &AutotagConfig) -> Result<AudioFileInfo> {
        ensure!(
            self.duration_seconds.is_finite() && self.duration_seconds >= 0.,
            "Invalid playable duration"
        );
        let format_name = self
            .media
            .properties
            .get("format.format_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let format = match self.media.codec.as_str() {
            "flac" => AudioFileFormat::FLAC,
            "mp3" => AudioFileFormat::MP3,
            "alac" => AudioFileFormat::MP4,
            "aac"
                if format_name
                    .split(',')
                    .any(|f| f == "mov" || f == "mp4" || f == "m4a") =>
            {
                AudioFileFormat::MP4
            }
            "aac" => AudioFileFormat::AAC,
            "vorbis" | "opus" => AudioFileFormat::OGG,
            "wmav1" | "wmav2" | "wmapro" | "wmalossless" | "wmavoice" => AudioFileFormat::WMA,
            _ if format_name == "aiff" => AudioFileFormat::AIFF,
            _ if format_name == "wav" => AudioFileFormat::WAV,
            _ => anyhow::bail!("Unsupported tagging container: {}", self.media.codec),
        };
        let tag = |key: &str| {
            self.media
                .tags
                .get(key)
                .filter(|s| !s.trim().is_empty())
                .cloned()
        };
        let mut title = tag("title");
        let mut artists = tag("artist")
            .map(|s| AudioFileInfo::parse_artist_tag(vec![s.as_str()]))
            .unwrap_or_default();
        if config.options.parse_filename && (title.is_none() || artists.is_empty()) {
            let pattern = filename_template(
                config
                    .options
                    .filename_template
                    .as_deref()
                    .unwrap_or("%artists% - %title%"),
            )?;
            let filename = self
                .path
                .file_name()
                .and_then(|p| p.to_str())
                .context("Filename cannot be parsed as Unicode")?;
            if let Some(c) = pattern.captures(filename) {
                if title.is_none() {
                    title = c.name("title").map(|v| v.as_str().trim().to_owned());
                }
                if artists.is_empty()
                    && let Some(value) = c.name("artists")
                {
                    artists = AudioFileInfo::parse_artist_tag(vec![value.as_str().trim()]);
                }
            }
        }
        if let Some(pattern) = &config.options.title_regex {
            let re = regex::Regex::new(pattern)?;
            title = title.map(|s| re.replace_all(&s, "").into_owned());
        }
        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        for (key, value) in &self.media.tags {
            tags.insert(key.to_uppercase(), vec![value.clone()]);
        }
        let tagged = match tag("1t_taggeddate") {
            Some(v) if v.ends_with("_AT") => FileTaggedStatus::AutoTagger,
            Some(v) if v.ends_with("_AF") => FileTaggedStatus::AudioFeatures,
            Some(_) => FileTaggedStatus::Tagged,
            None => FileTaggedStatus::Untagged,
        };
        Ok(AudioFileInfo {
            title,
            artists,
            format,
            path: self.path.clone(),
            isrc: tag("isrc"),
            duration: Some(Duration::try_from_secs_f64(self.duration_seconds)?),
            track_number: tag("track")
                .or_else(|| tag("tracknumber"))
                .and_then(|s| s.split('/').next()?.parse().ok()),
            tagged,
            tags,
        })
    }
}

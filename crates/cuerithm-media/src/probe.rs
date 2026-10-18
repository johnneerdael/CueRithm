use crate::MediaRuntime;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, ffi::OsString, io::Read, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub bits_per_sample: Option<u32>,
    pub tags: BTreeMap<String, String>,
}
impl MediaRuntime {
    pub fn probe(&self, path: &Path) -> Result<MediaInfo> {
        ensure!(path.is_file(), "Audio source must be a local file");
        let path = path.canonicalize()?;
        let args: Vec<OsString> = ["-v","error","-protocol_whitelist","file,pipe","-select_streams","a:0",
            "-show_entries","stream=codec_name,sample_rate,channels,bits_per_raw_sample:stream_tags=title,artist,album,genre:format_tags=title,artist,album,genre","-of","json"].into_iter().map(Into::into)
            .chain([path.as_os_str().to_owned()]).collect();
        let mut out = self.run(&self.ffprobe, &args, tempfile::tempfile()?, 1_048_576)?;
        let mut bytes = Vec::new();
        out.read_to_end(&mut bytes)?;
        let doc: serde_json::Value = serde_json::from_slice(&bytes)?;
        let stream = doc["streams"]
            .as_array()
            .and_then(|s| s.first())
            .context("No audio stream found")?;
        let sample_rate = stream["sample_rate"]
            .as_str()
            .context("Missing sample rate")?
            .parse()?;
        let channels = stream["channels"]
            .as_u64()
            .context("Missing channel count")? as u32;
        ensure!(
            (8000..=384000).contains(&sample_rate) && (1..=32).contains(&channels),
            "Unsupported audio rate/channel layout"
        );
        // Container defaults first, selected audio-stream values take precedence.
        let mut tags = BTreeMap::new();
        for object in [&doc["format"]["tags"], &stream["tags"]] {
            if let Some(values) = object.as_object() {
                for (key, value) in values {
                    if let Some(value) = value.as_str().filter(|s| !s.trim().is_empty()) {
                        tags.insert(key.to_lowercase(), value.to_owned());
                    }
                }
            }
        }
        Ok(MediaInfo {
            codec: stream["codec_name"]
                .as_str()
                .context("Missing codec")?
                .to_owned(),
            sample_rate,
            channels,
            tags,
            bits_per_sample: stream["bits_per_raw_sample"]
                .as_str()
                .and_then(|s| s.parse().ok()),
        })
    }
}

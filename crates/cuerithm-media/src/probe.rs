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
    #[serde(default)]
    pub properties: BTreeMap<String, serde_json::Value>,
}
impl MediaInfo {
    pub fn columns(&self) -> BTreeMap<String, serde_json::Value> {
        let mut values = self.properties.clone();
        values.insert("audio.codec".into(), self.codec.clone().into());
        values.insert("audio.sample_rate".into(), self.sample_rate.into());
        values.insert("audio.channels".into(), self.channels.into());
        values.insert("audio.bits_per_sample".into(), self.bits_per_sample.into());
        values.extend(
            self.tags
                .iter()
                .map(|(key, value)| (format!("tag.{key}"), value.clone().into())),
        );
        // OneTagger's FLAC label field uses PUBLISHER. Keep that raw field
        // visible, and resolve the library's existing Label column as well.
        if !values.contains_key("tag.label")
            && let Some(publisher) = values.get("tag.publisher").cloned()
        {
            values.insert("tag.label".into(), publisher);
        }
        values
    }
}
fn properties(
    prefix: &str,
    value: &serde_json::Value,
    out: &mut BTreeMap<String, serde_json::Value>,
) {
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            if key == "tags" || key == "filename" {
                continue;
            }
            properties(&format!("{prefix}.{key}"), value, out);
        }
    } else if !value.is_null() {
        out.insert(prefix.into(), value.clone());
    }
}
impl MediaRuntime {
    pub fn probe(&self, path: &Path) -> Result<MediaInfo> {
        self.probe_cancellable(path, &crate::CancellationToken::default())
    }
    pub fn probe_cancellable(
        &self,
        path: &Path,
        cancel: &crate::CancellationToken,
    ) -> Result<MediaInfo> {
        cancel.check()?;
        ensure!(path.is_file(), "Audio source must be a local file");
        let path = path.canonicalize()?;
        let args: Vec<OsString> = [
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream:format",
            "-of",
            "json",
        ]
        .into_iter()
        .map(Into::into)
        .chain([path.as_os_str().to_owned()])
        .collect();
        let mut out = self.run_controlled(
            &self.ffprobe,
            &args,
            tempfile::tempfile()?,
            1_048_576,
            std::time::Duration::from_secs(300),
            cancel,
        )?;
        let mut bytes = Vec::new();
        out.read_to_end(&mut bytes)?;
        cancel.check()?;
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
        let mut fields = BTreeMap::new();
        properties("audio", stream, &mut fields);
        properties("format", &doc["format"], &mut fields);
        Ok(MediaInfo {
            properties: fields,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn label_column_resolves_the_publisher_alias_without_losing_raw_metadata() {
        let mut info = MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            tags: [("publisher".into(), "Fixture label".into())].into(),
            properties: Default::default(),
        };
        assert_eq!(info.columns()["tag.label"], "Fixture label");
        assert_eq!(info.columns()["tag.publisher"], "Fixture label");
        info.tags.insert("label".into(), "Explicit label".into());
        assert_eq!(info.columns()["tag.label"], "Explicit label");
        assert_eq!(info.columns()["tag.publisher"], "Fixture label");
    }
}

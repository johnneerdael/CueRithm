use crate::MediaRuntime;
use anyhow::{Result, ensure};
use std::{ffi::OsString, io::Read, path::Path};

pub struct Artwork {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}
pub fn artwork_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}
impl MediaRuntime {
    pub fn artwork(&self, path: &Path) -> Result<Option<Artwork>> {
        ensure!(path.is_file(), "Artwork source is missing");
        let path = path.canonicalize()?;
        let args: Vec<OsString> = [
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-select_streams",
            "v",
            "-show_entries",
            "stream=index:stream_disposition=attached_pic:stream_tags=comment,title",
            "-of",
            "json",
        ]
        .into_iter()
        .map(Into::into)
        .chain([path.as_os_str().to_owned()])
        .collect();
        let mut probe = self.run(&self.ffprobe, &args, tempfile::tempfile()?, 1_048_576)?;
        let mut json = Vec::new();
        probe.read_to_end(&mut json)?;
        let doc: serde_json::Value = serde_json::from_slice(&json)?;
        let Some(streams) = doc["streams"].as_array() else {
            return Ok(None);
        };
        let pictures: Vec<_> = streams
            .iter()
            .filter(|s| s["disposition"]["attached_pic"] == 1)
            .collect();
        let picture = pictures
            .iter()
            .find(|s| {
                s["tags"]["comment"]
                    .as_str()
                    .is_some_and(|s| s.to_lowercase().contains("front"))
            })
            .or_else(|| pictures.first());
        let Some(index) = picture.and_then(|s| s["index"].as_u64()) else {
            return Ok(None);
        };
        ensure!(index < 1024, "Invalid artwork stream");
        let args: Vec<OsString> = [
            "-v",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .chain([path.as_os_str().to_owned()])
        .chain([
            "-map".into(),
            format!("0:{index}").into(),
            "-c:v".into(),
            "copy".into(),
            "-frames:v".into(),
            "1".into(),
            "-f".into(),
            "image2pipe".into(),
            "pipe:1".into(),
        ])
        .collect();
        let mut output = self.run(&self.ffmpeg, &args, tempfile::tempfile()?, 8 * 1024 * 1024)?;
        let mut bytes = Vec::new();
        output.read_to_end(&mut bytes)?;
        Ok(artwork_mime(&bytes).map(|mime| Artwork { mime, bytes }))
    }
}

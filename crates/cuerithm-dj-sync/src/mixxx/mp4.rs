//! What an M4A says about the start of its audio, for the decoders that do not read it.
//!
//! FFmpeg and CoreAudio drop the encoder's priming samples when they decode an M4A, so the
//! first sample they return is the first sample of the music. FAAD, which Mixxx uses for M4A
//! where it is built in, returns every block from the first one and does not read the edit list,
//! but its first output block is one 1024-sample block short. Measured in Mixxx (`main`, FAAD):
//! its first sample is `priming - 1024` frames earlier in the music than FFmpeg's, so an
//! FFmpeg-encoded file (priming 1024) lines up and one from Apple's encoder (priming 2112)
//! is 1088 frames late.
//!
//! The priming is what FFmpeg trims: the first non-empty edit of the track's edit list
//! (`elst`), or the second number of Apple's `iTunSMPB` tag when there is no edit list.
use anyhow::{Context, Result};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// What FAAD's first block accounts for.
const FAAD_BLOCK: u32 = 1024;
/// A `moov` box larger than this is not read.
const MAX_MOOV: u64 = 64 * 1024 * 1024;

/// The first audio track of an M4A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AacStart {
    /// `mp4a` (AAC). Apple Lossless (`alac`) is not decoded by FAAD.
    pub aac: bool,
    /// Samples of encoder priming FFmpeg trims.
    pub priming: u32,
}
impl AacStart {
    /// How many frames later FAAD plays the music than FFmpeg does.
    pub(super) fn faad_shift(self) -> u32 {
        if self.aac {
            self.priming.saturating_sub(FAAD_BLOCK)
        } else {
            0
        }
    }
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}
fn be64(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// The child boxes of `data[start..end]` as (type, body start, body end).
fn children(data: &[u8], start: usize, end: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut found = Vec::new();
    let mut at = start;
    while at + 8 <= end {
        let (Some(size), Some(kind)) = (be32(data, at), data.get(at + 4..at + 8)) else {
            break;
        };
        let (header, size) = match size {
            1 => (16, be64(data, at + 8).unwrap_or(0) as usize),
            0 => (8, end - at),
            n => (8, n as usize),
        };
        if size < header || at + size > end {
            break;
        }
        found.push((kind.try_into().unwrap_or([0; 4]), at + header, at + size));
        at += size;
    }
    found
}

fn child(data: &[u8], start: usize, end: usize, kind: &[u8; 4]) -> Option<(usize, usize)> {
    children(data, start, end)
        .into_iter()
        .find(|(k, _, _)| k == kind)
        .map(|(_, body, stop)| (body, stop))
}

fn descend(data: &[u8], mut range: (usize, usize), path: &[&[u8; 4]]) -> Option<(usize, usize)> {
    for kind in path {
        range = child(data, range.0, range.1, kind)?;
    }
    Some(range)
}

/// The priming of the first edit that is not an empty one.
fn edit_list_priming(data: &[u8], elst: (usize, usize)) -> Option<u32> {
    let version = *data.get(elst.0)?;
    let count = be32(data, elst.0 + 4)? as usize;
    let mut at = elst.0 + 8;
    for _ in 0..count {
        let media_time = if version == 1 {
            let value = be64(data, at + 8)? as i64;
            at += 20;
            value
        } else {
            let value = be32(data, at + 4)? as i32;
            at += 12;
            i64::from(value)
        };
        if media_time >= 0 {
            return u32::try_from(media_time).ok();
        }
    }
    None
}

/// Apple's gapless tag: ` 00000000 PRIMING PADDING SAMPLES ...`, as hexadecimal words.
fn smpb_priming(moov: &[u8]) -> Option<u32> {
    let at = moov.windows(8).position(|w| w == b"iTunSMPB")?;
    let text = String::from_utf8_lossy(&moov[at + 8..(at + 8 + 160).min(moov.len())]).into_owned();
    let words: Vec<&str> = text
        .split(|c: char| !c.is_ascii_hexdigit())
        .filter(|w| w.len() == 8)
        .collect();
    u32::from_str_radix(words.get(1)?, 16).ok()
}

/// Read the start of an M4A: `None` when it is not an MP4 with an AAC or Apple Lossless track.
pub(super) fn aac_start(path: &Path) -> Result<Option<AacStart>> {
    let mut file = File::open(path).with_context(|| format!("Open {}", path.display()))?;
    let length = file.metadata()?.len();
    // Find `moov` among the top-level boxes without reading the audio.
    let mut at = 0u64;
    let mut moov = None;
    while at + 8 <= length {
        let mut header = [0u8; 16];
        file.seek(SeekFrom::Start(at))?;
        let got = file.read(&mut header)?;
        if got < 8 {
            break;
        }
        let size32 = u32::from_be_bytes(header[0..4].try_into()?);
        let (head, size) = match size32 {
            1 if got >= 16 => (16, u64::from_be_bytes(header[8..16].try_into()?)),
            0 => (8, length - at),
            n => (8, u64::from(n)),
        };
        if size < head {
            return Ok(None);
        }
        if &header[4..8] == b"moov" {
            moov = Some((at + head, (at + size).min(length)));
            break;
        }
        at += size;
    }
    let Some((start, stop)) = moov else {
        return Ok(None);
    };
    if stop - start > MAX_MOOV {
        return Ok(None);
    }
    let mut data = vec![0u8; (stop - start) as usize];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut data)?;
    Ok(parse_moov(&data))
}

fn parse_moov(moov: &[u8]) -> Option<AacStart> {
    for (kind, body, stop) in children(moov, 0, moov.len()) {
        if &kind != b"trak" {
            continue;
        }
        let Some(stsd) = descend(moov, (body, stop), &[b"mdia", b"minf", b"stbl", b"stsd"]) else {
            continue;
        };
        // version/flags, entry count, then the first entry: size, format
        let format = moov.get(stsd.0 + 12..stsd.0 + 16)?;
        let aac = match format {
            b"mp4a" => true,
            b"alac" => false,
            _ => continue,
        };
        let priming = descend(moov, (body, stop), &[b"edts", b"elst"])
            .and_then(|elst| edit_list_priming(moov, elst))
            .or_else(|| smpb_priming(moov))
            .unwrap_or(0);
        return Some(AacStart { aac, priming });
    }
    None
}

/// Small M4A files for tests: just enough boxes for `aac_start`.
#[cfg(test)]
pub(super) mod sample {
    /// An M4A of `format` (`mp4a` or `alac`) with an optional edit list and Apple's gapless tag.
    pub(crate) fn m4a(format: &[u8; 4], edit: Option<&[(u32, i32)]>, smpb: bool) -> Vec<u8> {
        let mut moov = track(format, edit);
        if smpb {
            let mut tag = b"\0\0\0\0iTunSMPB\0\0\0\0data\0\0\0\0\0\0\0\0".to_vec();
            tag.extend_from_slice(
                b" 00000000 00000840 00000160 0000000000102660 00000000 00000000",
            );
            moov.extend(boxed(b"udta", &tag));
        }
        let mut bytes = boxed(b"ftyp", b"M4A \0\0\0\0M4A mp42isom");
        bytes.extend(boxed(b"mdat", &[7; 64]));
        bytes.extend(boxed(b"moov", &moov));
        bytes
    }

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    pub(super) fn track(format: &[u8; 4], edit: Option<&[(u32, i32)]>) -> Vec<u8> {
        let mut entry = 16u32.to_be_bytes().to_vec();
        entry.extend_from_slice(format);
        entry.extend_from_slice(&[0; 8]);
        let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
        stsd.extend(entry);
        let stbl = boxed(b"stbl", &boxed(b"stsd", &stsd));
        let mut body = boxed(b"mdia", &boxed(b"minf", &stbl));
        if let Some(entries) = edit {
            let mut elst = vec![0, 0, 0, 0];
            elst.extend((entries.len() as u32).to_be_bytes());
            for (duration, media_time) in entries {
                elst.extend(duration.to_be_bytes());
                elst.extend(media_time.to_be_bytes());
                elst.extend(1u32.to_be_bytes());
            }
            body.extend(boxed(b"edts", &boxed(b"elst", &elst)));
        }
        boxed(b"trak", &body)
    }
}

#[cfg(test)]
mod tests {
    use super::sample::m4a;
    use super::*;

    fn file(bytes: Vec<u8>, name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn ffmpegs_edit_list_gives_the_priming_and_no_shift_for_faad() {
        // What FFmpeg's encoder writes: an edit that starts 1024 samples in.
        let (_dir, path) = file(m4a(b"mp4a", Some(&[(1_058_400, 1024)]), false), "a.m4a");
        let start = aac_start(&path).unwrap().unwrap();
        assert_eq!(
            start,
            AacStart {
                aac: true,
                priming: 1024
            }
        );
        assert_eq!(start.faad_shift(), 0);
    }

    #[test]
    fn an_empty_edit_is_skipped_and_the_first_real_one_counts() {
        let (_dir, path) = file(
            m4a(b"mp4a", Some(&[(500, -1), (900, 2112)]), false),
            "a.m4a",
        );
        assert_eq!(aac_start(&path).unwrap().unwrap().priming, 2112);
    }

    #[test]
    fn apples_itunsmpb_is_used_without_an_edit_list_and_measures_1088_frames_for_faad() {
        // What Apple's encoder writes: 2112 samples of priming (0x840), 352 of padding.
        let (_dir, path) = file(m4a(b"mp4a", None, true), "apple.m4a");
        let start = aac_start(&path).unwrap().unwrap();
        assert_eq!(start.priming, 2112);
        assert_eq!(
            start.faad_shift(),
            1088,
            "measured: frame 23140 against 22052"
        );
    }

    #[test]
    fn apple_lossless_and_other_files_have_no_faad_shift() {
        let (_dir, alac) = file(m4a(b"alac", Some(&[(1_058_400, 0)]), false), "a.m4a");
        let start = aac_start(&alac).unwrap().unwrap();
        assert!(!start.aac);
        assert_eq!(start.faad_shift(), 0);
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("a.m4a");
        std::fs::write(&text, b"not an mp4 at all, but long enough to try").unwrap();
        assert_eq!(aac_start(&text).unwrap(), None);
    }
}

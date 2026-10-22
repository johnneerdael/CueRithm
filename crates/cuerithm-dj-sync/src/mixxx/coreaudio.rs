//! Ask Apple's AudioFile what gapless priming it reads from an MP3, so the offset to
//! Mixxx's macOS timeline is Apple's own answer and not a guess at its tag rules.
//!
//! Mixxx decodes MP3 on macOS through CoreAudio's `ExtAudioFile`, which discards the
//! decoder delay plus the priming frames of the file's packet table (taken from a
//! LAME tag or an `iTunSMPB` comment, by Apple's rules). A file with no packet table
//! reports none. The few Apple calls are declared here directly; the module only
//! exists on macOS.
use anyhow::{Result, bail, ensure};
use std::{ffi::c_void, path::Path};

type OSStatus = i32;
type CFIndex = isize;
type CFTypeRef = *const c_void;
type AudioFileId = *mut c_void;

/// `AudioFilePacketTableInfo`; only the priming count is read, but the layout is Apple's.
#[repr(C)]
#[allow(dead_code)]
struct PacketTableInfo {
    number_valid_frames: i64,
    priming_frames: i32,
    remainder_frames: i32,
}

/// `kAudioFileReadPermission`.
const READ_PERMISSION: i8 = 0x01;
/// `kAudioFilePropertyPacketTableInfo`, `'pnfo'`.
const PACKET_TABLE_INFO: u32 = 0x706E_666F;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFURLCreateFromFileSystemRepresentation(
        allocator: CFTypeRef,
        buffer: *const u8,
        length: CFIndex,
        is_directory: u8,
    ) -> CFTypeRef;
    fn CFRelease(value: CFTypeRef);
}

#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn AudioFileOpenURL(
        url: CFTypeRef,
        permissions: i8,
        type_hint: u32,
        file: *mut AudioFileId,
    ) -> OSStatus;
    fn AudioFileGetProperty(
        file: AudioFileId,
        property: u32,
        size: *mut u32,
        data: *mut c_void,
    ) -> OSStatus;
    fn AudioFileClose(file: AudioFileId) -> OSStatus;
}

/// Leading frames of gapless priming CoreAudio finds in `path`, or `None` when the
/// file has no packet table. An error means CoreAudio cannot open the file at all,
/// in which case Mixxx could not play it either.
pub(crate) fn priming_frames(path: &Path) -> Result<Option<u32>> {
    let bytes = path.as_os_str().as_encoded_bytes();
    ensure!(!bytes.is_empty(), "CoreAudio cannot open an empty path");
    // SAFETY: every pointer passed is valid for the call; the URL and the file handle
    // are released on every path before returning, and `table` is a plain-old-data
    // struct whose size is passed alongside it.
    unsafe {
        let url = CFURLCreateFromFileSystemRepresentation(
            std::ptr::null(),
            bytes.as_ptr(),
            bytes.len() as CFIndex,
            0,
        );
        ensure!(
            !url.is_null(),
            "CoreAudio cannot form a URL for {}",
            path.display()
        );
        let mut file: AudioFileId = std::ptr::null_mut();
        let status = AudioFileOpenURL(url, READ_PERMISSION, 0, &mut file);
        CFRelease(url);
        if status != 0 || file.is_null() {
            bail!("CoreAudio cannot open {} (status {status})", path.display());
        }
        let mut table = PacketTableInfo {
            number_valid_frames: 0,
            priming_frames: 0,
            remainder_frames: 0,
        };
        let mut size = std::mem::size_of::<PacketTableInfo>() as u32;
        let status = AudioFileGetProperty(
            file,
            PACKET_TABLE_INFO,
            &mut size,
            std::ptr::from_mut(&mut table).cast(),
        );
        AudioFileClose(file);
        if status != 0 {
            return Ok(None);
        }
        Ok(u32::try_from(table.priming_frames).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixxx::decoder::{Mp3Decoder, mp3_offset_samples};

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/mixxx/fixtures")
            .join(name)
    }

    #[test]
    fn coreaudio_reports_the_lame_tag_priming_and_none_without_a_tag() {
        // Measured the same way by tools/mixxx-acceptance/coreaudio_probe.c.
        assert_eq!(priming_frames(&fixture("lame_tag.mp3")).unwrap(), Some(576));
        assert_eq!(priming_frames(&fixture("no_tag.mp3")).unwrap(), None);
    }

    #[test]
    fn unreadable_input_is_an_error_not_a_silent_zero() {
        let directory = tempfile::tempdir().unwrap();
        assert!(priming_frames(&directory.path().join("missing.mp3")).is_err());
        let junk = directory.path().join("junk.mp3");
        std::fs::write(&junk, b"this is not audio").unwrap();
        assert!(priming_frames(&junk).is_err());
        assert!(priming_frames(Path::new("")).is_err());
    }

    #[test]
    fn live_coreaudio_gives_the_measured_offsets_for_the_fixtures() {
        for (name, expected) in [("lame_tag.mp3", 0), ("no_tag.mp3", -529)] {
            let path = fixture(name);
            let start = crate::mixxx::mp3_start(&path).unwrap();
            let priming = priming_frames(&path).unwrap();
            assert_eq!(
                mp3_offset_samples(Mp3Decoder::CoreAudio, &start, priming),
                expected,
                "{name}"
            );
        }
    }
}

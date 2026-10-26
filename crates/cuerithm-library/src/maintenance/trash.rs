//! How a duplicate merge removes an audio file. It moves the file to the operating system
//! Trash or Recycle Bin, where the user can restore it. Only a file that the preview already
//! announced as "cannot go to the Trash" is ever deleted permanently, and only after the
//! Trash has been tried and has refused it.
use std::{
    collections::BTreeSet,
    path::Path,
    sync::{LazyLock, Mutex},
};

pub trait FileTrash: Send + Sync {
    /// Move one regular file to the Trash. `Err` carries a plain-language reason and means
    /// the file is exactly where it was.
    fn move_to_trash(&self, path: &Path) -> Result<(), String>;
    /// Whether the Trash can be expected to take this file. This never changes anything; it
    /// lets the preview say in advance which files would have to be deleted permanently.
    fn can_trash(&self, _path: &Path) -> bool {
        true
    }
    /// Delete one regular file permanently. It is only called for a file the user was told
    /// would be deleted permanently, after `move_to_trash` failed for it.
    fn delete_permanently(&self, path: &Path) -> Result<(), String> {
        std::fs::remove_file(path).map_err(|error| error.to_string())
    }
}

/// Volumes where the Trash refused a file during this run, so the next preview says so.
static REFUSING_VOLUMES: LazyLock<Mutex<BTreeSet<String>>> = LazyLock::new(Mutex::default);

fn volume_key(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path)
            .ok()
            .map(|meta| format!("device:{}", meta.dev()))
    }
    #[cfg(not(unix))]
    {
        path.components()
            .next()
            .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
    }
}

/// macOS keeps a Trash on every local, writable volume and has none on network shares or
/// read-only volumes. Other platforms report "yes" here and rely on what a real attempt
/// taught us (`REFUSING_VOLUMES`).
fn volume_has_trash(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: `c_path` is a valid NUL-terminated path and `stat` is a plain C struct
        // that `statfs` fills in completely when it returns 0.
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c_path.as_ptr(), &mut stat) } != 0 {
            return false;
        }
        stat.f_flags & libc::MNT_LOCAL as u32 != 0 && stat.f_flags & libc::MNT_RDONLY as u32 == 0
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        true
    }
}

/// The platform Trash: Finder-independent `NSFileManager` on macOS (no extra permission
/// prompt and no sound), the Recycle Bin on Windows, and the freedesktop.org Trash on Linux.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemTrash;
impl SystemTrash {
    #[cfg(test)]
    fn forget_refusals() {
        REFUSING_VOLUMES.lock().unwrap().clear();
    }
}
impl FileTrash for SystemTrash {
    fn move_to_trash(&self, path: &Path) -> Result<(), String> {
        #[allow(unused_mut)]
        let mut context = trash::TrashContext::default();
        #[cfg(target_os = "macos")]
        {
            use trash::macos::{DeleteMethod, TrashContextExtMacos};
            context.set_delete_method(DeleteMethod::NsFileManager);
        }
        context.delete(path).map_err(|error| {
            if let (Some(key), Ok(mut refusing)) = (volume_key(path), REFUSING_VOLUMES.lock()) {
                refusing.insert(key);
            }
            match error {
                trash::Error::CouldNotAccess { .. } | trash::Error::CanonicalizePath { .. } => {
                    "The file or its folder could not be accessed".to_owned()
                }
                other => other.to_string(),
            }
        })
    }
    fn can_trash(&self, path: &Path) -> bool {
        let refused = volume_key(path).is_some_and(|key| {
            REFUSING_VOLUMES
                .lock()
                .map(|refusing| refusing.contains(&key))
                .unwrap_or(false)
        });
        !refused && volume_has_trash(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_writable_volume_is_expected_to_have_a_trash_until_the_trash_refuses_a_file() {
        SystemTrash::forget_refusals();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("track.wav");
        std::fs::write(&file, b"audio").unwrap();
        assert!(SystemTrash.can_trash(&file));
        // A refusal on this volume is remembered for the next preview.
        REFUSING_VOLUMES
            .lock()
            .unwrap()
            .insert(volume_key(&file).unwrap());
        assert!(!SystemTrash.can_trash(&file));
        SystemTrash::forget_refusals();
        assert!(SystemTrash.can_trash(&file));
    }

    #[test]
    fn permanent_deletion_removes_exactly_the_one_file() {
        let dir = tempfile::tempdir().unwrap();
        let (gone, kept) = (dir.path().join("gone.wav"), dir.path().join("kept.wav"));
        std::fs::write(&gone, b"a").unwrap();
        std::fs::write(&kept, b"b").unwrap();
        SystemTrash.delete_permanently(&gone).unwrap();
        assert!(!gone.exists() && kept.exists());
        assert!(SystemTrash.delete_permanently(&gone).is_err());
    }

    /// Moves one tiny generated file into the real Trash of whoever runs it, so it is not part
    /// of the normal run: `cargo test -p cuerithm-library system_trash -- --ignored`.
    #[test]
    #[ignore = "moves a generated file into the real system Trash"]
    fn the_system_trash_takes_a_generated_file_and_reports_a_missing_one() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir
            .path()
            .join(format!("cuerithm-trash-check-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(
            &file,
            b"Generated by the CueRithm Trash check. Safe to empty.",
        )
        .unwrap();
        SystemTrash.move_to_trash(&file).unwrap();
        assert!(!file.exists(), "the file left its folder");
        assert!(
            SystemTrash.move_to_trash(&file).is_err(),
            "a file that is not there is reported, not ignored"
        );
        SystemTrash::forget_refusals();
    }

    /// A duplicate merge hashes each file through one open handle and keeps it open until the Trash
    /// call returns. On Windows a handle opened without FILE_SHARE_DELETE would make the Recycle Bin
    /// refuse the move; the standard library opens with it, and this checks that the real call takes
    /// a file that is still open for reading. Like the test above it moves one generated file into the
    /// real Trash of whoever runs it: `cargo test -p cuerithm-library system_trash -- --ignored`.
    #[test]
    #[ignore = "moves a generated file into the real system Trash"]
    fn the_system_trash_takes_a_file_that_is_still_open_for_reading() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(format!(
            "cuerithm-trash-check-open-{}.txt",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(
            &file,
            b"Generated by the CueRithm Trash check. Safe to empty.",
        )
        .unwrap();
        let open = std::fs::File::open(&file).unwrap();
        SystemTrash.move_to_trash(&file).unwrap();
        assert!(!file.exists(), "the file left its folder");
        drop(open);
        SystemTrash::forget_refusals();
    }
}

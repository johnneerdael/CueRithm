//! Removal for owned private copies, never original media or native targets.
use std::{fs, io, path::Path};

pub(crate) fn remove(path: &Path) -> io::Result<()> {
    match remove_once(path) {
        #[cfg(windows)]
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            // fs::copy preserves the Windows read-only attribute. A crashed
            // preparation can leave these copies after its normal guard is gone.
            clear_readonly(path)?;
            remove_once(path)
        }
        result => result,
    }
}

fn remove_once(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileTypeExt;
        if metadata.file_type().is_symlink_dir() {
            return fs::remove_dir(path);
        }
    }
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(windows)]
fn clear_readonly(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.permissions().readonly() {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            clear_readonly(&entry?.path())?;
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn removing_private_payload_links_never_changes_source_bytes_or_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::write(&source, b"original source").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
        let payload = temp.path().join("private-payload");
        fs::create_dir(&payload).unwrap();
        std::os::unix::fs::symlink(&source, payload.join("link-to-source")).unwrap();
        remove(&payload).unwrap();
        assert_eq!(fs::read(&source).unwrap(), b"original source");
        assert_eq!(
            fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            0o400
        );
    }
}

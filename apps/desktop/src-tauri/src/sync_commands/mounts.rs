use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(crate) fn scan(parents: &[PathBuf], nested: bool) -> io::Result<Vec<PathBuf>> {
    fn children(parent: &Path) -> io::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        let entries = match fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound
                        | io::ErrorKind::PermissionDenied
                        | io::ErrorKind::NotADirectory
                ) =>
            {
                return Ok(paths);
            }
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound
                            | io::ErrorKind::PermissionDenied
                            | io::ErrorKind::NotADirectory
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            let path = entry.path();
            if path.is_dir() {
                paths.push(path);
            }
        }
        Ok(paths)
    }
    let mut paths = Vec::new();
    for parent in parents {
        paths.extend(children(parent)?);
    }
    if nested {
        for parent in paths.clone() {
            paths.extend(children(&parent)?);
        }
    }
    Ok(paths)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn inaccessible_top_level_and_other_user_directories_do_not_hide_accessible_drives() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cuerithm-discovery-{}-{unique}",
            std::process::id()
        ));
        let media = root.join("media");
        let drive = media.join("current-user/DJ USB");
        let other = media.join("other-user");
        let blocked = root.join("blocked-mount-root");
        fs::create_dir_all(&drive).unwrap();
        fs::create_dir_all(other.join("private-drive")).unwrap();
        fs::create_dir_all(&blocked).unwrap();
        for path in [&other, &blocked] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
        }
        let permission_denied = [&other, &blocked].into_iter().all(|path| {
            fs::read_dir(path)
                .err()
                .is_some_and(|error| error.kind() == io::ErrorKind::PermissionDenied)
        });
        let result = scan(&[blocked.clone(), media, root.join("missing")], true);
        for path in [&other, &blocked] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
        if !permission_denied {
            return;
        } // Privileged runners bypass the fixture's permissions.
        let paths = result.unwrap();
        assert!(paths.contains(&drive));
        assert!(!paths.iter().any(|path| path.ends_with("private-drive")));
    }
}

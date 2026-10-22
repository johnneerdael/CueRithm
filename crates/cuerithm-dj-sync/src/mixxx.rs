//! Native Mixxx library adapter. It edits a staged copy of `mixxxdb.sqlite`; the
//! caller publishes it through the shared journal. See
//! `docs/research/mixxx-sync-contract.md` for the observed storage contract.
mod beats;
mod cues;
#[cfg(test)]
pub(crate) mod fixture;
mod keys;
mod owned;
mod proto;
mod schema;
mod tracks;

use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

/// File Mixxx keeps its whole library in, inside its settings folder.
pub const DATABASE_FILE: &str = "mixxxdb.sqlite";

/// Every place a Mixxx library normally lives, for all desktop systems. Callers
/// keep only the ones that exist. `local_app_data` is the Windows
/// `%LOCALAPPDATA%` when known.
pub fn database_candidates(home: &Path, local_app_data: Option<&Path>) -> Vec<PathBuf> {
    let local = local_app_data.map_or_else(|| home.join("AppData/Local"), Path::to_path_buf);
    vec![
        // macOS: Qt AppLocalDataLocation without an organization name.
        home.join("Library/Application Support/Mixxx")
            .join(DATABASE_FILE),
        // Linux native packages.
        home.join(".mixxx").join(DATABASE_FILE),
        // Linux, official Flatpak (`--persist=.mixxx`).
        home.join(".var/app/org.mixxx.Mixxx/.mixxx")
            .join(DATABASE_FILE),
        // Windows.
        local.join("Mixxx").join(DATABASE_FILE),
    ]
}

/// Accept the settings folder or the database file itself.
pub fn resolve_database_path(path: &Path) -> Result<PathBuf> {
    let database = if path.is_dir() {
        path.join(DATABASE_FILE)
    } else {
        path.to_path_buf()
    };
    ensure!(
        database
            .file_name()
            .is_some_and(|name| name == DATABASE_FILE),
        "Choose the Mixxx settings folder or its {DATABASE_FILE} file"
    );
    ensure!(
        database.is_file(),
        "Mixxx library was not found; start Mixxx once so it creates {DATABASE_FILE}"
    );
    Ok(database)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixxx_candidates_cover_every_platform_layout() {
        let home = Path::new("/home/dj");
        let found = database_candidates(home, None);
        assert_eq!(
            found,
            vec![
                PathBuf::from("/home/dj/Library/Application Support/Mixxx/mixxxdb.sqlite"),
                PathBuf::from("/home/dj/.mixxx/mixxxdb.sqlite"),
                PathBuf::from("/home/dj/.var/app/org.mixxx.Mixxx/.mixxx/mixxxdb.sqlite"),
                PathBuf::from("/home/dj/AppData/Local/Mixxx/mixxxdb.sqlite"),
            ]
        );
        let redirected = database_candidates(home, Some(Path::new("/data/Local")));
        assert_eq!(
            redirected.last().unwrap(),
            &PathBuf::from("/data/Local/Mixxx/mixxxdb.sqlite")
        );
    }

    #[test]
    fn resolve_database_path_accepts_folder_or_file_and_rejects_other_names() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("settings");
        std::fs::create_dir(&folder).unwrap();
        // A folder without a library is a clear error, not a created file.
        assert!(
            resolve_database_path(&folder)
                .unwrap_err()
                .to_string()
                .contains("start Mixxx once")
        );
        let database = folder.join(DATABASE_FILE);
        std::fs::write(&database, b"x").unwrap();
        assert_eq!(resolve_database_path(&folder).unwrap(), database);
        assert_eq!(resolve_database_path(&database).unwrap(), database);
        let other = folder.join("other.sqlite");
        std::fs::write(&other, b"x").unwrap();
        assert!(
            resolve_database_path(&other)
                .unwrap_err()
                .to_string()
                .contains("settings folder")
        );
        assert!(resolve_database_path(&folder.join("missing")).is_err());
    }
}

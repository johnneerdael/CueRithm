//! The folders Mixxx scans (the `directories` table).
//!
//! While that table is empty Mixxx shows "Choose music library directory" every time it
//! starts (`CoreServices::initialize`: `loadRootDirs().isEmpty()`, which counts a row
//! whether or not its folder still exists). A library CueRithm fills would keep asking,
//! so an empty list is seeded with the folders the synced tracks are in, as the dialog
//! would have done for one folder. A list the user has built is never edited.
use super::is_under;
use anyhow::Result;
use rusqlite::{OptionalExtension, Transaction};
use std::{collections::BTreeSet, path::Path};

/// What seeding did.
#[derive(Debug, Default)]
pub(super) struct Seeded {
    /// Folders written to `directories`, outermost only.
    pub added: Vec<String>,
    /// Tracks directly in a drive root or the home folder, which are not added because
    /// Mixxx would scan everything below them.
    pub too_broad: usize,
}

/// Home folder as `track_locations` would spell it, for the "too broad" check.
pub(super) fn home_directory() -> Option<String> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    super::tracks::location_key(&std::fs::canonicalize(home).ok()?).ok()
}

fn is_drive_root(directory: &str) -> bool {
    directory == "/" || directory.trim_end_matches('/').ends_with(':')
}

/// If `directories` is empty, add the outermost folders that hold the tracks with
/// `library_ids`. Mixxx drops a folder inside another one when adding (`DirectoryDAO::
/// addDirectory`), so only the outermost are written, and only folders that exist.
pub(super) fn seed(
    tx: &Transaction<'_>,
    library_ids: impl IntoIterator<Item = i64>,
    home: Option<&str>,
    case_insensitive: bool,
) -> Result<Seeded> {
    let existing: i64 = tx.query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))?;
    if existing > 0 {
        return Ok(Seeded::default());
    }
    let mut seeded = Seeded::default();
    let mut folders = BTreeSet::new();
    for id in library_ids {
        let directory: Option<String> = tx
            .query_row(
                "SELECT t.directory FROM library l JOIN track_locations t ON t.id = l.location
                 WHERE l.id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let Some(directory) = directory.filter(|d| !d.is_empty()) else {
            continue;
        };
        let trimmed = directory.trim_end_matches('/');
        let broad = is_drive_root(&directory)
            || home.is_some_and(|home| {
                let home = home.trim_end_matches('/');
                if case_insensitive {
                    trimmed.eq_ignore_ascii_case(home)
                } else {
                    trimmed == home
                }
            });
        if broad {
            seeded.too_broad += 1;
        } else if Path::new(&directory).is_dir() {
            folders.insert(directory);
        }
    }
    let mut by_depth: Vec<String> = folders.into_iter().collect();
    by_depth.sort_by_key(|folder| folder.len());
    for folder in by_depth {
        if !seeded
            .added
            .iter()
            .any(|outer| is_under(&folder, outer, case_insensitive))
        {
            seeded.added.push(folder);
        }
    }
    seeded.added.sort();
    for folder in &seeded.added {
        tx.execute(
            "INSERT OR IGNORE INTO directories(directory) VALUES (?1)",
            [folder],
        )?;
    }
    Ok(seeded)
}

/// The sentence for the preview. Lists up to three folders.
pub(super) fn describe(seeded: &Seeded) -> Option<String> {
    let mut parts = Vec::new();
    if !seeded.added.is_empty() {
        let shown: Vec<&str> = seeded.added.iter().take(3).map(String::as_str).collect();
        let more = seeded.added.len() - shown.len();
        parts.push(format!(
            "Mixxx had no music folder, so it would have asked for one every time it starts. \
             CueRithm added {} to Mixxx's scan list: {}{}.",
            if seeded.added.len() == 1 {
                "the folder that holds the synced tracks".to_owned()
            } else {
                format!(
                    "the {} folders that hold the synced tracks",
                    seeded.added.len()
                )
            },
            shown.join(", "),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        ));
    }
    if seeded.too_broad > 0 {
        parts.push(format!(
            "{} track(s) are directly in your home folder or a drive root. CueRithm did not add \
             that as a scan folder, because Mixxx would scan everything below it; choose a \
             folder in Mixxx's Library preferences.",
            seeded.too_broad
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixxx::fixture;
    use rusqlite::Connection;

    fn listed(connection: &Connection) -> Vec<String> {
        connection
            .prepare("SELECT directory FROM directories ORDER BY directory")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn the_home_folder_and_drive_roots_are_never_added() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().to_str().unwrap().to_owned();
        std::fs::create_dir(temp.path().join("sub")).unwrap();
        let mut connection = fixture::create(&temp.path().join("mixxxdb.sqlite"), 39);
        let ids = [
            fixture::add_track(&connection, &format!("{home}/a.mp3"), "home"),
            fixture::add_track(&connection, &format!("{home}/sub/b.mp3"), "sub"),
            fixture::add_track(&connection, "C:/c.mp3", "root"),
            fixture::add_track(&connection, "/d.mp3", "unix root"),
        ];
        let tx = connection.transaction().unwrap();
        let seeded = seed(&tx, ids, Some(&home), false).unwrap();
        tx.commit().unwrap();
        assert_eq!(seeded.added, vec![format!("{home}/sub")]);
        assert_eq!(seeded.too_broad, 2, "the home folder and C:");
        assert_eq!(listed(&connection), seeded.added);
        let text = describe(&seeded).unwrap();
        assert!(text.contains("2 track(s) are directly in your home folder or a drive root"));
    }

    #[test]
    fn a_folder_that_does_not_exist_is_not_added_and_nothing_to_add_says_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let mut connection = fixture::create(&temp.path().join("mixxxdb.sqlite"), 39);
        let id = fixture::add_track(&connection, "/no/such/folder/a.mp3", "gone");
        let tx = connection.transaction().unwrap();
        let seeded = seed(&tx, [id], None, false).unwrap();
        tx.commit().unwrap();
        assert!(seeded.added.is_empty() && describe(&seeded).is_none());
        assert!(listed(&connection).is_empty());
    }

    #[test]
    fn nested_folders_collapse_to_the_outer_one() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().to_str().unwrap().to_owned();
        std::fs::create_dir_all(temp.path().join("Music/Deep")).unwrap();
        let mut connection = fixture::create(&temp.path().join("mixxxdb.sqlite"), 39);
        let ids = [
            fixture::add_track(&connection, &format!("{base}/Music/Deep/a.mp3"), "a"),
            fixture::add_track(&connection, &format!("{base}/Music/b.mp3"), "b"),
        ];
        let tx = connection.transaction().unwrap();
        let seeded = seed(&tx, ids, None, true).unwrap();
        tx.commit().unwrap();
        assert_eq!(seeded.added, vec![format!("{base}/Music")]);
    }
}

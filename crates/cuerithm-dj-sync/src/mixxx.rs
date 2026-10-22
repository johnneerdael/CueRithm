//! Native Mixxx library adapter. It edits a staged copy of `mixxxdb.sqlite`; the
//! caller publishes it through the shared journal. See
//! `docs/research/mixxx-sync-contract.md` for the observed storage contract.
#[cfg(test)]
mod acceptance;
mod beats;
#[cfg(target_os = "macos")]
mod coreaudio;
mod cues;
mod decoder;
#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
mod keys;
mod lame;
mod owned;
mod proto;
mod repair;
mod schema;
#[cfg(test)]
mod staged_tests;
#[cfg(test)]
mod sync_tests;
pub(crate) mod tracks;
mod verify;

use crate::{
    SyncLibrary, SyncOptions,
    ownership::{PlaylistOwnershipReport, PlaylistOwnershipRequest},
};
use anyhow::{Context, Result, ensure};
pub(crate) use decoder::{Mp3Decoder, mp3_offset_samples};
pub(crate) use lame::mp3_start;
pub(crate) use repair::{reference_id, repair_reference};
use rusqlite::Connection;
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

/// Refuse a library a crashed Mixxx left half-written. Runs before anything is read.
pub(crate) fn preflight(database: &Path) -> Result<()> {
    schema::ensure_closed_cleanly(database)
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

/// CoreAudio's gapless priming for `path`. Only asked when CoreAudio is the decoder,
/// which is only possible on macOS; elsewhere there is nothing to ask.
pub(crate) fn coreaudio_priming(path: &Path, decoder: Mp3Decoder) -> Result<Option<u32>> {
    #[cfg(target_os = "macos")]
    if decoder == Mp3Decoder::CoreAudio {
        return coreaudio::priming_frames(path);
    }
    let _ = (path, decoder);
    Ok(None)
}

/// Facts about the system Mixxx runs on that change what is written or reported.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Host {
    /// A and a differ only by case on this system's file systems.
    pub case_insensitive: bool,
    /// How this system's Mixxx decodes MP3 and AAC.
    pub decoder: Mp3Decoder,
}
impl Host {
    pub(crate) fn current() -> Self {
        Self {
            case_insensitive: tracks::case_insensitive_host(),
            decoder: Mp3Decoder::host(),
        }
    }
}

/// What a staged write produced.
#[derive(Debug)]
pub struct StagedMixxx {
    /// Playlist and crate bindings to record locally once the sync is published.
    pub ownership: PlaylistOwnershipReport,
    /// A summary line first, then anything the user should know before applying.
    pub warnings: Vec<String>,
    /// False when the staged library already matches, so nothing needs publishing.
    pub changed: bool,
}

#[cfg(test)]
type AfterWrite = Box<dyn FnOnce(&Connection)>;
#[cfg(test)]
thread_local! {
    static AFTER_WRITE: std::cell::RefCell<Option<AfterWrite>> = const { std::cell::RefCell::new(None) };
}
/// Run `hook` on the staged connection after the writes and before verification.
#[cfg(test)]
pub(crate) fn after_write(hook: impl FnOnce(&Connection) + 'static) {
    AFTER_WRITE.with_borrow_mut(|slot| *slot = Some(Box::new(hook)));
}

/// Write `library` into the staged copy of a Mixxx library. `stage` is a private copy
/// of `database`, which is only used to describe the destination (for example a
/// Flatpak install). Nothing here touches the live file.
pub fn write_staged_owned(
    stage: &Path,
    database: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
    request: &PlaylistOwnershipRequest,
) -> Result<StagedMixxx> {
    write_staged(stage, database, library, options, request, Host::current())
}

fn write_staged(
    stage: &Path,
    database: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
    request: &PlaylistOwnershipRequest,
    host: Host,
) -> Result<StagedMixxx> {
    let mut connection = Connection::open(stage).context("Cannot open the staged Mixxx library")?;
    let layout = schema::inspect(&connection)?;
    let snapshot = verify::Snapshot::capture(&connection)?;
    let mut warnings = Vec::new();
    let transaction = connection.transaction()?;
    let before = transaction.total_changes();
    let tracks = tracks::write_tracks(
        &transaction,
        &layout,
        library,
        options,
        host.case_insensitive,
        &mut warnings,
    )
    .with_context(|| {
        format!(
            "Writing tracks to a Mixxx schema {} library",
            layout.version
        )
    })?;
    let containers = owned::write_containers(
        &transaction,
        library,
        options,
        request,
        &tracks.library_ids,
        &mut warnings,
    )
    .with_context(|| {
        format!(
            "Writing playlists to a Mixxx schema {} library",
            layout.version
        )
    })?;
    #[cfg(test)]
    AFTER_WRITE.with_borrow_mut(|slot| {
        if let Some(hook) = slot.take() {
            hook(&transaction);
        }
    });
    let changed = transaction.total_changes() > before;
    if changed {
        snapshot.check(&transaction, &tracks)?;
        transaction.commit()?;
    } else {
        transaction.rollback()?;
    }
    let check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    ensure!(
        check == "ok",
        "The staged Mixxx library failed its integrity check: {check}"
    );
    advisories(&connection, database, library, host, &mut warnings)?;
    warnings.insert(0, summary(&tracks, &containers));
    Ok(StagedMixxx {
        ownership: containers.report,
        warnings,
        changed,
    })
}

fn summary(tracks: &tracks::TrackOutcome, containers: &owned::ContainerOutcome) -> String {
    let plural =
        |count: usize, noun: &str| format!("{count} {noun}{}", if count == 1 { "" } else { "s" });
    let mut parts = vec![
        format!("{} added", plural(tracks.added, "track")),
        format!("{} replaced", tracks.replaced),
        format!("{} left unchanged", tracks.kept),
    ];
    if tracks.restored > 0 {
        parts.push(format!("{} made visible again", tracks.restored));
    }
    if tracks.replaced_cues > 0 {
        parts.push(format!(
            "{} replaced",
            plural(tracks.replaced_cues, "hot cue or loop")
        ));
    }
    parts.push(format!(
        "{} created, {} updated and {} removed",
        plural(containers.created, "playlist or crate"),
        containers.updated,
        containers.deleted
    ));
    format!("Mixxx: {}.", parts.join("; "))
}

fn is_under(key: &str, directory: &str, case_insensitive: bool) -> bool {
    let (key, directory) = if case_insensitive {
        (key.to_lowercase(), directory.to_lowercase())
    } else {
        (key.to_owned(), directory.to_owned())
    };
    let directory = directory.trim_end_matches('/');
    key == directory || key.starts_with(&format!("{directory}/"))
}

/// Facts that do not stop a sync but change what the user will see in Mixxx.
fn advisories(
    connection: &Connection,
    database: &Path,
    library: &SyncLibrary,
    host: Host,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let case_insensitive = host.case_insensitive;
    let mut keys = Vec::new();
    let (mut aac, mut wma) = (0usize, 0usize);
    for track in &library.tracks {
        keys.push(tracks::location_key(&std::fs::canonicalize(&track.path)?)?);
        let extension = track
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match extension.as_str() {
            "m4a" | "mp4" | "aac" => aac += 1,
            "wma" => wma += 1,
            _ => {}
        }
    }
    // Measured on macOS, where Mixxx decodes AAC with CoreAudio; elsewhere it uses
    // Media Foundation (Windows) or FAAD (Linux), which this has not been run against.
    if aac > 0 && host.decoder == Mp3Decoder::Mad {
        warnings.push(format!(
            "{aac} AAC track(s): Mixxx on this system decodes AAC with Media Foundation or FAAD, \
             which CueRithm has not verified, so their cues and grid may sit a few milliseconds \
             off."
        ));
    }
    if wma > 0 {
        warnings.push(format!(
            "{wma} WMA track(s): Mixxx plays WMA only when it was built with FFmpeg or, on \
             Windows, Media Foundation, and CueRithm has not verified how it aligns them."
        ));
    }
    let mut statement = connection.prepare("SELECT directory FROM directories")?;
    let scanned = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !scanned.is_empty() {
        let outside = keys
            .iter()
            .filter(|key| {
                !scanned
                    .iter()
                    .any(|dir| is_under(key, &dir.replace('\\', "/"), case_insensitive))
            })
            .count();
        if outside > 0 {
            warnings.push(format!(
                "{outside} track(s) are outside the folders Mixxx scans. They stay in the \
                 library, but Mixxx will not find new files next to them."
            ));
        }
    }
    let text = database.to_string_lossy().replace('\\', "/");
    if let Some((home, _)) = text.split_once("/.var/app/org.mixxx.Mixxx/") {
        let visible = [
            format!("{home}/Music"),
            "/media".to_owned(),
            "/run/media".to_owned(),
        ];
        let hidden = keys
            .iter()
            .filter(|key| !visible.iter().any(|root| is_under(key, root, false)))
            .count();
        if hidden > 0 {
            warnings.push(format!(
                "{hidden} track(s) are outside the folders the Flatpak version of Mixxx can \
                 read (Music, /media and /run/media). Grant it access or Mixxx will show them \
                 as missing."
            ));
        }
    }
    Ok(())
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

//! Native Mixxx library adapter. It edits a staged copy of `mixxxdb.sqlite`; the
//! caller publishes it through the shared journal. See
//! `docs/research/mixxx-sync-contract.md` for the observed storage contract.
#[cfg(test)]
mod acceptance;
mod beats;
#[cfg(target_os = "macos")]
mod coreaudio;
mod create;
mod cues;
mod decoder;
#[cfg(any(test, feature = "test-support"))]
pub mod fixture;
mod folders;
mod keys;
mod lame;
mod mp4;
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
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
#[cfg(test)]
pub(crate) use decoder::pretend_aac;
pub(crate) use decoder::{AacDecoder, Mp3Decoder, aac_decoder, mp3_offset_samples};
pub(crate) use lame::mp3_start;
#[cfg(test)]
pub(crate) use mp4::sample::m4a as sample_m4a;
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
        // macOS, official build: it is App Sandboxed, so Qt's AppLocalDataLocation is
        // inside its container (Mixxx 2.5.6 reports exactly this path in its log).
        home.join("Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx")
            .join(DATABASE_FILE),
        // macOS, a build that is not sandboxed (for example one built from source).
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

/// Where a Mixxx library is or would be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseTarget {
    pub database: PathBuf,
    /// False when there is no library yet and a sync would create one.
    pub exists: bool,
}

/// Accept the settings folder or the database file, either of which may not exist yet.
/// A folder that does not exist must at least have a parent, which is all CueRithm
/// creates; a Mixxx that was never started has no folder on Windows and Linux.
pub fn resolve_database_target(path: &Path) -> Result<DatabaseTarget> {
    let database = if path.is_dir() {
        path.join(DATABASE_FILE)
    } else if path.file_name().is_some_and(|name| name == DATABASE_FILE) {
        path.to_path_buf()
    } else if path.exists() {
        anyhow::bail!("Choose the Mixxx settings folder or its {DATABASE_FILE} file");
    } else {
        path.join(DATABASE_FILE)
    };
    let exists = database.is_file();
    if !exists {
        ensure!(!database.exists(), "{} is not a file", database.display());
        let folder = database.parent().context("Mixxx settings folder missing")?;
        let parent = folder.parent().filter(|p| !p.as_os_str().is_empty());
        ensure!(
            folder.is_dir() || parent.is_some_and(Path::is_dir),
            "Neither the Mixxx settings folder nor the folder it would be in exists; \
             check the location, or on macOS start Mixxx once so the system creates its container"
        );
    }
    Ok(DatabaseTarget { database, exists })
}

/// Frames later than CueRithm's clock that FAAD plays an M4A, which does not skip the encoder's
/// priming (0 for Apple Lossless and anything that is not an M4A with AAC).
pub(crate) fn faad_shift_frames(path: &Path) -> Result<u32> {
    Ok(mp4::aac_start(path)?.map_or(0, mp4::AacStart::faad_shift))
}

/// The folder to watch for changes between preview and apply: the settings folder, or
/// its parent while a Mixxx that was never started has not made one.
pub fn watched_folder(path: &Path) -> Result<PathBuf> {
    let database = resolve_database_target(path)?.database;
    let folder = database.parent().context("Mixxx settings folder missing")?;
    if folder.is_dir() {
        Ok(folder.to_path_buf())
    } else {
        Ok(folder
            .parent()
            .context("Mixxx settings folder missing")?
            .to_path_buf())
    }
}

/// The canonical library path for a settings folder or file, which may not exist yet:
/// the nearest existing ancestor, canonicalized, with the missing names appended. It
/// stays the same once a sync has created the library, so a Mixxx that was never
/// started has one identity before and after.
pub fn canonical_database_path(path: &Path) -> Result<PathBuf> {
    let database = resolve_database_target(path)?.database;
    let mut missing = Vec::new();
    let mut existing = database.as_path();
    while !existing.exists() {
        missing.push(
            existing
                .file_name()
                .context("The Mixxx location has no existing ancestor")?,
        );
        existing = existing
            .parent()
            .context("The Mixxx location has no existing ancestor")?;
    }
    let mut canonical = std::fs::canonicalize(existing)?;
    canonical.extend(missing.iter().rev());
    Ok(canonical)
}

/// Create a new library at `path` (see `create`).
pub(crate) fn create_library(path: &Path) -> Result<()> {
    create::create(path)
}

/// For the preview of a replacement: what the library at `path` holds that goes.
pub(crate) fn describe_replaced(path: &Path) -> Result<String> {
    let connection = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    create::describe_replaced(&connection)
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
    /// Windows, where Mixxx decodes AAC with Media Foundation. The MP3 decoder cannot say so:
    /// libmad decodes MP3 on Linux and Windows alike. Taken from the host being described and
    /// not from the system the code was built for, so tests can describe any of the three.
    pub windows: bool,
}
impl Host {
    pub(crate) fn current() -> Self {
        Self {
            case_insensitive: tracks::fold_case_candidates(),
            decoder: Mp3Decoder::host(),
            windows: cfg!(windows),
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
    // Mixxx runs with foreign keys off, and its own schema could not be written with
    // them on: `library.location` references `track_locations(location)` but holds an
    // id, and several tables reference a `library_old` that no longer exists. The
    // SQLite bundled here turns them on by default.
    connection.pragma_update(None, "foreign_keys", false)?;
    let layout = schema::inspect(&connection)?;
    let mut warnings: Vec<String> = layout.advisory().into_iter().collect();
    let snapshot = verify::Snapshot::capture(&connection)?;
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
    // Mixxx asks for a music folder at every start while its list is empty.
    let seeded = folders::seed(
        &transaction,
        tracks.library_ids.values().copied(),
        folders::home_directory().as_deref(),
        host.case_insensitive,
    )?;
    warnings.extend(folders::describe(&seeded));
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
    advisories(
        &connection,
        database,
        library,
        host,
        &seeded.added,
        &mut warnings,
    )?;
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

/// Whether `key` is `directory` or inside it. Where case folding is on, a match that
/// differs only by case also needs the file system to say the leading folder is that
/// directory: on a case-sensitive volume `/Music/A` and `/Music/a` are two folders.
fn is_under(key: &str, directory: &str, case_insensitive: bool) -> bool {
    let directory = directory.trim_end_matches('/');
    if key == directory || key.starts_with(&format!("{directory}/")) {
        return true;
    }
    if !case_insensitive {
        return false;
    }
    let (folded_key, folded_directory) = (key.to_lowercase(), directory.to_lowercase());
    if folded_key != folded_directory && !folded_key.starts_with(&format!("{folded_directory}/")) {
        return false;
    }
    // The leading components of `key` that stand for `directory`.
    let depth = directory.split('/').count();
    let leading = key.split('/').take(depth).collect::<Vec<_>>().join("/");
    tracks::file_check()(&leading, directory)
}

/// Facts that do not stop a sync but change what the user will see in Mixxx.
fn advisories(
    connection: &Connection,
    database: &Path,
    library: &SyncLibrary,
    host: Host,
    seeded: &[String],
    warnings: &mut Vec<String>,
) -> Result<()> {
    let case_insensitive = host.case_insensitive;
    let mut keys = Vec::new();
    let (mut aac, mut wma) = (0usize, 0usize);
    // AAC tracks whose priming FAAD would not skip: the frames, at the track's rate.
    let mut late: Vec<(u32, u32)> = Vec::new();
    for track in &library.tracks {
        keys.push(tracks::location_key(&std::fs::canonicalize(&track.path)?)?);
        let extension = track
            .path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match extension.as_str() {
            "m4a" | "mp4" => {
                aac += 1;
                let shift = faad_shift_frames(&track.path).unwrap_or(0);
                if shift > 0 {
                    late.push((shift, track.sample_rate));
                }
            }
            "aac" => aac += 1,
            "wma" => wma += 1,
            _ => {}
        }
    }
    // AAC. Measured: CoreAudio (macOS) and FFmpeg (Debian's Mixxx 2.5.0) drop the encoder's
    // priming like CueRithm's clock does; FAAD (Mixxx built with it) does not, so its timeline
    // is `priming - 1024` frames later (see `mp4.rs`), which the timing step applies when it
    // finds FAAD. Media Foundation (Windows) was not run.
    if aac > 0 && host.decoder == Mp3Decoder::Mad {
        let latest_ms = late
            .iter()
            .filter(|(_, rate)| *rate > 0)
            .map(|(frames, rate)| f64::from(*frames) * 1000.0 / f64::from(*rate))
            .fold(0.0, f64::max);
        if host.windows {
            warnings.push(format!(
                "{aac} AAC track(s): Mixxx on Windows decodes AAC with Media Foundation, which \
                 CueRithm has not checked, so their cues and grid may sit a few milliseconds off."
            ));
        } else if !late.is_empty() {
            match aac_decoder() {
                AacDecoder::Faad => warnings.push(format!(
                    "{} AAC track(s) come from an encoder with more priming than 1024 samples \
                     (Apple's, for one). Your Mixxx decodes AAC with FAAD, which does not skip \
                     it, so CueRithm moved their grid and cues up to {:.0} ms later to match.",
                    late.len(),
                    latest_ms
                )),
                AacDecoder::Unknown => warnings.push(format!(
                    "{} AAC track(s) come from an encoder with more priming than 1024 samples \
                     (Apple's, for one). CueRithm could not tell which AAC decoder your Mixxx \
                     uses: with FFmpeg (Debian's) their cues are right, with FAAD (builds that \
                     include it) they sit about {:.0} ms late.",
                    late.len(),
                    latest_ms
                )),
                AacDecoder::TrimsPriming => {}
            }
        }
    }
    if wma > 0 {
        warnings.push(if host.decoder == Mp3Decoder::CoreAudio {
            // Read from the debug log of the macOS download, Mixxx 2.5.6: it lists no
            // decoder for wma.
            format!(
                "{wma} WMA track(s): the Mixxx download for macOS has no WMA decoder (checked \
                 in Mixxx 2.5.6), so they are added to the library but will not play there."
            )
        } else {
            format!(
                "{wma} WMA track(s): Mixxx plays WMA only when it was built with FFmpeg or, on \
                 Windows, Media Foundation, and CueRithm has not verified how it aligns them."
            )
        });
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
    // The official macOS build is App Sandboxed: it reads the Music and Downloads
    // folders and folders the user has chosen in a dialog, and asks for the rest.
    if let Some((home, _)) = text.split_once("/Library/Containers/org.mixxx.mixxx/") {
        // Only a folder the user picked in Mixxx's own dialog has a security-scoped
        // bookmark (`Library::requestAddDir`), and those are in `sandbox.cfg` under the
        // base64 of the folder's path (`Sandbox::keyForCanonicalPath`). A folder in
        // `directories` without one, such as one CueRithm seeded, is not readable.
        let bookmarks =
            std::fs::read_to_string(database.with_file_name("sandbox.cfg")).unwrap_or_default();
        let bookmarked = |dir: &str| {
            let mut path = dir.trim_end_matches('/');
            loop {
                if bookmarks.contains(&BASE64.encode(path)) {
                    return true;
                }
                match path.rsplit_once('/') {
                    Some((parent, _)) if !parent.is_empty() => path = parent,
                    _ => return false,
                }
            }
        };
        // Mixxx's own container is always readable to it.
        let granted: Vec<String> = [
            format!("{home}/Music"),
            format!("{home}/Downloads"),
            format!("{home}/Library/Containers/org.mixxx.mixxx/Data"),
        ]
        .into_iter()
        .chain(
            scanned
                .iter()
                .filter(|dir| !seeded.contains(dir) && bookmarked(dir))
                .map(|dir| dir.replace('\\', "/")),
        )
        .collect();
        let blocked = keys
            .iter()
            .filter(|key| !granted.iter().any(|root| is_under(key, root, true)))
            .count();
        if blocked > 0 {
            warnings.push(format!(
                "{blocked} track(s) are outside your Music and Downloads folders and the folders \
                 you added to Mixxx. Mixxx on macOS runs in a sandbox and will ask you to \
                 choose each such folder the first time it opens a track there."
            ));
        }
    }
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
    fn a_case_only_match_of_a_folder_needs_the_file_system_to_agree() {
        // Same spelling: no question. Different case: only if the file system says the
        // leading folder is the directory.
        assert!(is_under("/Music/a/x.mp3", "/Music/a", false));
        assert!(!is_under("/Music/A/x.mp3", "/Music/a", false));
        {
            let _same = tracks::pretend_same_file(|leading, directory| {
                assert_eq!((leading, directory), ("/Music/A", "/Music/a"));
                true
            });
            assert!(is_under("/Music/A/x.mp3", "/Music/a", true));
        }
        {
            let _different = tracks::pretend_same_file(|_, _| false);
            assert!(!is_under("/Music/A/x.mp3", "/Music/a", true));
            assert!(!is_under("/Music/A", "/Music/a/", true));
            assert!(
                is_under("/Music/a/x.mp3", "/Music/a/", true),
                "exact needs no check"
            );
        }
        assert!(!is_under("/Other/x.mp3", "/Music/a", true));
    }

    #[test]
    fn mixxx_candidates_cover_every_platform_layout() {
        let home = Path::new("/home/dj");
        let found = database_candidates(home, None);
        assert_eq!(
            found,
            vec![
                PathBuf::from(
                    "/home/dj/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/mixxxdb.sqlite"
                ),
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

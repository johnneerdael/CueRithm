//! Track identity, insertion, replacement and cue rows for one sync.
use super::{
    beats::{self, BeatPlan},
    cues::{self, CueRow},
    keys::{self, MixxxKey},
    proto,
    schema::Layout,
};
use crate::{BeatgridLock, MixxxExistingTracks, SyncLibrary, SyncOptions, SyncTrack};
use anyhow::{Context, Result, ensure};
use rusqlite::{Transaction, params, params_from_iter, types::Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Whether this host's default file systems treat `A.mp3` and `a.mp3` as one file.
pub(super) fn case_insensitive_host() -> bool {
    cfg!(any(target_os = "macos", windows))
}

/// Whether two location texts name one file on disk. A case-folded match is only a match
/// when this says so: macOS and Windows are usually case-insensitive but can have
/// case-sensitive volumes, where `A.mp3` and `a.mp3` are two files.
pub(super) fn same_file(a: &str, b: &str) -> bool {
    let (a, b) = (Path::new(a), Path::new(b));
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        matches!(
            (std::fs::metadata(a), std::fs::metadata(b)),
            (Ok(x), Ok(y)) if x.dev() == y.dev() && x.ino() == y.ino()
        )
    }
    #[cfg(not(unix))]
    {
        matches!(
            (std::fs::canonicalize(a), std::fs::canonicalize(b)),
            (Ok(x), Ok(y)) if x == y
        )
    }
}

type FileCheck = fn(&str, &str) -> bool;
#[cfg(test)]
thread_local! {
    static SAME_FILE: std::cell::Cell<Option<FileCheck>> = const { std::cell::Cell::new(None) };
}
/// Test seam: what `same_file` answers until the guard is dropped. Tests build locations
/// that do not exist on disk, and must not depend on the file system they run on.
#[cfg(test)]
pub(super) struct PretendSameFile;
#[cfg(test)]
impl Drop for PretendSameFile {
    fn drop(&mut self) {
        SAME_FILE.set(None);
    }
}
#[cfg(test)]
pub(super) fn pretend_same_file(check: FileCheck) -> PretendSameFile {
    SAME_FILE.set(Some(check));
    PretendSameFile
}
pub(super) fn file_check() -> FileCheck {
    #[cfg(test)]
    if let Some(check) = SAME_FILE.get() {
        return check;
    }
    same_file
}

/// `track_locations.location` for a canonical path: forward slashes on Windows, the
/// verbatim `\\?\` prefix removed, `\\?\UNC\server\share` as `//server/share`.
/// Mixxx stores text, so a path that is not valid Unicode cannot be recorded.
pub(super) fn location_key(path: &Path) -> Result<String> {
    let reference = crate::destination_reference(path);
    let text = reference
        .to_str()
        .context("Mixxx stores Unicode paths; this file name is not valid Unicode")?;
    let windows_style = text.starts_with("\\\\")
        || (text.as_bytes().get(1) == Some(&b':')
            && text
                .as_bytes()
                .get(2)
                .is_some_and(|b| matches!(b, b'\\' | b'/')));
    Ok(if windows_style {
        text.replace('\\', "/")
    } else {
        text.to_owned()
    })
}

/// One `track_locations` row and the `library` row that points at it, if any.
#[derive(Debug, Clone, Copy)]
struct Found {
    location_id: i64,
    library_id: Option<i64>,
}

/// Every known location, for matching by exact text or, where the file system folds
/// case, by lower-cased text.
struct Index {
    exact: BTreeMap<String, Found>,
    folded: BTreeMap<String, Vec<String>>,
    case_insensitive: bool,
}
impl Index {
    fn load(tx: &Transaction<'_>, case_insensitive: bool) -> Result<Self> {
        let mut statement = tx.prepare(
            "SELECT t.location, t.id, MIN(l.id) FROM track_locations t
             LEFT JOIN library l ON l.location = t.id WHERE t.location IS NOT NULL
             GROUP BY t.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                Found {
                    location_id: row.get(1)?,
                    library_id: row.get(2)?,
                },
            ))
        })?;
        let mut index = Self {
            exact: BTreeMap::new(),
            folded: BTreeMap::new(),
            case_insensitive,
        };
        for row in rows {
            let (location, found) = row?;
            index.insert(location, found);
        }
        Ok(index)
    }
    fn insert(&mut self, location: String, found: Found) {
        if self.case_insensitive {
            self.folded
                .entry(location.to_lowercase())
                .or_default()
                .push(location.clone());
        }
        self.exact.insert(location, found);
    }
    fn find(&self, key: &str) -> Result<Option<Found>> {
        if let Some(found) = self.exact.get(key) {
            return Ok(Some(*found));
        }
        if !self.case_insensitive {
            return Ok(None);
        }
        let Some(candidates) = self.folded.get(&key.to_lowercase()) else {
            return Ok(None);
        };
        // A different spelling is only the same track when the file system agrees.
        let same = file_check();
        let candidates: Vec<&String> = candidates
            .iter()
            .filter(|candidate| same(key, candidate))
            .collect();
        if candidates.is_empty() {
            return Ok(None);
        }
        ensure!(
            candidates.len() == 1,
            "Mixxx library holds {} rows for this file ({}); remove the duplicates in Mixxx first",
            candidates.len(),
            candidates
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(Some(self.exact[candidates[0]]))
    }
}

#[derive(Debug, Default)]
pub(super) struct TrackOutcome {
    /// CueRithm track id to `library.id`, for playlists.
    pub library_ids: BTreeMap<String, i64>,
    pub added: usize,
    pub replaced: usize,
    pub kept: usize,
    /// Tracks made visible again (`mixxx_deleted` or `fs_deleted` cleared).
    pub restored: usize,
    pub replaced_cues: usize,
    /// Rows this sync inserted, and rows whose beat or key blobs it wrote. Only
    /// these are decoded by the readback check; other rows hold Mixxx's own data.
    pub created: BTreeSet<i64>,
    pub grids: BTreeSet<i64>,
    pub keys: BTreeSet<i64>,
}

fn filetype(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "aif" {
        "aiff".into()
    } else {
        extension
    }
}

/// Channel count and bitrate in kbit/s, read without modifying the file. Mixxx
/// refreshes exact stream facts the first time it decodes the track.
fn probe_audio(path: &Path) -> Option<(u32, u32)> {
    use lofty::file::AudioFile;
    let tagged = lofty::probe::Probe::open(path).ok()?.read().ok()?;
    let properties = tagged.properties();
    Some((
        u32::from(properties.channels()?),
        properties.audio_bitrate().unwrap_or(0),
    ))
}

pub(super) fn split_location(key: &str) -> (String, String) {
    match key.rsplit_once('/') {
        Some((directory, name)) => {
            let directory = if directory.is_empty() || directory.ends_with(':') {
                format!("{directory}/")
            } else {
                directory.to_owned()
            };
            (directory, name.to_owned())
        }
        None => (String::new(), key.to_owned()),
    }
}

/// What CueRithm contributes for one track, computed once.
struct Plan {
    beats: Option<BeatPlan>,
    key: Option<MixxxKey>,
    cues: Vec<CueRow>,
}
fn plan(track: &SyncTrack, options: &SyncOptions, warnings: &mut Vec<String>) -> Result<Plan> {
    let name = if track.title.is_empty() {
        &track.id
    } else {
        &track.title
    };
    Ok(Plan {
        beats: if options.skip_beatgrids {
            None
        } else {
            beats::plan_beats(track).with_context(|| format!("Beat grid of '{name}'"))?
        },
        key: keys::mixxx_key(&track.key).with_context(|| format!("Key of '{name}'"))?,
        cues: cues::plan_cues(track, warnings),
    })
}

fn key_columns(key: &MixxxKey) -> Vec<(&'static str, Value)> {
    vec![
        ("key", Value::Text(key.text.into())),
        ("key_id", Value::Integer(i64::from(key.id))),
        ("keys_version", Value::Text("KeyMap-1.0".into())),
        ("keys_sub_version", Value::Text(String::new())),
        // Source 2 is "user": externally supplied, so Mixxx treats it with care.
        ("keys", Value::Blob(proto::key_map(key.id, key.text, 2))),
    ]
}
fn beat_columns(plan: &BeatPlan) -> Vec<(&'static str, Value)> {
    vec![
        ("bpm", Value::Real(plan.bpm)),
        ("beats_version", Value::Text(plan.version.into())),
        ("beats_sub_version", Value::Text(String::new())),
        ("beats", Value::Blob(plan.blob.clone())),
    ]
}

/// Delete a track's hot cue and loop rows and insert `rows`; main cue, intro/outro
/// and other cue types are not touched. Returns the number of rows written.
pub(super) fn write_cues(tx: &Transaction<'_>, track_id: i64, rows: &[CueRow]) -> Result<usize> {
    tx.execute(
        "DELETE FROM cues WHERE track_id = ?1 AND type IN (?2, ?3)",
        params![track_id, cues::HOT_CUE, cues::LOOP],
    )?;
    let mut insert = tx.prepare(
        "INSERT INTO cues(track_id, type, position, length, hotcue, label, color)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for row in rows {
        insert.execute(params![
            track_id,
            row.kind,
            row.position,
            row.length,
            row.hotcue,
            row.label,
            row.color
        ])?;
    }
    Ok(rows.len())
}

/// SQLite hands integral REAL values back as integers; compare numbers as numbers.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(x), Value::Real(y)) | (Value::Real(y), Value::Integer(x)) => {
            *x as f64 == *y
        }
        _ => a == b,
    }
}

/// Keep only the columns whose value would actually change, so a repeat sync writes
/// nothing.
fn changed_columns(
    tx: &Transaction<'_>,
    id: i64,
    wanted: Vec<(&'static str, Value)>,
) -> Result<Vec<(&'static str, Value)>> {
    if wanted.is_empty() {
        return Ok(wanted);
    }
    let names: Vec<&str> = wanted.iter().map(|(name, _)| *name).collect();
    let current: Vec<Value> = tx.query_row(
        &format!("SELECT {} FROM library WHERE id = ?1", names.join(", ")),
        [id],
        |row| (0..names.len()).map(|i| row.get::<_, Value>(i)).collect(),
    )?;
    Ok(wanted
        .into_iter()
        .zip(current)
        .filter(|((_, new), old)| !same(new, old))
        .map(|(column, _)| column)
        .collect())
}

/// Whether the track already holds exactly these hot cue and loop rows.
fn cues_match(tx: &Transaction<'_>, track_id: i64, rows: &[CueRow]) -> Result<bool> {
    let mut statement = tx.prepare(
        "SELECT type, position, length, hotcue, label, color FROM cues
         WHERE track_id = ?1 AND type IN (?2, ?3) ORDER BY hotcue, type",
    )?;
    let current = statement
        .query_map(params![track_id, cues::HOT_CUE, cues::LOOP], |row| {
            Ok(CueRow {
                kind: row.get(0)?,
                position: row.get(1)?,
                length: row.get(2)?,
                hotcue: row.get(3)?,
                label: row.get(4)?,
                color: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut wanted: Vec<&CueRow> = rows.iter().collect();
    wanted.sort_by_key(|row| (row.hotcue, row.kind));
    Ok(current.len() == wanted.len() && current.iter().zip(wanted).all(|(a, b)| a == b))
}

fn update(tx: &Transaction<'_>, id: i64, columns: &[(&'static str, Value)]) -> Result<()> {
    let assignments: Vec<String> = columns
        .iter()
        .enumerate()
        .map(|(index, (name, _))| format!("{name} = ?{}", index + 2))
        .collect();
    let values = std::iter::once(Value::Integer(id)).chain(columns.iter().map(|(_, v)| v.clone()));
    tx.execute(
        &format!(
            "UPDATE library SET {} WHERE id = ?1",
            assignments.join(", ")
        ),
        params_from_iter(values),
    )?;
    Ok(())
}

/// Replace the selected groups of an existing row. Values CueRithm does not have
/// (empty text, no key, no grid, rating 0) never blank what Mixxx holds.
fn replace(
    tx: &Transaction<'_>,
    library_id: i64,
    track: &SyncTrack,
    options: &SyncOptions,
    plan: &Plan,
    outcome: &mut TrackOutcome,
) -> Result<()> {
    let mixxx = &options.mixxx;
    let mut columns: Vec<(&'static str, Value)> = Vec::new();
    if mixxx.replace_metadata {
        for (column, text) in [
            ("title", &track.title),
            ("artist", &track.artist),
            ("album", &track.album),
            ("genre", &track.genre),
            ("comment", &track.comment),
        ] {
            if !text.is_empty() {
                columns.push((column, Value::Text(text.clone())));
            }
        }
        if let Some(year) = track.year {
            columns.push(("year", Value::Text(year.to_string())));
        }
        if track.rating > 0 {
            columns.push(("rating", Value::Integer(i64::from(track.rating.min(5)))));
        }
        if let Some(color) = track.color {
            columns.push(("color", Value::Integer(i64::from(color & 0x00FF_FFFF))));
        }
    }
    if mixxx.replace_key
        && let Some(key) = &plan.key
    {
        columns.extend(key_columns(key));
        outcome.keys.insert(library_id);
    }
    if let Some(beats) = &plan.beats {
        columns.extend(beat_columns(beats));
        outcome.grids.insert(library_id);
        match options.beatgrid_lock {
            BeatgridLock::Lock => columns.push(("bpm_lock", Value::Integer(1))),
            BeatgridLock::Unlock => columns.push(("bpm_lock", Value::Integer(0))),
            BeatgridLock::Default => {}
        }
    }
    let columns = changed_columns(tx, library_id, columns)?;
    let replace_cues =
        mixxx.replace_cues && !plan.cues.is_empty() && !cues_match(tx, library_id, &plan.cues)?;
    if columns.is_empty() && !replace_cues {
        outcome.kept += 1;
        return Ok(());
    }
    let mut columns = columns;
    if !columns.is_empty() {
        // Without this Mixxx would read the file's tags on first load and overwrite
        // what was just written (it does so only for rows never parsed).
        columns.push(("header_parsed", Value::Integer(1)));
        update(tx, library_id, &columns)?;
    }
    if replace_cues {
        outcome.replaced_cues += write_cues(tx, library_id, &plan.cues)?;
    }
    outcome.replaced += 1;
    Ok(())
}

/// Make a removed or missing-file track visible again. Returns whether anything changed.
fn restore(tx: &Transaction<'_>, found: Found) -> Result<bool> {
    let mut changed = 0;
    if let Some(library_id) = found.library_id {
        changed += tx.execute(
            "UPDATE library SET mixxx_deleted = 0 WHERE id = ?1 AND COALESCE(mixxx_deleted, 0) != 0",
            [library_id],
        )?;
    }
    changed += tx.execute(
        "UPDATE track_locations SET fs_deleted = 0 WHERE id = ?1 AND COALESCE(fs_deleted, 0) != 0",
        [found.location_id],
    )?;
    Ok(changed > 0)
}

/// Everything needed to add one track.
struct NewTrack<'a> {
    layout: &'a Layout,
    key: &'a str,
    /// A `track_locations` row left without a library row, to reuse.
    existing_location: Option<i64>,
    track: &'a SyncTrack,
    options: &'a SyncOptions,
    plan: &'a Plan,
}

fn insert(
    tx: &Transaction<'_>,
    new: &NewTrack<'_>,
    warnings: &mut Vec<String>,
) -> Result<(i64, i64)> {
    let NewTrack {
        layout,
        key,
        existing_location,
        track,
        options,
        plan,
    } = *new;
    let location_id = match existing_location {
        Some(id) => id,
        None => {
            let (directory, name) = split_location(key);
            let size = std::fs::metadata(&track.path)
                .with_context(|| format!("Cannot read {}", track.path.display()))?
                .len();
            tx.execute(
                "INSERT INTO track_locations(location, filename, directory, filesize, fs_deleted, needs_verification)
                 VALUES(?1, ?2, ?3, ?4, 0, 0)",
                params![key, name, directory, size as i64],
            )?;
            tx.last_insert_rowid()
        }
    };
    let (channels, bitrate) = probe_audio(&track.path).unwrap_or_else(|| {
        warnings.push(format!(
            "Audio properties of '{}' could not be read; Mixxx will read them when it first plays the track",
            if track.title.is_empty() { &track.id } else { &track.title }
        ));
        (2, 0)
    });
    let text = |value: &str| Value::Text(value.to_owned());
    let mut columns: Vec<(&'static str, Value)> = vec![
        ("artist", text(&track.artist)),
        ("title", text(&track.title)),
        ("album", text(&track.album)),
        ("album_artist", text("")),
        (
            "year",
            text(&track.year.map(|y| y.to_string()).unwrap_or_default()),
        ),
        ("genre", text(&track.genre)),
        ("composer", text("")),
        ("grouping", text("")),
        ("tracknumber", text("")),
        ("filetype", Value::Text(filetype(&track.path))),
        ("location", Value::Integer(location_id)),
        ("comment", text(&track.comment)),
        ("url", text("")),
        ("rating", Value::Integer(i64::from(track.rating.min(5)))),
        ("cuepoint", Value::Real(-1.0)),
        ("replaygain", Value::Real(0.0)),
        ("replaygain_peak", Value::Real(-1.0)),
        ("timesplayed", Value::Integer(0)),
        ("played", Value::Integer(0)),
        ("mixxx_deleted", Value::Integer(0)),
        // Parsed, never synchronized: Mixxx will not re-read tags over these values.
        ("header_parsed", Value::Integer(1)),
        ("channels", Value::Integer(i64::from(channels))),
        ("samplerate", Value::Integer(i64::from(track.sample_rate))),
        ("bitrate", Value::Integer(i64::from(bitrate))),
        ("duration", Value::Real(track.duration_seconds)),
        ("coverart_source", Value::Integer(0)),
        ("coverart_type", Value::Integer(0)),
        ("coverart_location", text("")),
        ("coverart_hash", Value::Integer(0)),
        (
            "bpm_lock",
            Value::Integer(i64::from(
                plan.beats.is_some() && matches!(options.beatgrid_lock, BeatgridLock::Lock),
            )),
        ),
    ];
    if let Some(color) = track.color {
        columns.push(("color", Value::Integer(i64::from(color & 0x00FF_FFFF))));
    }
    match &plan.key {
        Some(key) => columns.extend(key_columns(key)),
        None => columns.push(("key", text(""))),
    }
    match &plan.beats {
        Some(beats) => columns.extend(beat_columns(beats)),
        None => columns.push((
            "bpm",
            Value::Real(
                if track.bpm.is_finite() && track.bpm > 0.0 && options.skip_beatgrids {
                    track.bpm
                } else {
                    0.0
                },
            ),
        )),
    }
    for (name, _) in &columns {
        ensure!(
            layout.has_library_column(name),
            "Mixxx library has no library.{name} column"
        );
    }
    let names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
    let marks: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
    tx.execute(
        &format!(
            "INSERT INTO library({}) VALUES({})",
            names.join(", "),
            marks.join(", ")
        ),
        params_from_iter(columns.iter().map(|(_, value)| value.clone())),
    )?;
    Ok((tx.last_insert_rowid(), location_id))
}

/// Resolve every CueRithm track to a Mixxx row, adding or updating it by mode.
pub(super) fn write_tracks(
    tx: &Transaction<'_>,
    layout: &Layout,
    library: &SyncLibrary,
    options: &SyncOptions,
    case_insensitive: bool,
    warnings: &mut Vec<String>,
) -> Result<TrackOutcome> {
    let mut index = Index::load(tx, case_insensitive)?;
    let mut outcome = TrackOutcome::default();
    for track in &library.tracks {
        let canonical = std::fs::canonicalize(&track.path)
            .with_context(|| format!("Cannot resolve {}", track.path.display()))?;
        let key = location_key(&canonical)?;
        let found = index.find(&key)?;
        if let Some(Found {
            location_id,
            library_id: Some(library_id),
        }) = found
        {
            if restore(
                tx,
                Found {
                    location_id,
                    library_id: Some(library_id),
                },
            )? {
                outcome.restored += 1;
            }
            match options.mixxx.existing_tracks {
                MixxxExistingTracks::Keep => outcome.kept += 1,
                MixxxExistingTracks::Replace => {
                    let plan = plan(track, options, warnings)?;
                    replace(tx, library_id, track, options, &plan, &mut outcome)?;
                }
            }
            outcome.library_ids.insert(track.id.clone(), library_id);
            continue;
        }
        // Unknown file, or a location row left without a library row.
        let existing = found.map(|found| found.location_id);
        if let Some(location_id) = existing {
            tx.execute(
                "UPDATE track_locations SET fs_deleted = 0 WHERE id = ?1",
                [location_id],
            )?;
        }
        let plan = plan(track, options, warnings)?;
        let (library_id, location_id) = insert(
            tx,
            &NewTrack {
                layout,
                key: &key,
                existing_location: existing,
                track,
                options,
                plan: &plan,
            },
            warnings,
        )?;
        write_cues(tx, library_id, &plan.cues)?;
        index.insert(
            key,
            Found {
                location_id,
                library_id: Some(library_id),
            },
        );
        outcome.library_ids.insert(track.id.clone(), library_id);
        outcome.created.insert(library_id);
        if plan.key.is_some() {
            outcome.keys.insert(library_id);
        }
        if plan.beats.is_some() {
            outcome.grids.insert(library_id);
        }
        outcome.added += 1;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MixxxOptions, SyncBeat, SyncCue, mixxx::fixture};
    use rusqlite::Connection;
    use std::path::PathBuf;

    struct Setup {
        directory: tempfile::TempDir,
        connection: Connection,
    }
    fn setup(version: u32) -> Setup {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("mixxxdb.sqlite"), version);
        Setup {
            directory,
            connection,
        }
    }
    impl Setup {
        /// A music file on disk and its canonical Mixxx location text.
        fn file(&self, name: &str) -> (PathBuf, String) {
            let path = self.directory.path().join(name);
            std::fs::write(&path, b"not really audio").unwrap();
            let key = location_key(&std::fs::canonicalize(&path).unwrap()).unwrap();
            (path, key)
        }
        fn run(
            &mut self,
            tracks: Vec<SyncTrack>,
            options: &SyncOptions,
            case_insensitive: bool,
        ) -> Result<(TrackOutcome, Vec<String>)> {
            let layout = crate::mixxx::schema::inspect(&self.connection)?;
            let library = SyncLibrary {
                tracks,
                playlists: vec![],
            };
            let mut warnings = Vec::new();
            let tx = self.connection.transaction()?;
            let outcome = write_tracks(
                &tx,
                &layout,
                &library,
                options,
                case_insensitive,
                &mut warnings,
            )?;
            tx.commit()?;
            Ok((outcome, warnings))
        }
    }
    fn options(mode: MixxxExistingTracks) -> SyncOptions {
        SyncOptions {
            mixxx: MixxxOptions {
                existing_tracks: mode,
                ..MixxxOptions::default()
            },
            ..SyncOptions::default()
        }
    }
    fn full_track(id: &str, path: &Path) -> SyncTrack {
        SyncTrack {
            id: id.into(),
            path: path.to_path_buf(),
            title: format!("CueRithm {id}"),
            artist: "CueRithm Artist".into(),
            album: "CueRithm Album".into(),
            genre: "House".into(),
            comment: "note".into(),
            key: "8A".into(),
            bpm: 128.0,
            duration_seconds: 240.0,
            sample_rate: 44_100,
            rating: 4,
            year: Some(2024),
            color: Some(0x112233),
            beatgrid: vec![SyncBeat {
                seconds: 0.25,
                bpm: 128.0,
                beat: 1,
            }],
            cues: vec![
                SyncCue {
                    index: 0,
                    name: "Drop".into(),
                    start_seconds: 30.0,
                    color: Some(0xFF0000),
                    ..SyncCue::default()
                },
                SyncCue {
                    index: 1,
                    name: "Loop".into(),
                    start_seconds: 60.0,
                    end_seconds: Some(64.0),
                    ..SyncCue::default()
                },
            ],
            ..SyncTrack::default()
        }
    }
    fn library_row(connection: &Connection, id: i64) -> Vec<Value> {
        fixture::dump(
            connection,
            &format!("SELECT * FROM library WHERE id = {id}"),
        )
        .remove(0)
    }
    fn value(connection: &Connection, id: i64, column: &str) -> Value {
        connection
            .query_row(
                &format!("SELECT {column} FROM library WHERE id = ?1"),
                [id],
                |r| r.get(0),
            )
            .unwrap()
    }

    #[test]
    fn location_key_normalizes_windows_and_unc_spellings() {
        for (input, expected) in [
            (r"C:\Users\dj\Music\a.mp3", "C:/Users/dj/Music/a.mp3"),
            (r"\\?\C:\Users\dj\Music\a.mp3", "C:/Users/dj/Music/a.mp3"),
            (r"\\?\UNC\nas\music\a.mp3", "//nas/music/a.mp3"),
            (r"\\nas\music\a.mp3", "//nas/music/a.mp3"),
            ("D:/Music/a.mp3", "D:/Music/a.mp3"),
            ("/home/dj/Music/a.mp3", "/home/dj/Music/a.mp3"),
            (r"/home/dj/odd\name.mp3", r"/home/dj/odd\name.mp3"),
        ] {
            assert_eq!(location_key(Path::new(input)).unwrap(), expected, "{input}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let bad = PathBuf::from(std::ffi::OsString::from_vec(b"/music/\xff.mp3".to_vec()));
            assert!(
                location_key(&bad)
                    .unwrap_err()
                    .to_string()
                    .contains("Unicode")
            );
        }
    }

    #[test]
    fn split_location_handles_roots() {
        assert_eq!(split_location("/a.mp3"), ("/".into(), "a.mp3".into()));
        assert_eq!(split_location("C:/a.mp3"), ("C:/".into(), "a.mp3".into()));
        assert_eq!(
            split_location("/m/n/a.mp3"),
            ("/m/n".into(), "a.mp3".into())
        );
    }

    #[test]
    fn exact_match_updates_the_existing_row_only() {
        let mut s = setup(39);
        let (path_a, key_a) = s.file("a.mp3");
        let (_, key_b) = s.file("b.mp3");
        let a = fixture::add_track(&s.connection, &key_a, "Mixxx A");
        let b = fixture::add_track(&s.connection, &key_b, "Mixxx B");
        let before_b = library_row(&s.connection, b);
        let (outcome, _) = s
            .run(
                vec![full_track("t", &path_a)],
                &options(MixxxExistingTracks::Replace),
                false,
            )
            .unwrap();
        assert_eq!((outcome.added, outcome.replaced), (0, 1));
        assert_eq!(outcome.library_ids["t"], a);
        assert_eq!(
            value(&s.connection, a, "title"),
            Value::Text("CueRithm t".into())
        );
        assert_eq!(
            library_row(&s.connection, b),
            before_b,
            "the other row is untouched"
        );
        assert_eq!(
            fixture::dump(&s.connection, "SELECT id FROM library").len(),
            2
        );
    }

    #[test]
    fn case_insensitive_match_on_macos_and_windows_but_exact_on_linux() {
        let _same = pretend_same_file(|_, _| true);
        for (insensitive, expected_rows) in [(true, 1), (false, 2)] {
            let mut s = setup(39);
            let (path, key) = s.file("Song.mp3");
            fixture::add_track(&s.connection, &key.to_uppercase(), "Existing");
            let (outcome, _) = s
                .run(
                    vec![full_track("t", &path)],
                    &options(MixxxExistingTracks::Keep),
                    insensitive,
                )
                .unwrap();
            let rows = fixture::dump(&s.connection, "SELECT id FROM library").len();
            assert_eq!(rows, expected_rows, "case_insensitive={insensitive}");
            assert_eq!(outcome.added, expected_rows - 1);
        }
    }

    #[test]
    fn a_case_variant_on_a_case_sensitive_volume_is_a_different_track() {
        // `Song.mp3` is in the library; `song.mp3` is another file that merely folds to the
        // same text. The file system says they are not one file, so the second is added
        // rather than silently merged into the first's row.
        let _same = pretend_same_file(|_, _| false);
        let mut s = setup(39);
        let (path, key) = s.file("Song.mp3");
        fixture::add_track(&s.connection, &key.to_uppercase(), "Existing");
        let (outcome, _) = s
            .run(
                vec![full_track("t", &path)],
                &options(MixxxExistingTracks::Keep),
                true,
            )
            .unwrap();
        assert_eq!(outcome.added, 1);
        assert_eq!(
            fixture::dump(&s.connection, "SELECT id FROM library").len(),
            2
        );
    }

    #[test]
    fn a_case_variant_of_the_same_file_is_matched_by_its_identity_not_its_spelling() {
        // The real check, on whatever file system the tests run on: a path that is the file
        // itself is the same file; a name that does not exist is not.
        let s = setup(39);
        let (_, key) = s.file("Song.mp3");
        assert!(same_file(&key, &key));
        assert!(!same_file(&key, &key.replace("Song", "Other")));
        assert!(!same_file("/no/such/file.mp3", &key));
    }

    #[test]
    fn ambiguous_case_match_is_error_but_an_exact_row_wins() {
        let _same = pretend_same_file(|_, _| true);
        let mut s = setup(39);
        let (path, key) = s.file("Song.mp3");
        fixture::add_track(&s.connection, &key.to_uppercase(), "Upper");
        fixture::add_track(&s.connection, &key.replace("Song", "SONG"), "Mixed");
        let error = s
            .run(
                vec![full_track("t", &path)],
                &options(MixxxExistingTracks::Keep),
                true,
            )
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("2 rows") && error.contains("duplicates"),
            "{error}"
        );
        let exact = fixture::add_track(&s.connection, &key, "Exact");
        let (outcome, _) = s
            .run(
                vec![full_track("t", &path)],
                &options(MixxxExistingTracks::Keep),
                true,
            )
            .unwrap();
        assert_eq!(outcome.library_ids["t"], exact);
    }

    #[test]
    fn keep_mode_leaves_existing_tracks_byte_for_byte_but_restores_deleted_and_missing_flags() {
        let mut s = setup(40);
        let (path, key) = s.file("a.mp3");
        let id = fixture::add_track(&s.connection, &key, "Mixxx A");
        s.connection
            .execute("UPDATE library SET mixxx_deleted = 1", [])
            .unwrap();
        s.connection
            .execute("UPDATE track_locations SET fs_deleted = 1", [])
            .unwrap();
        let mut before = library_row(&s.connection, id);
        let (outcome, _) = s
            .run(
                vec![full_track("t", &path)],
                &options(MixxxExistingTracks::Keep),
                false,
            )
            .unwrap();
        assert_eq!(
            (
                outcome.kept,
                outcome.restored,
                outcome.replaced,
                outcome.added
            ),
            (1, 1, 0, 0)
        );
        let deleted = fixture::column_index(&s.connection, "library", "mixxx_deleted");
        before[deleted] = Value::Integer(0);
        assert_eq!(library_row(&s.connection, id), before);
        let missing: i64 = s
            .connection
            .query_row("SELECT fs_deleted FROM track_locations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(missing, 0);
        // A second run changes nothing and restores nothing.
        let (again, _) = s
            .run(
                vec![full_track("t", &path)],
                &options(MixxxExistingTracks::Keep),
                false,
            )
            .unwrap();
        assert_eq!((again.kept, again.restored), (1, 0));
    }

    #[test]
    fn replace_mode_updates_selected_groups_only() {
        type Restrict = fn(&mut MixxxOptions);
        type Toggle = (&'static str, Restrict, &'static [&'static str]);
        let toggles: [Toggle; 3] = [
            (
                "metadata",
                |o| {
                    o.replace_key = false;
                    o.replace_cues = false;
                },
                &[
                    "title", "artist", "album", "genre", "comment", "year", "rating", "color",
                ],
            ),
            (
                "key",
                |o| {
                    o.replace_metadata = false;
                    o.replace_cues = false;
                },
                &["key", "key_id", "keys_sub_version", "keys_version", "keys"],
            ),
            (
                "cues",
                |o| {
                    o.replace_metadata = false;
                    o.replace_key = false;
                },
                &[],
            ),
        ];
        for (name, restrict, changed) in toggles {
            let mut s = setup(39);
            let (path, key) = s.file("a.mp3");
            let id = fixture::add_track(&s.connection, &key, "Mixxx A");
            let before = library_row(&s.connection, id);
            let mut opts = options(MixxxExistingTracks::Replace);
            opts.skip_beatgrids = true; // isolate the three toggles from the grid
            restrict(&mut opts.mixxx);
            s.run(vec![full_track("t", &path)], &opts, false).unwrap();
            let after = library_row(&s.connection, id);
            let names = fixture::column_names(&s.connection, "library");
            let differing: Vec<&str> = names
                .iter()
                .enumerate()
                .filter(|(i, n)| before[*i] != after[*i] && **n != "header_parsed")
                .map(|(_, n)| n.as_str())
                .collect();
            let mut expected: Vec<&str> = changed.to_vec();
            expected.sort_unstable();
            let mut got = differing.clone();
            got.sort_unstable();
            // `title`, `rating` and friends all differ from the fixture row.
            assert_eq!(got, expected, "toggle {name}");
        }
        // The grid follows skip_beatgrids, independent of the three toggles.
        let mut s = setup(39);
        let (path, key) = s.file("a.mp3");
        let id = fixture::add_track(&s.connection, &key, "Mixxx A");
        let mut opts = options(MixxxExistingTracks::Replace);
        opts.mixxx.replace_metadata = false;
        opts.mixxx.replace_key = false;
        opts.mixxx.replace_cues = false;
        s.run(vec![full_track("t", &path)], &opts, false).unwrap();
        assert_eq!(
            value(&s.connection, id, "beats_version"),
            Value::Text("BeatGrid-2.0".into())
        );
        assert_eq!(value(&s.connection, id, "bpm"), Value::Real(128.0));
        assert_eq!(
            value(&s.connection, id, "title"),
            Value::Text("Mixxx A".into())
        );
    }

    #[test]
    fn empty_key_and_empty_grid_never_clear_existing_values() {
        let mut s = setup(39);
        let (path, key) = s.file("a.mp3");
        let id = fixture::add_track(&s.connection, &key, "Mixxx A");
        s.connection
            .execute(
                "UPDATE library SET key='Am', key_id=22, keys_version='KeyMap-1.0', keys=x'0816',
                 beats_version='BeatGrid-2.0', beats=x'0a', bpm=120.0, comment='keep me', rating=5",
                [],
            )
            .unwrap();
        let before = library_row(&s.connection, id);
        let bare = SyncTrack {
            id: "t".into(),
            path,
            sample_rate: 44_100,
            ..SyncTrack::default()
        };
        let (outcome, _) = s
            .run(vec![bare], &options(MixxxExistingTracks::Replace), false)
            .unwrap();
        assert_eq!((outcome.replaced, outcome.kept), (0, 1));
        assert_eq!(library_row(&s.connection, id), before);
    }

    #[test]
    fn lock_option_sets_bpm_lock() {
        for (lock, expected) in [
            (BeatgridLock::Lock, 1),
            (BeatgridLock::Unlock, 0),
            (BeatgridLock::Default, 1),
        ] {
            let mut s = setup(39);
            let (path, key) = s.file("a.mp3");
            let id = fixture::add_track(&s.connection, &key, "A");
            s.connection
                .execute("UPDATE library SET bpm_lock = 1", [])
                .unwrap();
            let mut opts = options(MixxxExistingTracks::Replace);
            opts.beatgrid_lock = lock;
            s.run(vec![full_track("t", &path)], &opts, false).unwrap();
            // Default keeps the existing 1; Lock sets 1; Unlock sets 0.
            assert_eq!(
                value(&s.connection, id, "bpm_lock"),
                Value::Integer(expected),
                "{lock:?}"
            );
        }
        // New rows: Lock locks, everything else leaves the grid unlocked.
        for (lock, expected) in [
            (BeatgridLock::Lock, 1),
            (BeatgridLock::Unlock, 0),
            (BeatgridLock::Default, 0),
        ] {
            let mut s = setup(39);
            let (path, _) = s.file("n.mp3");
            let mut opts = options(MixxxExistingTracks::Keep);
            opts.beatgrid_lock = lock;
            let (outcome, _) = s.run(vec![full_track("t", &path)], &opts, false).unwrap();
            assert_eq!(
                value(&s.connection, outcome.library_ids["t"], "bpm_lock"),
                Value::Integer(expected)
            );
        }
    }

    #[test]
    fn new_rows_without_written_grids_stay_unlocked() {
        for version in [39, 40] {
            for skip_beatgrids in [false, true] {
                let mut s = setup(version);
                let (path, _) = s.file("new.mp3");
                let mut track = full_track("t", &path);
                if !skip_beatgrids {
                    track.beatgrid.clear();
                }
                let mut opts = options(MixxxExistingTracks::Keep);
                opts.beatgrid_lock = BeatgridLock::Lock;
                opts.skip_beatgrids = skip_beatgrids;
                let (outcome, _) = s.run(vec![track], &opts, false).unwrap();
                let id = outcome.library_ids["t"];
                assert_eq!(value(&s.connection, id, "beats"), Value::Null);
                assert_eq!(
                    value(&s.connection, id, "bpm_lock"),
                    Value::Integer(0),
                    "schema {version}, skip_beatgrids={skip_beatgrids}"
                );
            }
        }
    }

    #[test]
    fn new_row_has_header_parsed_and_null_source_sync_and_correct_blobs() {
        for version in [39, 40] {
            let mut s = setup(version);
            let (path, key) = s.file("New Song.mp3");
            let (outcome, warnings) = s
                .run(
                    vec![full_track("t", &path)],
                    &options(MixxxExistingTracks::Keep),
                    false,
                )
                .unwrap();
            assert_eq!(outcome.added, 1);
            let id = outcome.library_ids["t"];
            let get = |column: &str| value(&s.connection, id, column);
            assert_eq!(get("header_parsed"), Value::Integer(1));
            assert_eq!(get("source_synchronized_ms"), Value::Null);
            assert_eq!(get("mixxx_deleted"), Value::Integer(0));
            assert_eq!(get("filetype"), Value::Text("mp3".into()));
            assert_eq!(get("title"), Value::Text("CueRithm t".into()));
            assert_eq!(get("year"), Value::Text("2024".into()));
            assert_eq!(get("rating"), Value::Integer(4));
            assert_eq!(get("color"), Value::Integer(0x112233));
            // INTEGER affinity (as in Mixxx's own table) stores -1.0 as the integer -1.
            assert_eq!(get("cuepoint"), Value::Integer(-1));
            assert_eq!(get("samplerate"), Value::Integer(44_100));
            assert_eq!(get("duration"), Value::Real(240.0));
            assert_eq!(get("key"), Value::Text("Am".into()));
            assert_eq!(get("key_id"), Value::Integer(22));
            assert_eq!(get("keys_version"), Value::Text("KeyMap-1.0".into()));
            assert_eq!(get("beats_version"), Value::Text("BeatGrid-2.0".into()));
            assert_eq!(get("bpm"), Value::Real(128.0));
            assert_eq!(get("bpm_lock"), Value::Integer(0));
            let Value::Blob(beats) = get("beats") else {
                panic!("beats blob")
            };
            assert_eq!(proto::decode::beat_grid(&beats).unwrap(), (128.0, 11_025));
            let Value::Blob(keys) = get("keys") else {
                panic!("keys blob")
            };
            assert_eq!(proto::decode::key_map(&keys).unwrap(), (22, "Am".into(), 2));
            if version == 40 {
                assert_eq!(get("tuning_frequency_hz"), Value::Real(0.0));
            }
            let (directory, filename, size, deleted): (String, String, i64, i64) = s
                .connection
                .query_row(
                    "SELECT directory, filename, filesize, fs_deleted FROM track_locations WHERE location = ?1",
                    [&key],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .unwrap();
            assert_eq!(filename, "New Song.mp3");
            assert!(key.starts_with(&directory) && key.ends_with("New Song.mp3"));
            assert_eq!((size, deleted), (16, 0));
            assert!(
                warnings.iter().any(|w| w.contains("could not be read")),
                "an unreadable audio file falls back with a warning: {warnings:?}"
            );
        }
    }

    #[test]
    fn new_row_without_key_or_grid_leaves_them_empty_and_aif_becomes_aiff() {
        let mut s = setup(39);
        let (path, _) = s.file("a.AIF");
        let bare = SyncTrack {
            id: "t".into(),
            path,
            title: "Bare".into(),
            bpm: 126.0,
            sample_rate: 48_000,
            duration_seconds: 10.0,
            ..SyncTrack::default()
        };
        let (outcome, _) = s
            .run(vec![bare], &options(MixxxExistingTracks::Keep), false)
            .unwrap();
        let id = outcome.library_ids["t"];
        assert_eq!(
            value(&s.connection, id, "filetype"),
            Value::Text("aiff".into())
        );
        assert_eq!(value(&s.connection, id, "beats_version"), Value::Null);
        assert_eq!(value(&s.connection, id, "keys_version"), Value::Null);
        assert_eq!(value(&s.connection, id, "key"), Value::Text(String::new()));
        assert_eq!(value(&s.connection, id, "bpm"), Value::Real(0.0));
        // With the grid skipped on purpose, the track's own BPM is still recorded.
        let mut s = setup(39);
        let (path, _) = s.file("b.mp3");
        let mut opts = options(MixxxExistingTracks::Keep);
        opts.skip_beatgrids = true;
        let (outcome, _) = s.run(vec![full_track("t", &path)], &opts, false).unwrap();
        let id = outcome.library_ids["t"];
        assert_eq!(value(&s.connection, id, "beats_version"), Value::Null);
        assert_eq!(value(&s.connection, id, "bpm"), Value::Real(128.0));
    }

    #[test]
    fn readable_audio_supplies_channels() {
        let mut s = setup(39);
        let path = s.directory.path().join("mono.wav");
        fixture::write_wav(&path, 1, 22_050, 2_205);
        let mut track = full_track("t", &path);
        track.sample_rate = 22_050;
        let (outcome, warnings) = s
            .run(vec![track], &options(MixxxExistingTracks::Keep), false)
            .unwrap();
        let id = outcome.library_ids["t"];
        assert_eq!(value(&s.connection, id, "channels"), Value::Integer(1));
        assert_eq!(
            value(&s.connection, id, "filetype"),
            Value::Text("wav".into())
        );
        assert!(
            warnings.iter().all(|w| !w.contains("could not be read")),
            "{warnings:?}"
        );
    }

    #[test]
    fn orphan_location_row_is_reused_and_the_same_file_twice_inserts_once() {
        let mut s = setup(39);
        let (path, key) = s.file("a.mp3");
        s.connection
            .execute(
                "INSERT INTO track_locations(location, filename, directory, filesize, fs_deleted, needs_verification)
                 VALUES(?1, 'a.mp3', '/x', 1, 1, 0)",
                [&key],
            )
            .unwrap();
        let first = full_track("one", &path);
        let second = full_track("two", &path);
        let (outcome, _) = s
            .run(
                vec![first, second],
                &options(MixxxExistingTracks::Keep),
                false,
            )
            .unwrap();
        assert_eq!(outcome.added, 1);
        assert_eq!(outcome.library_ids["one"], outcome.library_ids["two"]);
        let locations = fixture::dump(&s.connection, "SELECT id FROM track_locations").len();
        assert_eq!(locations, 1, "the existing location row is reused");
        let missing: i64 = s
            .connection
            .query_row("SELECT fs_deleted FROM track_locations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(missing, 0);
    }

    fn cue_rows(connection: &Connection, id: i64) -> Vec<(i64, f64, f64, i64, String, i64)> {
        let mut statement = connection
            .prepare("SELECT type, position, length, hotcue, label, color FROM cues WHERE track_id = ?1 ORDER BY type, hotcue")
            .unwrap();
        statement
            .query_map([id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn new_track_gets_cues() {
        let mut s = setup(39);
        let (path, _) = s.file("a.mp3");
        let (outcome, _) = s
            .run(
                vec![full_track("t", &path)],
                &options(MixxxExistingTracks::Keep),
                false,
            )
            .unwrap();
        let rows = cue_rows(&s.connection, outcome.library_ids["t"]);
        assert_eq!(
            rows,
            vec![
                (1, 2_646_000.0, 0.0, 0, "Drop".into(), 0xFF0000),
                (4, 5_292_000.0, 352_800.0, 1, "Loop".into(), 0x32BE44),
            ]
        );
    }

    #[test]
    fn replace_swaps_hot_cues_and_loops_but_keeps_main_intro_and_outro_rows() {
        let mut s = setup(39);
        let (path, key) = s.file("a.mp3");
        let id = fixture::add_track(&s.connection, &key, "A");
        for (kind, hotcue) in [(1, 0), (1, 7), (4, 2), (2, -1), (6, -1), (7, -1), (8, -1)] {
            s.connection
                .execute(
                    "INSERT INTO cues(track_id, type, position, length, hotcue, label, color)
                     VALUES(?1, ?2, 1000, 0, ?3, 'old', 255)",
                    params![id, kind, hotcue],
                )
                .unwrap();
        }
        let (outcome, _) = s
            .run(
                vec![full_track("t", &path)],
                &options(MixxxExistingTracks::Replace),
                false,
            )
            .unwrap();
        assert_eq!(outcome.replaced_cues, 2);
        let rows = cue_rows(&s.connection, id);
        let kinds: Vec<i64> = rows.iter().map(|r| r.0).collect();
        assert_eq!(
            kinds,
            vec![1, 2, 4, 6, 7, 8],
            "one new hot cue, one new loop, others kept"
        );
        assert!(
            rows.iter()
                .filter(|r| r.0 == 1 || r.0 == 4)
                .all(|r| r.4 != "old")
        );
        assert!(
            rows.iter()
                .filter(|r| [2, 6, 7, 8].contains(&r.0))
                .all(|r| r.4 == "old")
        );
    }

    #[test]
    fn no_cuerithm_cues_leaves_existing_cues_and_keep_mode_never_touches_them() {
        let mut s = setup(39);
        let (path, key) = s.file("a.mp3");
        let id = fixture::add_track(&s.connection, &key, "A");
        s.connection
            .execute("INSERT INTO cues(track_id, type, position, length, hotcue, label, color) VALUES(?1, 1, 5, 0, 0, 'mine', 1)", [id])
            .unwrap();
        let mut bare = full_track("t", &path);
        bare.cues.clear();
        s.run(vec![bare], &options(MixxxExistingTracks::Replace), false)
            .unwrap();
        assert_eq!(cue_rows(&s.connection, id).len(), 1);
        s.run(
            vec![full_track("t", &path)],
            &options(MixxxExistingTracks::Keep),
            false,
        )
        .unwrap();
        assert_eq!(
            cue_rows(&s.connection, id),
            vec![(1, 5.0, 0.0, 0, "mine".into(), 1)]
        );
        // Replace with the cue group switched off also keeps them.
        let mut opts = options(MixxxExistingTracks::Replace);
        opts.mixxx.replace_cues = false;
        s.run(vec![full_track("t", &path)], &opts, false).unwrap();
        assert_eq!(cue_rows(&s.connection, id).len(), 1);
    }

    #[test]
    fn failing_tracks_name_the_track() {
        let mut s = setup(39);
        let (path, _) = s.file("a.mp3");
        let mut track = full_track("t", &path);
        track.key = "nonsense".into();
        let error = format!(
            "{:#}",
            s.run(vec![track], &options(MixxxExistingTracks::Keep), false)
                .unwrap_err()
        );
        assert!(
            error.contains("CueRithm t") && error.contains("nonsense"),
            "{error}"
        );
        let mut track = full_track("t", &path);
        track.beatgrid[0].bpm = 0.0;
        assert!(
            s.run(vec![track], &options(MixxxExistingTracks::Keep), false)
                .is_err()
        );
        let missing = SyncTrack {
            id: "gone".into(),
            path: s.directory.path().join("gone.mp3"),
            ..SyncTrack::default()
        };
        assert!(
            s.run(vec![missing], &options(MixxxExistingTracks::Keep), false)
                .is_err()
        );
    }
}

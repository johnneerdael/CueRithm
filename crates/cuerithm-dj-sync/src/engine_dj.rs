//! Current Engine DJ Database2 writer. The caller owns backup and application preflight.
mod owned;
pub(crate) fn reference_id(database: &Path, base: &Path, source: &Path) -> Result<String> {
    let conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let major: i64 = conn.query_row(
        "SELECT schemaVersionMajor FROM Information LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    ensure!(major == 3, "Engine DJ path repair requires schema major 3");
    let relative = relative_path(base, source)?;
    let ids = conn
        .prepare("SELECT id FROM Track WHERE path=?1")?
        .query_map([relative], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        ids.len() == 1,
        "Engine DJ source reference is missing or ambiguous"
    );
    Ok(ids[0].to_string())
}
pub(crate) fn repair_reference(
    database: &Path,
    base: &Path,
    source: &Path,
    destination: &Path,
    expected: &str,
) -> Result<()> {
    ensure!(
        reference_id(database, base, source)? == expected,
        "Engine DJ native track identity changed"
    );
    let mut conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let tx = conn.transaction()?;
    let from = relative_path(base, source)?;
    let to = relative_path(base, destination)?;
    let exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM Track WHERE path=?1 AND path!=?2",
        params![to, from],
        |row| row.get(0),
    )?;
    ensure!(
        exists == 0,
        "Engine DJ destination already has a native track"
    );
    let name = destination
        .file_name()
        .and_then(|s| s.to_str())
        .context("Engine DJ requires a Unicode filename")?;
    ensure!(
        tx.execute(
            "UPDATE Track SET path=?1,filename=?2 WHERE id=?3 AND path=?4",
            params![to, name, expected, from]
        )? == 1,
        "Engine DJ source reference changed"
    );
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
#[path = "engine_dj/owned_tests.rs"]
mod owned_tests;
use crate::{BeatgridLock, SyncLibrary, SyncOptions, SyncTrack};
use anyhow::{Context, Result, ensure};
use flate2::{Compression, write::ZlibEncoder};
pub use owned::write_staged_owned;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use sha1::{Digest, Sha1};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::{Component, Path, PathBuf},
};

/// Stage self-contained hardware media while retaining the original library as
/// the coordinate system for native relative paths. Return `(relative target,
/// staged file)` pairs for the caller's publication journal.
pub fn stage_device_media(
    stage_root: &Path,
    original_library_root: &Path,
    library: &SyncLibrary,
) -> Result<(SyncLibrary, Vec<(PathBuf, PathBuf)>)> {
    use sha2::{Digest, Sha256};
    ensure!(
        original_library_root.is_absolute(),
        "Engine DJ device library must have an absolute path"
    );
    let mut projected = library.clone();
    let mut files = Vec::new();
    let mut ids = HashSet::new();
    for track in &mut projected.tracks {
        ensure!(ids.insert(track.id.clone()), "Duplicate source track ID");
        let before =
            std::fs::metadata(&track.path).context("Engine DJ device source audio is missing")?;
        ensure!(
            before.is_file(),
            "Engine DJ device source must be a regular audio file"
        );
        let extension = track
            .path
            .extension()
            .and_then(|e| e.to_str())
            .context("Engine DJ device audio requires a file extension")?;
        ensure!(
            !extension.is_empty() && extension.chars().all(|c| c.is_ascii_alphanumeric()),
            "Invalid Engine DJ device audio extension"
        );
        let digest = format!("{:x}", Sha256::digest(track.id.as_bytes()));
        let relative =
            Path::new("Music").join(format!("{}.{}", &digest[..24], extension.to_lowercase()));
        let staged = stage_root.join(&relative);
        std::fs::create_dir_all(staged.parent().unwrap())?;
        if track.path != staged {
            std::fs::copy(&track.path, &staged)?;
        }
        let after = std::fs::metadata(&track.path)?;
        ensure!(
            before.len() == after.len()
                && before.modified()? == after.modified()?
                && std::fs::metadata(&staged)?.len() == before.len(),
            "Engine DJ device audio changed during copying"
        );
        track.path = original_library_root.join(&relative);
        files.push((relative, staged));
    }
    Ok((projected, files))
}

/// Update an existing Engine Library folder, Database2 folder, or exact `m.db`.
/// Unrelated tracks and playlists remain present. No audio files are changed.
pub fn resolve_database_path(path: &Path) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.into());
    }
    for relative in ["m.db", "Database2/m.db", "Engine Library/Database2/m.db"] {
        let database = path.join(relative);
        if database.is_file() {
            return Ok(database);
        }
    }
    anyhow::bail!("Select an initialized Engine DJ library folder, drive or m.db")
}
pub fn write(path: &Path, library: &SyncLibrary, options: &SyncOptions) -> Result<()> {
    let db = resolve_database_path(path)?;
    ensure!(
        db.file_name().is_some_and(|s| s == "m.db"),
        "Select the current Engine DJ m.db"
    );
    let base = db
        .parent()
        .and_then(Path::parent)
        .context("Engine DJ database has no library folder")?;
    write_staged(&db, base, library, options)
}

/// Write a staged database while resolving media against the original Engine Library root.
pub fn write_staged(
    database_path: &Path,
    library_root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> Result<()> {
    let mut connection =
        Connection::open_with_flags(database_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .context("Open existing Engine DJ database")?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    let (uuid, major): (String, i64) = connection.query_row(
        "SELECT uuid, schemaVersionMajor FROM Information LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        major == 3,
        "Engine DJ requires the current schema major 3; found {major}"
    );
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut ids = HashMap::new();
    for track in &library.tracks {
        ensure!(
            ids.insert(track.id.clone(), 0).is_none(),
            "Duplicate source track ID"
        );
        let id = write_track(&tx, library_root, &uuid, track, options)?;
        ids.insert(track.id.clone(), id);
    }
    write_playlists(&tx, library, &ids, &uuid, options)?;
    tx.commit()?;
    Ok(())
}

fn relative_path(base: &Path, target: &Path) -> Result<String> {
    ensure!(
        base.is_absolute() && target.is_absolute(),
        "Engine DJ requires absolute library and audio paths"
    );
    let base: Vec<_> = base.components().collect();
    let target: Vec<_> = target.components().collect();
    ensure!(
        base.first() == target.first(),
        "Audio and Engine DJ library must use the same filesystem root"
    );
    ensure!(
        !target.iter().any(|c| matches!(c, Component::ParentDir)),
        "Normalize audio paths before Engine DJ sync"
    );
    let common = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for _ in common..base.len() {
        result.push("..");
    }
    for component in &target[common..] {
        result.push(component.as_os_str());
    }
    Ok(result
        .to_str()
        .context("Engine DJ requires UTF-8 paths")?
        .replace('\\', "/"))
}

fn write_track(
    tx: &Transaction<'_>,
    base: &Path,
    uuid: &str,
    track: &SyncTrack,
    options: &SyncOptions,
) -> Result<i64> {
    ensure!(
        track.sample_rate > 0
            && track.duration_seconds.is_finite()
            && track.duration_seconds >= 0.0,
        "Engine DJ requires sample rate and duration"
    );
    ensure!(
        track.bpm.is_finite() && track.bpm >= 0.0,
        "Invalid Engine DJ BPM"
    );
    let relative = relative_path(base, &track.path)?;
    let existing: Option<i64> = tx
        .query_row("SELECT id FROM Track WHERE path=?1", [&relative], |r| {
            r.get(0)
        })
        .optional()?;
    let locked = match options.beatgrid_lock {
        BeatgridLock::Lock => true,
        BeatgridLock::Unlock => false,
        BeatgridLock::Default => {
            if let Some(id) = existing {
                tx.query_row(
                    "SELECT COALESCE(isBeatGridLocked,0) FROM Track WHERE id=?1",
                    [id],
                    |r| r.get(0),
                )?
            } else {
                !track.beatgrid.is_empty() && !options.skip_beatgrids
            }
        }
    };
    let key = key_number(&track.key);
    if existing.is_none() {
        tx.execute("INSERT INTO Track(path, filename, fileType, isAvailable, isAnalyzed, isMetadataImported,
            originDatabaseUuid, dateCreated, dateAdded, playOrder, playedIndicator,
            isMetadataOfPackedTrackChanged, isPerfomanceDataOfPackedTrackChanged, albumArt)
            VALUES(?1,?2,?3,1,0,1,?4,strftime('%s','now'),strftime('%s','now'),0,0,0,0,'image://planck/0')",
            params![relative, track.path.file_name().and_then(|s| s.to_str()).unwrap_or_default(),
                track.path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase(), uuid])?;
    }
    let id = existing.unwrap_or_else(|| tx.last_insert_rowid());
    if options.add_album_art
        && let Some(artwork) = &track.artwork
    {
        let art_id = write_artwork(tx, artwork)?;
        tx.execute(
            "UPDATE Track SET albumArtId=?1 WHERE id=?2",
            params![art_id, id],
        )?;
    }
    tx.execute("UPDATE Track SET title=?1,artist=?2,album=?3,genre=?4,label=?5,comment=?6,
        bpm=?7,bpmAnalyzed=?7,length=?8,year=?9,key=?10,rating=?11,isBeatGridLocked=?12,
        originDatabaseUuid=COALESCE(originDatabaseUuid,?13),originTrackId=COALESCE(NULLIF(originTrackId,0),id),
        lastEditTime=strftime('%s','now') WHERE id=?14",
        params![track.title,track.artist,track.album,track.genre,track.label,track.comment,
            (track.bpm > 0.0).then_some(track.bpm),track.duration_seconds.round() as i64,
            track.year,key, i64::from(track.rating.min(5))*20,locked,uuid,id])?;
    // Source dates use milliseconds; Engine stores Unix seconds. Zero is its
    // missing-date sentinel, so a source without a date never acquires sync time.
    tx.execute(
        "UPDATE Track SET dateAdded=?1,composer=?2,remixer=?3,fileType=?4 WHERE id=?5",
        params![
            track
                .attribute_date("dateAdded")
                .map(|v| v.div_euclid(1000))
                .unwrap_or(0),
            track.attribute_text("composer").unwrap_or_default(),
            track.attribute_text("remixer").unwrap_or_default(),
            track
                .path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_lowercase(),
            id
        ],
    )?;
    let data = performance_data(track, key.unwrap_or(0), options)?;
    tx.execute(
        "INSERT OR IGNORE INTO PerformanceData(trackId) VALUES(?1)",
        [id],
    )?;
    tx.execute("UPDATE PerformanceData SET trackData=?1,quickCues=?2,loops=?3,activeOnLoadLoops=0 WHERE trackId=?4",
        params![data.track,data.cues,data.loops,id])?;
    if !options.skip_beatgrids {
        tx.execute(
            "UPDATE PerformanceData SET beatData=?1 WHERE trackId=?2",
            params![data.beats, id],
        )?;
    }
    Ok(id)
}

fn write_artwork(tx: &Transaction<'_>, bytes: &[u8]) -> Result<i64> {
    ensure!(
        bytes.len() <= 10 * 1024 * 1024,
        "Engine DJ cover image exceeds 10 MB"
    );
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().context("Decode Engine DJ cover image")?;
    let thumbnail = image.resize(256, 256, image::imageops::FilterType::Lanczos3);
    let mut png = std::io::Cursor::new(Vec::new());
    thumbnail.write_to(&mut png, image::ImageFormat::Png)?;
    let png = png.into_inner();
    ensure!(
        png.len() <= 2 * 1024 * 1024,
        "Engine DJ normalized cover image exceeds 2 MB"
    );
    let hash = format!("{:x}", Sha1::digest(&png));
    if let Some(id) = tx
        .query_row("SELECT id FROM AlbumArt WHERE hash=?1", [&hash], |r| {
            r.get(0)
        })
        .optional()?
    {
        return Ok(id);
    }
    tx.execute(
        "INSERT INTO AlbumArt(hash,albumArt) VALUES(?1,?2)",
        params![hash, png],
    )?;
    Ok(tx.last_insert_rowid())
}

fn key_number(value: &str) -> Option<u32> {
    let key = value
        .trim()
        .to_lowercase()
        .replace('♯', "#")
        .replace('♭', "b");
    if let Some(last) = key.chars().last()
        && let Ok(n) = key[..key.len() - last.len_utf8()].parse::<u32>()
        && (1..=12).contains(&n)
    {
        return match last {
            'a' | 'b' => Some(((n + 4) % 12) * 2 + u32::from(last == 'a')),
            'd' | 'm' => Some((n - 1) * 2 + u32::from(last == 'm')),
            _ => None,
        };
    }
    let simplified = key
        .replace(" major", "")
        .replace(" minor", "m")
        .replace("maj", "")
        .replace("min", "m");
    let names = [
        "c", "am", "g", "em", "d", "bm", "a", "f#m", "e", "c#m", "b", "g#m", "f#", "d#m", "c#",
        "a#m", "g#", "fm", "d#", "cm", "a#", "gm", "f", "dm",
    ];
    let canonical = match simplified.as_str() {
        "gb" => "f#",
        "db" => "c#",
        "ab" => "g#",
        "eb" => "d#",
        "bb" => "a#",
        "gbm" => "f#m",
        "dbm" => "c#m",
        "abm" => "g#m",
        "ebm" => "d#m",
        "bbm" => "a#m",
        s => s,
    };
    names.iter().position(|s| *s == canonical).map(|n| n as u32)
}

struct PerformanceData {
    track: Vec<u8>,
    beats: Option<Vec<u8>>,
    cues: Option<Vec<u8>>,
    loops: Option<Vec<u8>>,
}
fn compressed(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut out = (u32::try_from(bytes.len())?).to_be_bytes().to_vec();
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    out.extend(encoder.finish()?);
    Ok(out)
}
fn cue_name(name: &str, default: String) -> Vec<u8> {
    let value = if name.is_empty() { &default } else { name };
    let mut end = value.len().min(255);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.as_bytes()[..end].to_vec()
}
fn color(value: Option<u32>, nearest: bool) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 8] = [
        [244, 211, 69],
        [239, 129, 48],
        [146, 83, 196],
        [206, 50, 50],
        [135, 198, 62],
        [30, 188, 97],
        [48, 193, 185],
        [75, 136, 226],
    ];
    let rgb = value
        .map(|c| [(c >> 16) as u8, (c >> 8) as u8, c as u8])
        .unwrap_or(PALETTE[7]);
    if !nearest {
        return rgb;
    }
    *PALETTE
        .iter()
        .min_by_key(|p| {
            p.iter()
                .zip(rgb)
                .map(|(a, b)| (i32::from(*a) - i32::from(b)).pow(2))
                .sum::<i32>()
        })
        .unwrap()
}
fn performance_data(track: &SyncTrack, key: u32, options: &SyncOptions) -> Result<PerformanceData> {
    let rate = f64::from(track.sample_rate);
    let mut header = Vec::with_capacity(28);
    header.extend(rate.to_be_bytes());
    header.extend(((rate * track.duration_seconds).floor() as u64).to_be_bytes());
    header.extend(0_f64.to_be_bytes());
    header.extend(key.to_be_bytes());
    let beats = if options.skip_beatgrids || track.beatgrid.is_empty() {
        None
    } else {
        let points = beat_points(track)?;
        let mut bytes = Vec::new();
        bytes.extend(rate.to_be_bytes());
        bytes.extend((rate * track.duration_seconds).round().to_be_bytes());
        bytes.push(1);
        // Engine stores the editable grid and an identical rendered grid.
        for _ in 0..2 {
            bytes.extend((points.len() as u64).to_be_bytes());
            for (seconds, index, count) in &points {
                bytes.extend((seconds * rate).to_le_bytes());
                bytes.extend(index.to_le_bytes());
                bytes.extend(count.to_le_bytes());
                bytes.extend(0_u32.to_le_bytes());
            }
        }
        bytes.push(0);
        Some(compressed(&bytes)?)
    };
    let mut cues = [None; 8];
    let mut loops = [None; 8];
    for cue in &track.cues {
        ensure!(
            cue.index < 8,
            "Engine DJ supports eight cue slots and eight loop slots"
        );
        ensure!(
            cue.start_seconds.is_finite() && cue.start_seconds >= 0.0,
            "Invalid Engine DJ cue time"
        );
        let slots = if let Some(end) = cue.end_seconds {
            ensure!(
                end.is_finite() && end > cue.start_seconds,
                "Invalid Engine DJ loop end"
            );
            &mut loops
        } else {
            &mut cues
        };
        ensure!(
            slots[usize::from(cue.index)].replace(cue).is_none(),
            "Duplicate Engine DJ cue or loop slot"
        );
    }
    let cue_bytes = if cues.iter().all(Option::is_none) {
        None
    } else {
        let mut bytes = 8_u64.to_be_bytes().to_vec();
        for (index, cue) in cues.iter().enumerate() {
            if let Some(cue) = cue {
                let name = cue_name(&cue.name, format!("Cue {}", index + 1));
                bytes.push(name.len() as u8);
                bytes.extend(name);
                bytes.extend((cue.start_seconds * rate).round().to_be_bytes());
                bytes.push(255);
                bytes.extend(color(cue.color, options.nearest_color));
            } else {
                bytes.push(0);
                bytes.extend((-1_f64).to_be_bytes());
                bytes.extend([0; 4]);
            }
        }
        bytes.extend([0; 17]);
        Some(compressed(&bytes)?)
    };
    let loop_bytes = if loops.iter().all(Option::is_none) {
        None
    } else {
        let mut bytes = 8_u64.to_le_bytes().to_vec();
        for (index, cue) in loops.iter().enumerate() {
            if let Some(cue) = cue {
                let name = cue_name(&cue.name, format!("Loop {}", index + 1));
                bytes.push(name.len() as u8);
                bytes.extend(name);
                bytes.extend((cue.start_seconds * rate).round().to_le_bytes());
                bytes.extend((cue.end_seconds.unwrap() * rate).round().to_le_bytes());
                bytes.extend([1, 1, 255]);
                bytes.extend(color(cue.color, options.nearest_color));
            } else {
                bytes.push(0);
                bytes.extend((-1_f64).to_le_bytes());
                bytes.extend((-1_f64).to_le_bytes());
                bytes.extend([0, 0, 255, 0, 0, 0]);
            }
        }
        Some(bytes)
    };
    Ok(PerformanceData {
        track: compressed(&header)?,
        beats,
        cues: cue_bytes,
        loops: loop_bytes,
    })
}

/// Convert tempo segments into Engine's sparse anchors, including preroll and terminal anchor.
fn beat_points(track: &SyncTrack) -> Result<Vec<(f64, i64, u32)>> {
    let markers = &track.beatgrid;
    let mut points: Vec<(f64, i64, u32)> = Vec::new();
    let mut index = 0_i64;
    for (n, marker) in markers.iter().enumerate() {
        ensure!(
            marker.seconds.is_finite()
                && marker.seconds >= 0.0
                && marker.bpm.is_finite()
                && marker.bpm > 0.0,
            "Invalid Engine DJ tempo marker"
        );
        if n > 0 {
            ensure!(
                marker.seconds > markers[n - 1].seconds,
                "Engine DJ tempo markers must be ordered"
            );
        }
        let period = 60.0 / marker.bpm.min(500.0);
        if track.duration_seconds < period {
            break;
        }
        let mut position = marker.seconds;
        let next = markers.get(n + 1);
        let mut count = ((next.map_or(track.duration_seconds, |m| m.seconds) - position) / period)
            .round() as i64;
        if n == 0 {
            index = -4;
            count += 4;
            position -= 4.0 * period;
            if position > 0.0 {
                let preroll = (position / (4.0 * period)).ceil() as i64 * 4;
                points.push((position - preroll as f64 * period, -4, preroll as u32));
                index = preroll - 4;
            }
        }
        ensure!(
            (0..=u32::MAX as i64).contains(&count),
            "Engine DJ beat count exceeds bounds"
        );
        if count > 0 {
            points.push((position, index, count as u32));
        }
        index += count;
        if next.is_none() || count == 0 {
            if let Some(last) = points.last() {
                points.push((last.0 + (index - last.1) as f64 * period, index, 0));
            }
            break;
        }
    }
    Ok(points)
}

fn write_playlists(
    tx: &Transaction<'_>,
    library: &SyncLibrary,
    tracks: &HashMap<String, i64>,
    uuid: &str,
    options: &SyncOptions,
) -> Result<()> {
    let mut ids = HashMap::new();
    let mut native = HashMap::new();
    for playlist in &library.playlists {
        if let Ok(Some(crate::smart_rules::NativeSmart::EngineDj(rules))) =
            crate::smart_rules::compile(playlist, library, options, crate::DjSoftware::EngineDj)
        {
            native.insert(playlist.id.clone(), rules);
        }
    }
    let has_smart_table: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='Smartlist')",
        [],
        |row| row.get(0),
    )?;
    ensure!(
        native.is_empty() || has_smart_table,
        "Current Engine database has no Smartlist table"
    );
    let paths = engine_playlist_paths(library)?;
    let mut remaining: Vec<_> = library.playlists.iter().collect();
    let mut source_ids = HashSet::new();
    for playlist in &remaining {
        ensure!(
            source_ids.insert(&playlist.id),
            "Duplicate source playlist ID"
        );
    }
    while !remaining.is_empty() {
        let before = remaining.len();
        let mut deferred = Vec::new();
        for playlist in remaining {
            let parent = if let Some(parent) = &playlist.parent_id {
                if let Some(id) = ids.get(parent) {
                    *id
                } else {
                    deferred.push(playlist);
                    continue;
                }
            } else {
                0
            };
            if let Some(rules) = native.get(&playlist.id) {
                write_smart_playlist(tx, library, playlist, parent, rules, &native, &paths)?;
                continue;
            }
            if has_smart_table {
                remove_smart_representation(tx, playlist, &paths[&playlist.id])?;
            }
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM Playlist WHERE title=?1 AND parentListId=?2",
                    params![playlist.name, parent],
                    |r| r.get(0),
                )
                .optional()?;
            let id = if let Some(id) = existing {
                id
            } else {
                let id: i64 = tx.query_row("SELECT MAX(COALESCE((SELECT seq FROM sqlite_sequence WHERE name='Playlist'),0),COALESCE(MAX(id),0))+1 FROM Playlist", [], |r| r.get(0))?;
                tx.execute(
                    "UPDATE Playlist SET nextListId=?1 WHERE parentListId=?2 AND nextListId=0",
                    params![id, parent],
                )?;
                tx.execute("INSERT INTO Playlist(id,title,parentListId,nextListId,isPersisted,isExplicitlyExported,lastEditTime) VALUES(?1,?2,?3,0,1,0,datetime('now'))",params![id,playlist.name,parent])?;
                id
            };
            ids.insert(playlist.id.clone(), id);
            tx.execute(
                "UPDATE Playlist SET isPersisted=1,lastEditTime=datetime('now') WHERE id=?1",
                [id],
            )?;
            tx.execute("DELETE FROM PlaylistEntity WHERE listId=?1", [id])?;
            let mut previous = None;
            let mut seen = HashSet::new();
            for track in &playlist.track_ids {
                let track = tracks
                    .get(track)
                    .context("Engine DJ playlist references a missing source track")?;
                if !seen.insert(track) {
                    continue;
                }
                tx.execute("INSERT INTO PlaylistEntity(listId,trackId,databaseUuid,nextEntityId,membershipReference) VALUES(?1,?2,?3,0,0)",params![id,track,uuid])?;
                let entity = tx.last_insert_rowid();
                if let Some(previous) = previous {
                    tx.execute(
                        "UPDATE PlaylistEntity SET nextEntityId=?1 WHERE id=?2",
                        params![entity, previous],
                    )?;
                }
                previous = Some(entity);
            }
        }
        ensure!(
            deferred.len() < before,
            "Engine DJ playlist hierarchy has a cycle or missing parent"
        );
        remaining = deferred;
    }
    reorder_engine_playlists(tx, library, &ids)?;
    if !library.playlists.is_empty() {
        // Parent folders include descendant membership, retaining counts for shared tracks.
        tx.execute("DELETE FROM PlaylistEntity WHERE membershipReference>0", [])?;
        tx.execute("WITH RECURSIVE descendants(parent,child) AS (
            SELECT parentListId,id FROM Playlist WHERE parentListId<>0 UNION ALL
            SELECT d.parent,p.id FROM descendants d JOIN Playlist p ON p.parentListId=d.child)
            INSERT INTO PlaylistEntity(listId,trackId,databaseUuid,nextEntityId,membershipReference)
            SELECT d.parent,e.trackId,e.databaseUuid,0,COUNT(*) FROM descendants d JOIN PlaylistEntity e ON e.listId=d.child AND e.membershipReference=0
            WHERE NOT EXISTS(SELECT 1 FROM PlaylistEntity own WHERE own.listId=d.parent AND own.trackId=e.trackId AND own.databaseUuid IS e.databaseUuid AND own.membershipReference=0)
            GROUP BY d.parent,e.databaseUuid,e.trackId",[])?;
        for playlist in library
            .playlists
            .iter()
            .filter(|playlist| native.contains_key(&playlist.id))
        {
            let mut parent = playlist.parent_id.as_ref();
            while let Some(source_parent) = parent {
                let folder = *ids
                    .get(source_parent)
                    .context("Missing Engine smartlist folder")?;
                let mut seen = HashSet::new();
                for source_track in &playlist.track_ids {
                    if !seen.insert(source_track) {
                        continue;
                    }
                    let track = tracks
                        .get(source_track)
                        .context("Smartlist refers to an unselected track")?;
                    let direct:bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM PlaylistEntity WHERE listId=?1 AND trackId=?2 AND databaseUuid=?3 AND membershipReference=0)",params![folder,track,uuid],|row|row.get(0))?;
                    if direct {
                        continue;
                    }
                    let updated=tx.execute("UPDATE PlaylistEntity SET membershipReference=membershipReference+1 WHERE listId=?1 AND trackId=?2 AND databaseUuid=?3 AND membershipReference>0",params![folder,track,uuid])?;
                    if updated == 0 {
                        tx.execute("INSERT INTO PlaylistEntity(listId,trackId,databaseUuid,nextEntityId,membershipReference) VALUES(?1,?2,?3,0,1)",params![folder,track,uuid])?;
                    }
                }
                parent = library
                    .playlists
                    .iter()
                    .find(|playlist| &playlist.id == source_parent)
                    .and_then(|playlist| playlist.parent_id.as_ref());
            }
        }
        tx.execute("WITH ordered AS (SELECT id,LEAD(id,1,0) OVER(PARTITION BY listId ORDER BY id) nextId FROM PlaylistEntity WHERE membershipReference>0)
            UPDATE PlaylistEntity SET nextEntityId=(SELECT nextId FROM ordered WHERE ordered.id=PlaylistEntity.id) WHERE membershipReference>0",[])?;
        tx.execute("UPDATE PlaylistEntity SET nextEntityId=(SELECT MIN(d.id) FROM PlaylistEntity d WHERE d.listId=PlaylistEntity.listId AND d.membershipReference>0)
            WHERE membershipReference=0 AND nextEntityId=0 AND EXISTS(SELECT 1 FROM PlaylistEntity d WHERE d.listId=PlaylistEntity.listId AND d.membershipReference>0)",[])?;
    }
    Ok(())
}

fn unlink_engine_playlist(tx: &Transaction<'_>, id: i64, parent: i64) -> Result<()> {
    let next: i64 = tx.query_row("SELECT nextListId FROM Playlist WHERE id=?1", [id], |row| {
        row.get(0)
    })?;
    let minimum: i64 = tx.query_row(
        "SELECT MIN(nextListId) FROM Playlist WHERE parentListId=?1",
        [parent],
        |row| row.get(0),
    )?;
    let temporary = minimum
        .min(0)
        .checked_sub(1)
        .context("Engine playlist order overflow")?;
    tx.execute(
        "UPDATE Playlist SET nextListId=?1 WHERE id=?2",
        params![temporary, id],
    )?;
    tx.execute(
        "UPDATE Playlist SET nextListId=?1 WHERE parentListId=?2 AND nextListId=?3",
        params![next, parent, id],
    )?;
    tx.execute("DELETE FROM Playlist WHERE id=?1", [id])?;
    Ok(())
}
fn reorder_engine_playlists(
    tx: &Transaction<'_>,
    library: &SyncLibrary,
    ids: &HashMap<String, i64>,
) -> Result<()> {
    let mut groups: HashMap<i64, Vec<i64>> = HashMap::new();
    for playlist in &library.playlists {
        let Some(id) = ids.get(&playlist.id) else {
            continue;
        };
        let parent = playlist
            .parent_id
            .as_ref()
            .and_then(|id| ids.get(id))
            .copied()
            .unwrap_or(0);
        groups.entry(parent).or_default().push(*id);
    }
    for (parent, desired) in groups {
        let mut query = tx.prepare("SELECT id,nextListId FROM Playlist WHERE parentListId=?1")?;
        let rows = query
            .query_map([parent], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<HashMap<_, _>>>()?;
        let pointed: HashSet<_> = rows.values().filter(|id| **id != 0).copied().collect();
        let heads: Vec<_> = rows
            .keys()
            .filter(|id| !pointed.contains(id))
            .copied()
            .collect();
        ensure!(
            heads.len() == 1,
            "Engine playlist sibling chain is ambiguous"
        );
        let mut current = heads[0];
        let mut existing = Vec::new();
        let mut seen = HashSet::new();
        while current != 0 {
            ensure!(
                seen.insert(current),
                "Engine playlist sibling chain contains a cycle"
            );
            existing.push(current);
            current = *rows
                .get(&current)
                .context("Engine playlist sibling link is missing")?;
        }
        ensure!(
            existing.len() == rows.len(),
            "Engine playlist sibling chain is incomplete"
        );
        let selected: HashSet<_> = desired.iter().copied().collect();
        let mut replacements = desired.iter();
        let mut ordered = Vec::new();
        for id in existing {
            if selected.contains(&id) {
                if let Some(id) = replacements.next() {
                    ordered.push(*id);
                }
            } else {
                ordered.push(id);
            }
        }
        ordered.extend(replacements.copied());
        let mut sentinel = rows.values().copied().min().unwrap_or(0).min(0);
        for id in &ordered {
            sentinel = sentinel
                .checked_sub(1)
                .context("Engine playlist order overflow")?;
            tx.execute(
                "UPDATE Playlist SET nextListId=?1 WHERE id=?2",
                params![sentinel, id],
            )?;
        }
        for (position, id) in ordered.iter().enumerate() {
            tx.execute(
                "UPDATE Playlist SET nextListId=?1 WHERE id=?2",
                params![ordered.get(position + 1).copied().unwrap_or(0), id],
            )?;
        }
    }
    Ok(())
}

fn engine_playlist_paths(library: &SyncLibrary) -> Result<HashMap<String, Vec<String>>> {
    let nodes: HashMap<_, _> = library
        .playlists
        .iter()
        .map(|playlist| (&playlist.id, playlist))
        .collect();
    let mut result = HashMap::new();
    for playlist in &library.playlists {
        let mut parts = vec![playlist.name.clone()];
        let mut parent = playlist.parent_id.as_ref();
        let mut seen = HashSet::from([&playlist.id]);
        while let Some(id) = parent {
            ensure!(
                seen.insert(id),
                "Engine playlist hierarchy contains a cycle"
            );
            let node = nodes.get(id).context("Missing Engine playlist parent")?;
            parts.push(node.name.clone());
            parent = node.parent_id.as_ref();
        }
        parts.reverse();
        result.insert(playlist.id.clone(), parts);
    }
    Ok(result)
}
fn engine_smart_uuid(id: &str) -> String {
    let hash = Sha1::digest(format!("cuerithm:engine:smart:{id}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);
    bytes[6] = (bytes[6] & 15) | 0x50;
    bytes[8] = (bytes[8] & 63) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}
fn native_path(parts: &[String]) -> String {
    if parts.is_empty() {
        String::new()
    } else {
        format!("{};", parts.join(";"))
    }
}
fn matching_smart_uuid(
    tx: &Transaction<'_>,
    playlist: &crate::SyncPlaylist,
    path: &[String],
) -> Result<Option<String>> {
    let generated = engine_smart_uuid(&playlist.id);
    let mut query = tx.prepare(
        "SELECT listUuid FROM Smartlist WHERE listUuid=?1 OR (title=?2 AND parentPlaylistPath=?3)",
    )?;
    let rows = query
        .query_map(
            params![
                generated,
                playlist.name,
                native_path(&path[..path.len() - 1])
            ],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        rows.len() <= 1,
        "Ambiguous ordinary/smart Engine playlist names"
    );
    Ok(rows.into_iter().next())
}
fn remove_smart_representation(
    tx: &Transaction<'_>,
    playlist: &crate::SyncPlaylist,
    path: &[String],
) -> Result<()> {
    if let Some(id) = matching_smart_uuid(tx, playlist, path)? {
        tx.execute(
            "UPDATE Smartlist SET nextListUuid='',nextPlaylistPath=?1 WHERE nextListUuid=?2",
            params![native_path(path), id],
        )?;
        tx.execute("DELETE FROM Smartlist WHERE listUuid=?1", [id])?;
    }
    Ok(())
}
fn write_smart_playlist(
    tx: &Transaction<'_>,
    library: &SyncLibrary,
    playlist: &crate::SyncPlaylist,
    parent: i64,
    rules: &str,
    native: &HashMap<String, String>,
    paths: &HashMap<String, Vec<String>>,
) -> Result<()> {
    let path = &paths[&playlist.id];
    ensure!(
        path.iter().all(|name| !name.contains(';')),
        "Engine smartlist paths cannot contain semicolons"
    );
    let id = engine_smart_uuid(&playlist.id);
    let old = matching_smart_uuid(tx, playlist, path)?;
    if let Some(old) = &old
        && old != &id
    {
        tx.execute(
            "UPDATE Smartlist SET nextListUuid=?1 WHERE nextListUuid=?2",
            params![id, old],
        )?;
        tx.execute(
            "UPDATE Smartlist SET listUuid=?1 WHERE listUuid=?2",
            params![id, old],
        )?;
    }
    let prior:Option<(String,String,String)>=tx.query_row("SELECT parentPlaylistPath,nextListUuid,nextPlaylistPath FROM Smartlist WHERE listUuid=?1",[&id],
        |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional()?;
    let parent_path = native_path(&path[..path.len() - 1]);
    if let Some((old_parent, next_uuid, next_path)) = &prior
        && *old_parent != parent_path
    {
        tx.execute("UPDATE Smartlist SET nextListUuid=?1,nextPlaylistPath=?2 WHERE nextListUuid=?3 AND parentPlaylistPath=?4",params![next_uuid,next_path,id,old_parent])?;
    }
    let siblings: Vec<_> = library
        .playlists
        .iter()
        .filter(|other| other.parent_id == playlist.parent_id)
        .collect();
    let position = siblings
        .iter()
        .position(|other| other.id == playlist.id)
        .context("Missing Engine smartlist order")?;
    let next = siblings.get(position + 1);
    let (mut next_uuid, mut next_path) = (String::new(), String::new());
    if let Some(next) = next {
        if native.contains_key(&next.id) {
            next_uuid = engine_smart_uuid(&next.id);
        } else {
            next_path = native_path(&paths[&next.id]);
        }
    } else if let Some((_, previous_uuid, previous_path)) = &prior {
        let selected_uuid = native
            .keys()
            .any(|source| engine_smart_uuid(source) == *previous_uuid);
        let selected_path = paths
            .values()
            .any(|parts| native_path(parts) == *previous_path);
        if !selected_uuid && !selected_path {
            next_uuid = previous_uuid.clone();
            next_path = previous_path.clone();
        }
    }
    let ordinary: Option<i64> = tx
        .query_row(
            "SELECT id FROM Playlist WHERE title=?1 AND parentListId=?2",
            params![playlist.name, parent],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(ordinary) = ordinary {
        let children: i64 = tx.query_row(
            "SELECT COUNT(*) FROM Playlist WHERE parentListId=?1",
            [ordinary],
            |row| row.get(0),
        )?;
        ensure!(
            children == 0,
            "An existing Engine folder cannot become a smartlist"
        );
        tx.execute(
            "UPDATE Smartlist SET nextListUuid=?1,nextPlaylistPath='' WHERE nextPlaylistPath=?2",
            params![id, native_path(path)],
        )?;
        tx.execute("DELETE FROM PlaylistEntity WHERE listId=?1", [ordinary])?;
        unlink_engine_playlist(tx, ordinary, parent)?;
    }
    if old.is_none() && next.is_none() {
        tx.execute("UPDATE Smartlist SET nextListUuid=?1 WHERE parentPlaylistPath=?2 AND nextListUuid='' AND nextPlaylistPath=''",params![id,parent_path])?;
    }
    let updated=tx.execute("UPDATE Smartlist SET title=?1,parentPlaylistPath=?2,nextListUuid=?3,nextPlaylistPath=?4,rules=?5,lastEditTime=datetime('now') WHERE listUuid=?6",
        params![playlist.name,parent_path,next_uuid,next_path,rules,id])?;
    if updated == 0 {
        tx.execute("INSERT INTO Smartlist(listUuid,title,parentPlaylistPath,nextListUuid,nextPlaylistPath,rules,lastEditTime) VALUES(?1,?2,?3,?4,?5,?6,datetime('now'))",
            params![id,playlist.name,parent_path,next_uuid,next_path,rules])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyncBeat, SyncCue, SyncPlaylist};
    use flate2::read::ZlibDecoder;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    pub(super) fn track(path: PathBuf) -> SyncTrack {
        SyncTrack {
            id: "source".into(),
            path,
            title: "Synthetic".into(),
            artist: "Test".into(),
            key: "8A".into(),
            bpm: 120.0,
            duration_seconds: 20.0,
            sample_rate: 48_000,
            rating: 4,
            cues: vec![
                SyncCue {
                    index: 2,
                    name: "Drop".into(),
                    start_seconds: 3.25,
                    color: Some(0xce3232),
                    ..Default::default()
                },
                SyncCue {
                    index: 0,
                    name: "Loop".into(),
                    start_seconds: 4.0,
                    end_seconds: Some(8.0),
                    color: Some(0x1ebc61),
                },
            ],
            beatgrid: vec![SyncBeat {
                seconds: 0.0,
                bpm: 120.0,
                beat: 1,
            }],
            ..Default::default()
        }
    }
    fn inflate(data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        ZlibDecoder::new(&data[4..])
            .read_to_end(&mut result)
            .unwrap();
        assert_eq!(
            u32::from_be_bytes(data[..4].try_into().unwrap()) as usize,
            result.len()
        );
        result
    }
    #[test]
    fn native_payload_matches_independent_reference_outputs() {
        // SHA-256 values obtained by evaluating the shipped writer on synthetic inputs;
        // compare decompressed bytes so compression-library implementation choices do not matter.
        let data =
            performance_data(&track("/synthetic.wav".into()), 1, &SyncOptions::default()).unwrap();
        for (bytes, expected) in [
            (
                inflate(&data.track),
                "615123a3a787b5efec8ef6466a775175b15ca1649aef62fa682b6c27789693ea",
            ),
            (
                inflate(data.beats.as_ref().unwrap()),
                "9a5aea661f65f8fc202d17095f8498da967698787c6cae628231b503da484c63",
            ),
            (
                inflate(data.cues.as_ref().unwrap()),
                "eae7d5de0e8d1c2d8fd28b4dcc8017541d16f495c3b1df604bfae859273e85ee",
            ),
            (
                data.loops.unwrap(),
                "4940f312312047571eccd9b65d2c84ef0a9be8e423a9fdea04968faeb2821dae",
            ),
        ] {
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
        }
    }
    #[test]
    fn hardware_media_is_copied_and_database_paths_reference_device_music() {
        let (dir, database) = fixture();
        let source = dir.path().join("source.wav");
        std::fs::write(&source, b"synthetic device media").unwrap();
        let library = SyncLibrary {
            tracks: vec![track(source.clone())],
            ..Default::default()
        };
        let library_root = database.parent().unwrap().parent().unwrap();
        let stage = dir.path().join("staged");
        let (projected, files) = stage_device_media(&stage, library_root, &library).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].0.starts_with("Music"));
        assert_eq!(
            std::fs::read(&files[0].1).unwrap(),
            std::fs::read(&source).unwrap()
        );
        let staged_db = stage.join("m.db");
        std::fs::copy(&database, &staged_db).unwrap();
        write_staged(
            &staged_db,
            library_root,
            &projected,
            &SyncOptions::default(),
        )
        .unwrap();
        let db = Connection::open(staged_db).unwrap();
        let path: String = db
            .query_row("SELECT path FROM Track WHERE title='Synthetic'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            path,
            format!(
                "Music/{}",
                files[0].0.file_name().unwrap().to_str().unwrap()
            )
        );
        assert_eq!(library.tracks[0].path, source);
    }
    pub(super) fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Engine Library/Database2/m.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let db = Connection::open(&path).unwrap();
        // A synthetic schema exercises the native database contract, without library data.
        db.execute_batch("CREATE TABLE Information(uuid TEXT,schemaVersionMajor INTEGER);
            INSERT INTO Information VALUES('test-library',3);
            CREATE TABLE Track(id INTEGER PRIMARY KEY AUTOINCREMENT,path TEXT UNIQUE,filename TEXT,fileType TEXT,
                isAvailable INTEGER,isAnalyzed INTEGER,isMetadataImported INTEGER,originDatabaseUuid TEXT,
                originTrackId INTEGER,dateCreated INTEGER,dateAdded INTEGER,playOrder INTEGER,playedIndicator INTEGER,
                isMetadataOfPackedTrackChanged INTEGER,isPerfomanceDataOfPackedTrackChanged INTEGER,albumArt TEXT,albumArtId INTEGER,
                title TEXT,artist TEXT,album TEXT,genre TEXT,label TEXT,comment TEXT,composer TEXT,remixer TEXT,bpm REAL,bpmAnalyzed REAL,
                length INTEGER,year INTEGER,key INTEGER,rating INTEGER,isBeatGridLocked INTEGER,lastEditTime INTEGER);
            CREATE TABLE PerformanceData(trackId INTEGER PRIMARY KEY REFERENCES Track(id),trackData BLOB,
                quickCues BLOB,loops BLOB,beatData BLOB,activeOnLoadLoops INTEGER,overviewWaveFormData BLOB);
            CREATE TABLE AlbumArt(id INTEGER PRIMARY KEY AUTOINCREMENT,hash TEXT,albumArt BLOB);
            CREATE TRIGGER performance_row AFTER INSERT ON Track BEGIN INSERT INTO PerformanceData(trackId) VALUES(NEW.id); END;
            CREATE TABLE Playlist(id INTEGER PRIMARY KEY AUTOINCREMENT,title TEXT,parentListId INTEGER,
                nextListId INTEGER,isPersisted INTEGER,isExplicitlyExported INTEGER,lastEditTime TEXT,
                UNIQUE(title,parentListId),UNIQUE(parentListId,nextListId));
            CREATE TABLE PlaylistEntity(id INTEGER PRIMARY KEY AUTOINCREMENT,listId INTEGER REFERENCES Playlist(id),
                trackId INTEGER,databaseUuid TEXT,nextEntityId INTEGER,membershipReference INTEGER,
                UNIQUE(listId,databaseUuid,trackId));
            INSERT INTO Track(path,title,isBeatGridLocked) VALUES('unrelated.wav','Keep',0);
            INSERT INTO Playlist(title,parentListId,nextListId,isPersisted) VALUES('Keep',0,0,1);").unwrap();
        (dir, path)
    }
    #[test]
    fn portable_source_copy_survives_preparation_reopen_apply_and_restore() {
        let (dir, database) = fixture();
        let root = dir.path().canonicalize().unwrap();
        #[cfg(all(unix, not(target_os = "macos")))]
        let filename = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(b"source-\xff.wav".to_vec())
        };
        // APFS rejects invalid UTF-8 names at creation; exercise the same
        // publication flow with a representable name on macOS.
        #[cfg(target_os = "macos")]
        let filename = std::ffi::OsString::from("source-\u{e000}.wav");
        #[cfg(windows)]
        let filename = {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(&[0x61, 0xd800, 0x2e, 0x77, 0x61, 0x76])
        };
        let source = root.join(filename);
        let original_audio = b"synthetic portable source audio";
        std::fs::write(&source, original_audio).unwrap();
        let original_database = std::fs::read(&database).unwrap();
        let library = SyncLibrary {
            tracks: vec![track(source.clone())],
            ..Default::default()
        };
        let journals = root.join("journals");
        let preview = crate::prepare_sync(
            &journals,
            crate::SyncTarget {
                software: crate::DjSoftware::EngineDj,
                path: database.clone(),
                source_root: None,
            },
            &library,
            &SyncOptions {
                engine_copy_audio: true,
                ..Default::default()
            },
        )
        .unwrap();
        let journal = crate::transaction::FileTransaction::open(&journals, &preview.id).unwrap();
        assert!(journal.path().join("request.json").is_file());
        crate::apply_sync(&journals, &preview.id).unwrap();
        let connection = Connection::open(&database).unwrap();
        let relative: String = connection
            .query_row("SELECT path FROM Track WHERE title='Synthetic'", [], |r| {
                r.get(0)
            })
            .unwrap();
        drop(connection);
        let copied = database.parent().unwrap().parent().unwrap().join(relative);
        assert!(copied.file_name().unwrap().to_str().is_some());
        assert_eq!(std::fs::read(&copied).unwrap(), original_audio);
        assert_eq!(std::fs::read(&source).unwrap(), original_audio);
        crate::restore_sync(&journals, &preview.id).unwrap();
        assert_eq!(std::fs::read(&database).unwrap(), original_database);
        assert_eq!(std::fs::read(&source).unwrap(), original_audio);
        assert!(!copied.exists());
    }
    #[test]
    fn owned_playlist_rename_keeps_native_id() {
        let (dir, path) = fixture();
        let library_root = path.parent().unwrap().parent().unwrap();
        let mut library = SyncLibrary {
            tracks: vec![track(dir.path().join("source.wav"))],
            playlists: vec![SyncPlaylist {
                id: "set".into(),
                name: "First".into(),
                track_ids: vec!["source".into()],
                ..Default::default()
            }],
        };
        let first = write_staged_owned(
            &path,
            library_root,
            &library,
            &SyncOptions::default(),
            &crate::ownership::PlaylistOwnershipRequest::default(),
        )
        .unwrap();
        assert_eq!(first.current.bindings.len(), 1);
        assert!(first.current.bindings[0].created);
        library.playlists[0].name = "Renamed".into();
        let request = crate::ownership::PlaylistOwnershipRequest {
            previous: first.current.clone(),
            ..Default::default()
        };
        let next = write_staged_owned(
            &path,
            library_root,
            &library,
            &SyncOptions::default(),
            &request,
        )
        .unwrap();
        assert_eq!(
            next.current.bindings[0].locator,
            first.current.bindings[0].locator
        );
        let db = Connection::open(path).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM Playlist WHERE title IN('First','Renamed')",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }
    #[test]
    fn merges_database_preserving_existing_data_and_rolls_up_folder_membership() {
        let (dir, path) = fixture();
        let library = SyncLibrary {
            tracks: vec![track(dir.path().join("source.wav"))],
            // Child first deliberately exercises hierarchy resolution.
            playlists: vec![
                SyncPlaylist {
                    id: "child".into(),
                    parent_id: Some("folder".into()),
                    name: "Set".into(),
                    track_ids: vec!["source".into(), "source".into()],
                    ..Default::default()
                },
                SyncPlaylist {
                    id: "folder".into(),
                    name: "Folder".into(),
                    folder: true,
                    ..Default::default()
                },
            ],
        };
        write(
            path.parent().unwrap().parent().unwrap(),
            &library,
            &SyncOptions::default(),
        )
        .unwrap();
        write(&path, &library, &SyncOptions::default()).unwrap();
        let db = Connection::open(&path).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Track", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Playlist", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM PlaylistEntity", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        let metadata: (String, i64, i64, i64) = db
            .query_row(
                "SELECT path,key,rating,isBeatGridLocked FROM Track WHERE title='Synthetic'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(metadata, ("../source.wav".into(), 1, 80, 1));
        assert_eq!(db.query_row("SELECT membershipReference FROM PlaylistEntity JOIN Playlist ON listId=Playlist.id WHERE title='Folder'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        db.execute("UPDATE PerformanceData SET overviewWaveFormData=X'1234',beatData=X'5678' WHERE trackId=2",[]).unwrap();
        let options = SyncOptions {
            skip_beatgrids: true,
            beatgrid_lock: BeatgridLock::Unlock,
            ..Default::default()
        };
        write(&path, &library, &options).unwrap();
        let blobs: (Vec<u8>, Vec<u8>) = db
            .query_row(
                "SELECT overviewWaveFormData,beatData FROM PerformanceData WHERE trackId=2",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(blobs, (vec![0x12, 0x34], vec![0x56, 0x78]));
        assert_eq!(
            db.query_row("SELECT isBeatGridLocked FROM Track WHERE id=2", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            0
        );
    }
    #[test]
    fn invalid_hierarchy_rolls_back_track_updates_and_legacy_is_rejected() {
        let (dir, path) = fixture();
        let library = SyncLibrary {
            tracks: vec![track(dir.path().join("source.wav"))],
            playlists: vec![SyncPlaylist {
                id: "orphan".into(),
                parent_id: Some("missing".into()),
                ..Default::default()
            }],
        };
        assert!(write(&path, &library, &SyncOptions::default()).is_err());
        let db = Connection::open(&path).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Track", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        db.execute("UPDATE Information SET schemaVersionMajor=2", [])
            .unwrap();
        assert!(
            write(&path, &SyncLibrary::default(), &SyncOptions::default())
                .unwrap_err()
                .to_string()
                .contains("schema major 3")
        );
    }
    #[test]
    fn keys_and_utf8_cue_names_are_encoded_without_splitting_characters() {
        assert_eq!(key_number("8A"), Some(1));
        assert_eq!(key_number("1B"), Some(10));
        assert_eq!(key_number("C♯ minor"), Some(9));
        assert_eq!(key_number("unknown"), None);
        let bytes = cue_name(&"é".repeat(200), String::new());
        assert_eq!(bytes.len(), 254);
        assert!(std::str::from_utf8(&bytes).is_ok());
    }

    #[test]
    fn album_art_is_normalized_deduplicated_and_linked() {
        let (dir, path) = fixture();
        let mut track = track(dir.path().join("source.wav"));
        let image = image::RgbImage::from_pixel(512, 512, image::Rgb([20, 40, 60]));
        let mut jpeg = std::io::Cursor::new(Vec::new());
        image.write_to(&mut jpeg, image::ImageFormat::Jpeg).unwrap();
        track.artwork = Some(jpeg.into_inner());
        let library = SyncLibrary {
            tracks: vec![track],
            ..Default::default()
        };
        let options = SyncOptions {
            add_album_art: true,
            ..Default::default()
        };
        write(&path, &library, &options).unwrap();
        write(&path, &library, &options).unwrap();
        let db = Connection::open(path).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM AlbumArt", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        let png:Vec<u8>=db.query_row("SELECT AlbumArt.albumArt FROM AlbumArt JOIN Track ON Track.albumArtId=AlbumArt.id WHERE title='Synthetic'",[],|r|r.get(0)).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!((image.width(), image.height()), (256, 256));
    }

    #[test]
    fn native_smart_rows_preserve_order_membership_and_toggle_representation() {
        use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
        let (dir, path) = fixture();
        let db = Connection::open(&path).unwrap();
        db.execute_batch("CREATE TABLE Smartlist(listUuid TEXT PRIMARY KEY,title TEXT,parentPlaylistPath TEXT,nextPlaylistPath TEXT,nextListUuid TEXT,rules TEXT,lastEditTime TEXT)").unwrap();
        let definition = SmartDefinition {
            match_all: true,
            rules: vec![SmartRule {
                id: "bpm".into(),
                field: "bpm".into(),
                operator: Operator::NumberEquals,
                value: RuleValue::Number(120.0),
                or: false,
            }],
        };
        let mut input = SyncLibrary {
            tracks: vec![track(dir.path().join("source.wav"))],
            playlists: vec![
                SyncPlaylist {
                    id: "folder".into(),
                    name: "Folder".into(),
                    folder: true,
                    ..Default::default()
                },
                SyncPlaylist {
                    id: "smart-first".into(),
                    name: "Native First".into(),
                    parent_id: Some("folder".into()),
                    track_ids: vec!["source".into()],
                    smart: Some(definition.clone()),
                    ..Default::default()
                },
                SyncPlaylist {
                    id: "plain".into(),
                    name: "Plain".into(),
                    parent_id: Some("folder".into()),
                    track_ids: vec!["source".into()],
                    ..Default::default()
                },
                SyncPlaylist {
                    id: "smart-last".into(),
                    name: "Native Last".into(),
                    parent_id: Some("folder".into()),
                    track_ids: vec!["source".into()],
                    smart: Some(definition),
                    ..Default::default()
                },
            ],
        };
        write(&path, &input, &SyncOptions::default()).unwrap();
        let smart_id = engine_smart_uuid("smart-first");
        let (parent, next): (String, String) = db
            .query_row(
                "SELECT parentPlaylistPath,nextPlaylistPath FROM Smartlist WHERE listUuid=?1",
                [&smart_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(parent, "Folder;");
        assert_eq!(next, "Folder;Plain;");
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Smartlist", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(db.query_row("SELECT membershipReference FROM PlaylistEntity e JOIN Playlist p ON p.id=e.listId WHERE p.title='Folder'",[],|row|row.get::<_,i64>(0)).unwrap(),3);
        input.playlists[1].name = "Renamed".into();
        write(&path, &input, &SyncOptions::default()).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT title FROM Smartlist WHERE listUuid=?1",
                [&smart_id],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "Renamed"
        );
        write(
            &path,
            &input,
            &SyncOptions {
                materialize_smartlists: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Smartlist", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let first: i64 = db
            .query_row("SELECT id FROM Playlist WHERE title='Renamed'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            db.query_row(
                "SELECT title FROM Playlist WHERE id=(SELECT nextListId FROM Playlist WHERE id=?1)",
                [first],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "Plain"
        );
        write(&path, &input, &SyncOptions::default()).unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Smartlist", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM Playlist WHERE title IN ('Renamed','Native Last')",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM Track WHERE title='Keep'", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
    }
}

//! Serato DJ Pro 4 location database and native audio metadata writer.
//!
//! Select `~/Library/Application Support/Serato/Library/root.sqlite` on macOS,
//! or `%LOCALAPPDATA%/Serato/Library/root.sqlite` on Windows. `master.sqlite`
//! is Serato's aggregate database, not this writer's destination. Initialize the
//! library in Serato first. No Serato 3 database or crate files are produced.
//! Cues, loops and grids live in the audio files. Callers must back up every
//! `SyncTrack.path` as well as the SQLite database before invoking this function.

mod aggregate;
mod media;
mod owned;

pub use owned::{source_root_for_database, write_database_owned};

pub use aggregate::{refresh_master, refresh_master_owned};

use crate::model::{BeatgridLock, SyncLibrary, SyncOptions, SyncPlaylist, SyncTrack};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, functions::FunctionFlags, params};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Route local, removable-volume and network tracks to their Serato 4 database.
/// `local_library` is the directory containing the local `root.sqlite`.
/// Return the database and, for nonlocal tracks, the root used by portable IDs.
/// Read existing network `location.info` UUIDs; never invent library identities.
pub fn location_for_track(
    local_library: &Path,
    audio_path: &Path,
) -> Result<(PathBuf, Option<PathBuf>)> {
    location_for_track_in_environment(
        local_library,
        audio_path,
        &cuerithm_rekordbox::TargetEnvironment::Native,
    )
}

/// Route through physical databases while retaining the native volume grammar.
pub fn location_for_track_in_environment(
    local_library: &Path,
    audio_path: &Path,
    environment: &cuerithm_rekordbox::TargetEnvironment,
) -> Result<(PathBuf, Option<PathBuf>)> {
    let audio = native_serato_path(&environment.to_native(audio_path)?)?;
    let local = native_serato_path(&environment.to_native(local_library)?)?;
    let root = if audio.starts_with("//") {
        let components: Vec<_> = audio.trim_start_matches('/').split('/').collect();
        ensure!(
            components.len() >= 3 && !components[0].is_empty() && !components[1].is_empty(),
            "Invalid Serato network audio path"
        );
        Some(format!("//{}/{}", components[0], components[1]))
    } else if let Some(path) = audio.strip_prefix("/Volumes/") {
        let volume = path
            .split('/')
            .next()
            .context("Missing Serato audio volume")?;
        if volume.eq_ignore_ascii_case("Macintosh HD") {
            None
        } else {
            Some(format!("/Volumes/{volume}"))
        }
    } else if audio.as_bytes().get(1) == Some(&b':') {
        let primary = if local.as_bytes().get(1) == Some(&b':') {
            &local[..2]
        } else {
            "C:"
        };
        if audio[..2].eq_ignore_ascii_case(primary) || audio[..2].eq_ignore_ascii_case("C:") {
            None
        } else {
            Some(format!("{}/", &audio[..2]))
        }
    } else {
        None
    };
    let Some(root) = root else {
        let path = local_library.join("root.sqlite");
        ensure!(
            path.is_file(),
            "Serato 4 root.sqlite is missing; initialize the library in Serato first"
        );
        return Ok((path, None));
    };
    let root = environment.to_host(Path::new(&root))?;
    let info = root.join("_Serato_").join("Library").join("location.info");
    let database = if info.is_file() {
        let text = std::fs::read_to_string(&info).context("Read Serato network location.info")?;
        let uuid = uuid::Uuid::parse_str(text.trim())
            .context("Invalid UUID in Serato network location.info")?;
        local_library.join(format!("{}.sqlite", uuid.hyphenated()))
    } else {
        ensure!(
            !audio.starts_with("//"),
            "Serato network location.info is missing; initialize the share in Serato first"
        );
        root.join("_Serato_")
            .join("Library")
            .join("location.sqlite")
    };
    ensure!(
        database.is_file(),
        "Serato location database is missing; initialize this drive or share in Serato first"
    );
    Ok((database, Some(root)))
}

/// Validate all audio containers and serialize their Serato payloads without writes.
pub fn validate(library: &SyncLibrary, options: &SyncOptions) -> Result<()> {
    for track in &library.tracks {
        media::validate(track, options)?;
    }
    Ok(())
}

/// Update an existing Serato 4 root database and the selected audio files.
/// The caller owns backup/rollback across the database and the audio files.
pub fn write(path: &Path, library: &SyncLibrary, options: &SyncOptions) -> Result<()> {
    write_database(path, library, options)?;
    for track in &library.tracks {
        write_media(&track.path, track, options)?;
    }
    Ok(())
}

/// Write only a staged SQLite database, retaining original track paths.
pub fn write_database(path: &Path, library: &SyncLibrary, options: &SyncOptions) -> Result<()> {
    ensure!(
        path.file_name().and_then(|s| s.to_str()) == Some("root.sqlite"),
        "Select the Serato 4 Library/root.sqlite database; master.sqlite is an aggregate cache"
    );
    write_database_inner(path, None, library, options)
}

/// Write a staged external/network location database using its original volume root.
/// External targets are `<volume>/_Serato_/Library/location.sqlite`. Network
/// targets are `<local Serato Library>/<uuid>.sqlite`, with the UUID read from
/// `<share>/_Serato_/Library/location.info`. Initialize the location in Serato
/// before syncing; this function preserves its native schema and identity.
pub fn write_location_database(
    path: &Path,
    source_root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> Result<()> {
    ensure!(
        path.file_name().and_then(|s| s.to_str()) != Some("master.sqlite"),
        "master.sqlite is not a location database"
    );
    ensure!(
        path.extension().and_then(|s| s.to_str()) == Some("sqlite"),
        "Select a Serato SQLite location database"
    );
    write_database_inner(path, Some(source_root), library, options)
}

fn write_database_inner(
    path: &Path,
    source_root: Option<&Path>,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> Result<()> {
    write_database_inner_owned(path, source_root, library, options, None).map(|_| ())
}

fn write_database_inner_owned(
    path: &Path,
    source_root: Option<&Path>,
    library: &SyncLibrary,
    options: &SyncOptions,
    ownership: Option<&crate::ownership::PlaylistOwnershipRequest>,
) -> Result<crate::ownership::PlaylistOwnershipReport> {
    validate(library, options)?;
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .context("Open existing Serato 4 database")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    register_functions(&conn)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    ensure!(
        version > 0,
        "Serato database has no schema version; open and close Serato 4 first"
    );
    let mut revision: i64 = conn
        .query_row("SELECT revision FROM master LIMIT 1", [], |r| r.get(0))
        .context("Serato 4 master record is missing; add a track in Serato first")?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let tx = conn.transaction()?;
    let space: i64 = tx
        .query_row(
            "SELECT id FROM space WHERE name='Serato Library' COLLATE NOCASE",
            [],
            |r| r.get(0),
        )
        .context("Serato Library space is missing")?;
    let base: i64 = tx.query_row(
        "SELECT id FROM container WHERE parent_id IS NULL AND type=0 AND name='root'",
        [],
        |r| r.get(0),
    )?;
    let root = container(
        &tx,
        "Serato Library root",
        base,
        0,
        space,
        &mut revision,
        now,
    )?;
    let mut ids = HashMap::new();
    let rows = tx
        .prepare("SELECT portable_id,id FROM asset")?
        .query_map([], |r| {
            Ok((normalize(&r.get::<_, String>(0)?), r.get::<_, i64>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut existing = HashMap::new();
    for (reference, id) in rows {
        ensure!(
            existing.insert(reference, id).is_none(),
            "Ambiguous Serato native source reference"
        );
    }
    for track in &library.tracks {
        let portable = if let Some(root) = source_root {
            location_portable_id(track.reference_path(), root)?
        } else {
            portable_id(track.reference_path())?
        };
        let asset_id = upsert_track(
            &tx,
            track,
            &portable,
            existing.get(&normalize(&portable)).copied(),
            options,
            &mut revision,
            now,
        )?;
        let linked: Option<i64> = tx
            .query_row(
                "SELECT id FROM space_asset WHERE asset_id=? AND space_id=?",
                params![asset_id, space],
                |r| r.get(0),
            )
            .optional()?;
        let link = if let Some(link) = linked {
            link
        } else {
            tx.execute(
                "INSERT INTO space_asset(asset_id,space_id) VALUES (?,?)",
                params![asset_id, space],
            )?;
            tx.last_insert_rowid()
        };
        ids.insert(track.id.clone(), link);
    }
    let mut playlist_options = options.clone();
    if source_root.is_some() {
        playlist_options.materialize_smartlists = true;
    }
    let report = if let Some(request) = ownership {
        owned::write_playlists_owned(
            &tx,
            library,
            &playlist_options,
            &ids,
            (root, space),
            (&mut revision, now),
            request,
        )?
    } else {
        write_playlists(
            &tx,
            library,
            &playlist_options,
            &ids,
            (root, space),
            &mut revision,
            now,
        )?;
        crate::ownership::PlaylistOwnershipReport::default()
    };
    tx.execute(
        "UPDATE master SET revision=?,last_sync_time=?",
        params![revision, now],
    )?;
    tx.execute(
        "UPDATE serato SET revision=?,time_last_connected=?",
        params![revision, now],
    )?;
    // Prevent the legacy importer from replacing the new crate hierarchy.
    tx.execute("UPDATE dbv2_status SET autosync_disabled=1", [])?;
    tx.commit()?;
    Ok(report)
}

/// Write Serato tags to an explicit staged audio copy.
pub fn write_media(path: &Path, track: &SyncTrack, options: &SyncOptions) -> Result<()> {
    media::write(path, track, options)
}

fn register_functions(conn: &Connection) -> Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8
        | FunctionFlags::SQLITE_DETERMINISTIC
        | FunctionFlags::SQLITE_INNOCUOUS;
    conn.create_scalar_function("serato_str_norm", 1, flags, |ctx| {
        Ok(ctx.get::<Option<String>>(0)?.map(|s| s.to_lowercase()))
    })?;
    conn.create_scalar_function("serato_raw_key_string_to_key_type", 1, flags, |_| {
        Ok(-1_i64)
    })?;
    Ok(())
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

// Convert Windows extended spelling to Serato's portable path grammar. Keep
// real UNC paths distinct; an extended local disk prefix is not a network share.
fn native_serato_path(path: &Path) -> Result<String> {
    let text = path
        .to_str()
        .context("Serato paths must be Unicode")?
        .replace('\\', "/");
    if text
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("//?/UNC/"))
    {
        return Ok(format!("//{}", &text[8..]));
    }
    if let Some(disk) = text.strip_prefix("//?/") {
        ensure!(
            disk.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                && disk.as_bytes().get(1) == Some(&b':')
                && disk.as_bytes().get(2) == Some(&b'/'),
            "Unsupported Serato Windows namespace"
        );
        return Ok(disk.to_owned());
    }
    ensure!(!text.starts_with("//./"), "Unsupported Serato device path");
    Ok(text)
}
fn same_root_database_location(source: &Path, destination: &Path) -> Result<bool> {
    let source = native_serato_path(source)?;
    let destination = native_serato_path(destination)?;
    let drive = |text: &str| text.as_bytes().get(1) == Some(&b':');
    Ok(match (drive(&source), drive(&destination)) {
        (true, true) => source[..2].eq_ignore_ascii_case(&destination[..2]),
        (false, false) => true,
        _ => false,
    })
}
fn location_portable_id(path: &Path, root: &Path) -> Result<String> {
    let text = native_serato_path(path)?;
    let root = native_serato_path(root)?;
    let prefix = format!("{}/", root.trim_end_matches('/'));
    ensure!(
        text.to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase()),
        "Track is outside the selected Serato location root"
    );
    Ok(text[prefix.len()..].to_owned())
}

fn portable_id(path: &Path) -> Result<String> {
    let text = native_serato_path(path)?;
    ensure!(
        path.is_absolute() || text.as_bytes().get(1) == Some(&b':'),
        "Serato tracks require absolute paths"
    );
    ensure!(
        !text.starts_with("/Volumes/") && !text.starts_with("//"),
        "External and network tracks require their own Serato location database"
    );
    let portable = if text.as_bytes().get(1) == Some(&b':') {
        &text[2..]
    } else {
        &text
    };
    Ok(portable.trim_start_matches('/').to_owned())
}

fn next_revision(revision: &mut i64) -> Result<i64> {
    *revision = revision
        .checked_add(1)
        .context("Serato revision overflow")?;
    Ok(*revision)
}

fn upsert_track(
    conn: &Connection,
    t: &SyncTrack,
    portable: &str,
    id: Option<i64>,
    options: &SyncOptions,
    revision: &mut i64,
    now: i64,
) -> Result<i64> {
    let rev = next_revision(revision)?;
    let locked = match options.beatgrid_lock {
        BeatgridLock::Lock => true,
        BeatgridLock::Unlock => false,
        BeatgridLock::Default => !t.beatgrid.is_empty(),
    };
    let bpm = (t.bpm > 0.).then_some(t.bpm);
    let format = t
        .path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let format = match format.as_str() {
        "aac" | "adts" => "m4a",
        "aif" => "aiff",
        other => other,
    };
    let filename = t
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .context("Invalid audio filename")?;
    let size = std::fs::metadata(&t.path)?.len() as i64;
    if let Some(id) = id {
        conn.execute("UPDATE asset SET revision=?1,file_name=?2,file_size=?3,file_sample_rate=?4,format=?5,artist=?6,name=?7,album=?8,genre=?9,bpm=CASE WHEN ?10 THEN bpm ELSE ?11 END,key=?12,year=?13,label=?14,comments=?15,rating=?16,color=?17,length_sec=?18,length_ms=?19,time_modified=?20,is_beatgrid_locked=?21,analysis_flags=CASE WHEN ?10 THEN analysis_flags ELSE ?22 END WHERE id=?23",
            params![rev,filename,size,t.sample_rate,format,t.artist,t.title,t.album,t.genre,options.skip_beatgrids,bpm,t.key,t.year.map(|v|v.to_string()).unwrap_or_default(),t.label,t.comment,t.rating as f64/5.,t.color,t.duration_seconds.round() as i64,(t.duration_seconds*1000.).round() as i64,now,i64::from(locked),if t.beatgrid.is_empty(){0}else{31},id])?;
        write_saved_attributes(conn, id, t)?;
        Ok(id)
    } else {
        conn.execute("INSERT INTO asset(revision,portable_id,file_name,file_size,file_sample_rate,type,format,artist,name,album,genre,bpm,key,year,label,comments,rating,color,length_sec,length_ms,time_added,time_modified,is_beatgrid_locked,analysis_flags,is_corrupt,is_missing,is_readonly,third_party_type,is_stale,is_whitelabel,is_karaoke,dj_play_count,dj_recently_played,architectures,stems_analyze_state,comment_language,comment_descriptor,grouping,remixer,composer,emoji,part_of_set,track_number,video_portable_id) VALUES (?1,?2,?3,?4,?5,'audio',?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?20,?21,?22,0,0,0,0,0,0,0,0,0,0,0,'','','','','','','','','')",
            params![rev,portable,filename,size,t.sample_rate,format,t.artist,t.title,t.album,t.genre,bpm,t.key,t.year.map(|v|v.to_string()).unwrap_or_default(),t.label,t.comment,t.rating as f64/5.,t.color,t.duration_seconds.round() as i64,(t.duration_seconds*1000.).round() as i64,now,i64::from(locked),if options.skip_beatgrids||!t.beatgrid.is_empty(){31}else{0}])?;
        let id = conn.last_insert_rowid();
        write_saved_attributes(conn, id, t)?;
        Ok(id)
    }
}

fn write_saved_attributes(conn: &Connection, id: i64, track: &SyncTrack) -> Result<()> {
    let text = |field| {
        track.attribute_text(field).or_else(|| {
            matches!(
                track.attributes.get(field),
                Some(cuerithm_smartlists::FieldValue::Null)
            )
            .then_some("")
        })
    };
    let number = track.attribute_number("trackNumber").map(|n| n.to_string());
    let track_number = text("trackNumber").or(number.as_deref());
    let added = track
        .attribute_date("dateAdded")
        .map(|ms| ms.div_euclid(1000));
    conn.execute("UPDATE asset SET composer=COALESCE(?1,composer),remixer=COALESCE(?2,remixer),grouping=COALESCE(?3,grouping),track_number=COALESCE(?4,track_number),time_added=COALESCE(?5,time_added) WHERE id=?6",
        params![text("composer"),text("remixer"),text("grouping"),track_number,added,id])?;
    if let Some(bitrate) = track.attribute_number("bitrate") {
        let present: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('asset') WHERE name='file_bit_rate')",
            [],
            |r| r.get(0),
        )?;
        if present {
            ensure!(
                bitrate >= 0. && bitrate * 1000. <= i64::MAX as f64,
                "Invalid Serato source bitrate"
            );
            conn.execute(
                "UPDATE asset SET file_bit_rate=? WHERE id=?",
                params![(bitrate * 1000.).round() as i64, id],
            )?;
        }
    }
    Ok(())
}

fn container(
    conn: &Connection,
    name: &str,
    parent: i64,
    kind: i64,
    space: i64,
    revision: &mut i64,
    now: i64,
) -> Result<i64> {
    let existing = conn
        .query_row(
            "SELECT id FROM container WHERE parent_id=? AND type=? AND name=? COLLATE NOCASE",
            params![parent, kind, name],
            |r| r.get::<_, i64>(0),
        )
        .optional()?;
    let rev = next_revision(revision)?;
    if let Some(id) = existing {
        conn.execute(
            "UPDATE container SET space_id=?,revision=? WHERE id=?",
            params![space, rev, id],
        )?;
        return Ok(id);
    }
    conn.execute("INSERT INTO container(revision,parent_id,name,type,list_order,space_id,time_added,expanded,portable_id,color) VALUES (?1,?2,?3,?4,(SELECT COALESCE(MAX(list_order),0)+1 FROM container WHERE parent_id=?2),?5,?6,0,'',NULL)",params![rev,parent,name,kind,space,now])?;
    Ok(conn.last_insert_rowid())
}

const SMART_CRATE_DELIMITER: char = '\u{105e8a}';

fn smart_container(
    conn: &Connection,
    playlist: &SyncPlaylist,
    parent: i64,
    smart: bool,
    space: i64,
    revision: &mut i64,
    now: i64,
) -> Result<i64> {
    ensure!(
        !playlist.name.contains(SMART_CRATE_DELIMITER),
        "Serato crate name contains its reserved smart delimiter"
    );
    use sha2::{Digest, Sha256};
    let suffix_hash = format!("{:x}", Sha256::digest(playlist.id.as_bytes()));
    let suffix = format!("{SMART_CRATE_DELIMITER}{}", &suffix_hash[..32]);
    let prefix = format!("{}{SMART_CRATE_DELIMITER}", playlist.name);
    let mut query=conn.prepare("SELECT id,type,parent_id FROM container WHERE (type=2 AND substr(name,-length(?1))=?1) OR (parent_id=?2 AND type IN (1,2) AND (name=?3 COLLATE NOCASE OR substr(name,1,length(?4))=?4 COLLATE NOCASE))")?;
    let existing = query
        .query_map(params![suffix, parent, playlist.name, prefix], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        existing.len() <= 1,
        "Ambiguous ordinary/smart Serato crate names"
    );
    let kind = if smart { 2 } else { 1 };
    let name = if smart {
        format!("{}{suffix}", playlist.name)
    } else {
        playlist.name.clone()
    };
    if let Some((id, old_kind, old_parent)) = existing.first() {
        if smart && *old_kind != 2 {
            let children: i64 = conn.query_row(
                "SELECT COUNT(*) FROM container WHERE parent_id=?1",
                [id],
                |row| row.get(0),
            )?;
            ensure!(
                children == 0,
                "An existing Serato folder cannot become a smart crate"
            );
        }
        let order: Option<i64> = if *old_parent != parent {
            Some(conn.query_row(
                "SELECT COALESCE(MAX(list_order),0)+1 FROM container WHERE parent_id=?1",
                [parent],
                |row| row.get(0),
            )?)
        } else {
            None
        };
        conn.execute(
            "UPDATE container SET name=?1,type=?2,space_id=?3,revision=?4,parent_id=?5,list_order=COALESCE(?6,list_order) WHERE id=?7",
            params![name, kind, space, next_revision(revision)?, parent, order, id],
        )?;
        return Ok(*id);
    }
    container(conn, &name, parent, kind, space, revision, now)
}

fn write_playlists(
    conn: &Connection,
    library: &SyncLibrary,
    options: &SyncOptions,
    tracks: &HashMap<String, i64>,
    destination: (i64, i64),
    revision: &mut i64,
    now: i64,
) -> Result<()> {
    let (root, space) = destination;
    let mut pending: Vec<_> = library.playlists.iter().collect();
    let has_smart_table: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='smart_crate_rules')",
        [], |row| row.get(0))?;
    let mut ids = HashMap::new();
    let mut names = HashSet::new();
    while !pending.is_empty() {
        let before = pending.len();
        let mut later = Vec::new();
        for p in pending {
            let parent = if let Some(parent) = &p.parent_id {
                let Some(id) = ids.get(parent).copied() else {
                    later.push(p);
                    continue;
                };
                id
            } else {
                root
            };
            ensure!(
                names.insert((parent, p.name.to_lowercase())),
                "Two Serato crates have the same name under one parent"
            );
            let native = match crate::smart_rules::compile(
                p,
                library,
                options,
                crate::DjSoftware::Serato4,
            ) {
                Ok(Some(crate::smart_rules::NativeSmart::Serato4(rules))) => Some(rules),
                _ => None,
            };
            ensure!(
                native.is_none() || has_smart_table,
                "Current Serato database has no smart_crate_rules table"
            );
            let id = smart_container(conn, p, parent, native.is_some(), space, revision, now)?;
            ids.insert(p.id.clone(), id);
            write_playlist_contents(conn, p, id, native, has_smart_table, tracks, revision)?;
        }
        if later.len() == before {
            bail!("Playlist hierarchy contains a missing parent or cycle");
        }
        pending = later;
    }
    Ok(())
}

fn write_playlist_contents(
    conn: &Connection,
    p: &SyncPlaylist,
    id: i64,
    native: Option<String>,
    has_smart_table: bool,
    tracks: &HashMap<String, i64>,
    revision: &mut i64,
) -> Result<()> {
    if let Some(rules) = native {
        conn.execute("DELETE FROM container_asset WHERE container_id=?1", [id])?;
        let rev = next_revision(revision)?;
        let updated = conn.execute(
            "UPDATE smart_crate_rules SET revision=?1,version=1,rules=?2 WHERE container_id=?3",
            params![rev, rules, id],
        )?;
        if updated == 0 {
            conn.execute("INSERT INTO smart_crate_rules(container_id,revision,version,rules) VALUES(?1,?2,1,?3)",params![id,rev,rules])?;
        }
        return Ok(());
    }
    if has_smart_table {
        conn.execute("DELETE FROM smart_crate_rules WHERE container_id=?1", [id])?;
    }
    if !p.folder {
        conn.execute("DELETE FROM container_asset WHERE container_id=?", [id])?;
        for (order, track) in p.track_ids.iter().enumerate() {
            let link = tracks
                .get(track)
                .context("Playlist references an unselected track")?;
            conn.execute("INSERT INTO container_asset(container_id,space_asset_id,list_order,revision) VALUES (?,?,?,?)",params![id,link,order as i64+1,next_revision(revision)?])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod native_smart_tests {
    use super::*;
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    #[test]
    fn smart_rule_rows_reuse_container_across_rename_move_and_materialize() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE container(id INTEGER PRIMARY KEY,revision INTEGER,parent_id INTEGER,name TEXT,type INTEGER,list_order INTEGER,space_id INTEGER,time_added INTEGER,expanded INTEGER,portable_id TEXT,color INTEGER);CREATE TABLE container_asset(container_id INTEGER,space_asset_id INTEGER,list_order INTEGER,revision INTEGER);CREATE TABLE smart_crate_rules(container_id INTEGER PRIMARY KEY,revision INTEGER,version INTEGER,rules TEXT);").unwrap();
        let definition = SmartDefinition {
            match_all: true,
            rules: vec![SmartRule {
                id: "artist".into(),
                field: "artist".into(),
                operator: Operator::StringContains,
                value: RuleValue::Text("Fixture".into()),
                or: false,
            }],
        };
        let mut input = SyncLibrary {
            playlists: vec![
                SyncPlaylist {
                    id: "folder".into(),
                    name: "Folder".into(),
                    folder: true,
                    ..Default::default()
                },
                SyncPlaylist {
                    id: "smart".into(),
                    name: "Native".into(),
                    smart: Some(definition),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut revision = 1;
        write_playlists(
            &db,
            &input,
            &SyncOptions::default(),
            &HashMap::new(),
            (0, 1),
            &mut revision,
            1,
        )
        .unwrap();
        let id: i64 = db
            .query_row("SELECT id FROM container WHERE type=2", [], |row| {
                row.get(0)
            })
            .unwrap();
        let rules: String = db
            .query_row(
                "SELECT rules FROM smart_crate_rules WHERE container_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        let rules: serde_json::Value = serde_json::from_str(&rules).unwrap();
        assert_eq!(rules["rules"][0]["attribute"], "artist");
        input.playlists[1].name = "Renamed".into();
        input.playlists[1].parent_id = Some("folder".into());
        write_playlists(
            &db,
            &input,
            &SyncOptions::default(),
            &HashMap::new(),
            (0, 1),
            &mut revision,
            1,
        )
        .unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM container WHERE type=2", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT id FROM container WHERE type=2", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            id
        );
        assert!(
            db.query_row("SELECT name FROM container WHERE id=?1", [id], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap()
            .starts_with("Renamed")
        );
        write_playlists(
            &db,
            &input,
            &SyncOptions {
                materialize_smartlists: true,
                ..Default::default()
            },
            &HashMap::new(),
            (0, 1),
            &mut revision,
            1,
        )
        .unwrap();
        assert_eq!(
            db.query_row("SELECT type FROM container WHERE id=?1", [id], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT name FROM container WHERE id=?1", [id], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
            "Renamed"
        );
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM smart_crate_rules", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        write_playlists(
            &db,
            &input,
            &SyncOptions::default(),
            &HashMap::new(),
            (0, 1),
            &mut revision,
            1,
        )
        .unwrap();
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM container WHERE type=2", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT id FROM container WHERE type=2", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            id
        );
    }
}

fn reference_portable(path: &Path, root: Option<&Path>) -> Result<String> {
    match root {
        Some(root) => location_portable_id(path, root),
        None => portable_id(path),
    }
}
pub(crate) fn reference_id(database: &Path, root: Option<&Path>, source: &Path) -> Result<String> {
    reference_id_in_environment(
        database,
        root,
        source,
        &cuerithm_rekordbox::TargetEnvironment::Native,
    )
}
pub(crate) fn reference_id_in_environment(
    database: &Path,
    physical_root: Option<&Path>,
    physical_source: &Path,
    environment: &cuerithm_rekordbox::TargetEnvironment,
) -> Result<String> {
    let projected_source = crate::destination_reference(&environment.to_native(physical_source)?);
    let projected_root = physical_root
        .map(|root| environment.to_native(root))
        .transpose()?;
    let root = projected_root.as_deref();
    let source = projected_source.as_path();
    let conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    ensure!(
        version > 0,
        "Serato path repair requires an initialized location database"
    );
    let portable = reference_portable(source, root)?;
    let key = normalize(&portable);
    let ids = conn
        .prepare("SELECT id,portable_id FROM asset")?
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, p)| normalize(p) == key)
        .collect::<Vec<_>>();
    ensure!(
        ids.len() == 1,
        "Serato source reference is missing or ambiguous"
    );
    if ids[0].1 != portable {
        let spelling = ids[0].1.replace('\\', "/");
        let actual = if let Some(root) = root {
            root.join(&spelling)
        } else {
            let source = native_serato_path(source)?;
            if source.as_bytes().get(1) == Some(&b':') {
                PathBuf::from(format!("{}/{}", &source[..2], spelling))
            } else {
                PathBuf::from(format!("/{spelling}"))
            }
        };
        ensure!(
            same_file::is_same_file(environment.to_host(&actual)?, physical_source)?,
            "Serato normalized path identifies another physical source"
        );
    }
    Ok(ids[0].0.to_string())
}
pub(crate) fn repair_reference(
    database: &Path,
    root: Option<&Path>,
    source: &Path,
    destination: &Path,
    expected: &str,
) -> Result<()> {
    repair_reference_in_environment(
        database,
        root,
        source,
        destination,
        expected,
        &cuerithm_rekordbox::TargetEnvironment::Native,
    )
}
pub(crate) fn repair_reference_in_environment(
    database: &Path,
    physical_root: Option<&Path>,
    physical_source: &Path,
    physical_destination: &Path,
    expected: &str,
    environment: &cuerithm_rekordbox::TargetEnvironment,
) -> Result<()> {
    ensure!(
        reference_id_in_environment(database, physical_root, physical_source, environment)?
            == expected,
        "Serato native track identity changed"
    );
    let projected_root = physical_root
        .map(|root| environment.to_native(root))
        .transpose()?;
    let projected_source = crate::destination_reference(&environment.to_native(physical_source)?);
    let projected_destination =
        crate::destination_reference(&environment.to_native(physical_destination)?);
    let root = projected_root.as_deref();
    let source = projected_source.as_path();
    let destination = projected_destination.as_path();
    // A root.sqlite portable ID omits the drive. Cross-drive moves need a different
    // location database and cannot be represented by changing this one row.
    if root.is_none() {
        ensure!(
            same_root_database_location(source, destination)?,
            "Serato relocation requires another location database"
        );
    }
    let old = normalize(&reference_portable(source, root)?);
    let new = reference_portable(destination, root)?;
    let mut conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let tx = conn.transaction()?;
    let rows = tx
        .prepare("SELECT id,portable_id FROM asset")?
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        !rows
            .iter()
            .any(|(id, path)| id.to_string() != expected && normalize(path) == normalize(&new)),
        "Serato destination already has a native track"
    );
    let mut revision: i64 =
        tx.query_row("SELECT revision FROM master LIMIT 1", [], |row| row.get(0))?;
    let revision = next_revision(&mut revision)?;
    let destination_text = native_serato_path(destination)?;
    let filename = destination_text
        .rsplit('/')
        .next()
        .context("Serato requires a Unicode filename")?;
    let changed = tx.execute(
        "UPDATE asset SET portable_id=?1,file_name=?2,revision=?3 WHERE id=?4",
        params![new, filename, revision, expected],
    )?;
    ensure!(
        changed == 1
            && rows
                .iter()
                .any(|(id, path)| id.to_string() == expected && normalize(path) == old),
        "Serato source reference changed"
    );
    tx.execute("UPDATE master SET revision=?1", [revision])?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn validate_reference_location(
    source: &Path,
    destination: &Path,
    root: Option<&Path>,
) -> Result<()> {
    reference_portable(source, root)?;
    reference_portable(destination, root)?;
    if root.is_none() {
        ensure!(
            same_root_database_location(source, destination)?,
            "Serato relocation requires another location database"
        );
    }
    Ok(())
}
#[cfg(test)]
mod path_reference_tests {
    use super::*;
    #[test]
    fn extended_windows_disk_paths_are_local_but_unc_paths_keep_their_location() {
        assert_eq!(
            portable_id(Path::new(r"\\?\C:\music\old.wav")).unwrap(),
            "music/old.wav"
        );
        assert_eq!(
            portable_id(Path::new(r"C:\music\old.wav")).unwrap(),
            "music/old.wav"
        );
        assert!(portable_id(Path::new(r"\\?\UNC\server\share\old.wav")).is_err());
        assert_eq!(
            location_portable_id(
                Path::new(r"\\?\UNC\server\share\music\old.wav"),
                Path::new(r"\\server\share")
            )
            .unwrap(),
            "music/old.wav"
        );
    }
    #[test]
    fn root_database_cannot_follow_audio_to_an_external_location() {
        assert!(
            validate_reference_location(
                Path::new("/Users/fixture/a.wav"),
                Path::new("/Volumes/USB/a.wav"),
                None
            )
            .is_err()
        );
        assert!(
            validate_reference_location(
                Path::new("C:/Music/a.wav"),
                Path::new("D:/Music/a.wav"),
                None
            )
            .is_err()
        );
        assert!(
            validate_reference_location(
                Path::new("/Volumes/A/a.wav"),
                Path::new("/Volumes/B/a.wav"),
                Some(Path::new("/Volumes/A"))
            )
            .is_err()
        );
    }
}

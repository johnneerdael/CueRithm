//! Current djay Pro `MediaLibrary.db` adapter.
//!
//! Writes the native YapDatabase `database2` collections and TSAF object blobs.
//! The destination must be an existing database created by djay Pro. Passing its
//! containing directory is also supported. Index versions are invalidated inside
//! the same transaction so djay rebuilds its derived views on its next launch.
mod bookmark;
#[cfg(test)]
mod bookmark_tests;
#[path = "djay_pro/owned.rs"]
mod owned;
use crate::{SyncBeat, SyncLibrary, SyncOptions, SyncTrack};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const ROOT: &str = "mediaItemPlaylist-root";
const PLAYLISTS: &str = "mediaItemPlaylists";
const LINKS: &str = "mediaItemPlaylistItems";
const GRID: &str = "beatGridEdits";
const INDEXES: &[&str] = &[
    "mediaView",
    "mediaItemsView",
    "mediaItemOriginalSourceView",
    "mediaItemIndex",
    "mediaItemUserDataIndex",
    "mediaItemTitleIDSearchIndex",
    "mediaItemAnalyzedDataIndex",
    "mediaItemKeySignatureIndexView",
    "searchIndex",
    "relationship",
    "mediaItemPlaylistsView",
    "mediaItemPlaylistView",
    "mediaItemPlaylistIndex",
];

/// Sync into an existing current djay Pro database, or the directory containing it.
/// The caller must stop djay and back up the database (including SQLite sidecars).
pub fn write(path: &Path, library: &SyncLibrary, options: &SyncOptions) -> Result<()> {
    write_owned(
        path,
        library,
        options,
        &crate::ownership::PlaylistOwnershipRequest::default(),
    )
    .map(|_| ())
}

/// Update native records using acknowledged source-to-UUID evidence. The caller
/// supplies explicit removals; omission from this scope never authorizes deletion.
pub fn write_owned(
    path: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
    request: &crate::ownership::PlaylistOwnershipRequest,
) -> Result<crate::ownership::PlaylistOwnershipReport> {
    let path = database_path(path);
    ensure!(
        path.is_file(),
        "djay Pro MediaLibrary.db does not exist: {}",
        path.display()
    );
    validate(library)?;
    let mut connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .context("open current djay Pro library")?;
    connection.busy_timeout(std::time::Duration::from_secs(3))?;
    // Deliberately do not fabricate a YapDatabase schema or accept an old plist database.
    connection
        .prepare("SELECT rowid, collection, key, data, metadata FROM database2 LIMIT 0")
        .context("destination is not a current djay Pro database")?;
    connection
        .prepare("SELECT extension, key, data FROM yap2 LIMIT 0")
        .context("destination has no current YapDatabase index registry")?;
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let ownership = owned::Plan::prepare(&tx, library, request)?;
    let mut reviewed_uris = HashSet::new();
    for track in &library.tracks {
        ensure!(
            reviewed_uris.insert(uri_identity(&file_uri(&track.path)?)),
            "Duplicate selected djay source URI identity"
        );
    }
    let mut source_ids = HashMap::new();
    let mut bookmark_ids = HashMap::new();
    for (id, object) in objects(&tx, "localMediaItemLocations")? {
        if let Some(Value::Array(values)) = object.get("sourceURIs") {
            for value in values {
                if let Value::Uri(uri) | Value::String(uri) = value
                    && reviewed_uris.contains(&uri_identity(uri))
                {
                    ensure!(
                        source_ids
                            .insert(uri_identity(uri), id.clone())
                            .is_none_or(|previous| previous == id),
                        "Ambiguous djay source URI identity"
                    );
                }
            }
        }
        if let Some(Value::Binary(bytes)) = object.get("urlBookmarkData") {
            // Parse stored paths only; preserve unknown opaque bookmark bytes.
            // This never resolves OS aliases or searches by filename.
            match bookmark::stored_path(bytes) {
                Ok(Some(path)) => {
                    let uri = uri_identity(&file_uri(&path)?);
                    if reviewed_uris.contains(&uri) {
                        ensure!(
                            bookmark_ids
                                .insert(uri, id.clone())
                                .is_none_or(|previous| previous == id),
                            "Ambiguous djay bookmark identity"
                        );
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(error.context("Cannot establish djay bookmark identity safely"));
                }
            }
        }
    }
    let mut ids = HashMap::new();
    let mut entities = HashMap::new();
    for (collection, _, _) in ENTITY_TYPES {
        for (id, object) in objects(&tx, collection)? {
            if let Some(Value::String(name)) = object.get("name") {
                entities.insert((collection.to_string(), name.clone()), id);
            }
        }
    }
    let mut resolved_ids = HashSet::new();
    for track in &library.tracks {
        let uri = file_uri(&track.path)?;
        let identity = uri_identity(&uri);
        let primary = source_ids.get(&identity);
        let stored = bookmark_ids.get(&identity);
        ensure!(
            primary.zip(stored).is_none_or(|(a, b)| a == b),
            "Conflicting djay URI and bookmark identities"
        );
        let id = primary
            .or(stored)
            .cloned()
            .unwrap_or_else(|| stable_id("track", &track.id, false));
        ensure!(
            resolved_ids.insert(id.clone()),
            "Multiple selected djay sources resolve to one native track"
        );
        source_ids.insert(identity, id.clone());
        ids.insert(track.id.clone(), id);
    }
    // Resolve every selected identity before any native track is changed.
    for track in &library.tracks {
        let uri = file_uri(&track.path)?;
        let entity_ids = entity_ids(&tx, track, &mut entities)?;
        write_track(&tx, &ids[&track.id], track, &uri, &entity_ids, options)?;
    }
    write_playlists(&tx, library, &ids, options)?;
    let removed = ownership.remove(&tx)?;
    let report = ownership.report(&tx, library)?;
    if removed || !library.tracks.is_empty() || !library.playlists.is_empty() {
        for extension in INDEXES {
            tx.execute(
                "UPDATE yap2 SET data=0 WHERE extension=?1 AND key='versionTag'",
                [extension],
            )?;
        }
    }
    tx.commit().context("commit djay Pro sync")?;
    Ok(report)
}

fn database_path(path: &Path) -> PathBuf {
    if path
        .file_name()
        .is_some_and(|name| name == "MediaLibrary.db")
    {
        path.to_owned()
    } else {
        path.join("MediaLibrary.db")
    }
}
fn validate(library: &SyncLibrary) -> Result<()> {
    let mut ids = HashSet::new();
    for track in &library.tracks {
        ensure!(
            !track.id.is_empty() && ids.insert(&track.id),
            "duplicate or empty track id"
        );
        ensure!(track.bpm.is_finite() && track.bpm >= 0.0, "invalid BPM");
        ensure!(
            track.duration_seconds.is_finite() && track.duration_seconds >= 0.0,
            "invalid duration"
        );
        ensure!(
            track.rating <= 5,
            "djay ratings must be between zero and five"
        );
        let mut cue_ids = HashSet::new();
        for cue in &track.cues {
            ensure!(cue_ids.insert(cue.index), "duplicate djay cue number");
            ensure!(
                cue.index < 8,
                "djay Pro supports eight numbered cue/loop slots"
            );
            ensure!(
                cue.start_seconds.is_finite()
                    && (cue.start_seconds >= 0.0
                        || track
                            .native_timeline
                            .as_ref()
                            .is_some_and(|t| t.target == crate::DjSoftware::DjayPro)),
                "invalid cue time"
            );
            if let Some(end) = cue.end_seconds {
                ensure!(
                    end.is_finite() && end > cue.start_seconds,
                    "invalid loop end"
                );
            }
        }
        let mut previous = None;
        for beat in &track.beatgrid {
            ensure!(
                beat.seconds.is_finite()
                    && (beat.seconds >= 0.0
                        || track
                            .native_timeline
                            .as_ref()
                            .is_some_and(|t| t.target == crate::DjSoftware::DjayPro))
                    && beat.bpm.is_finite()
                    && beat.bpm > 0.0,
                "invalid beatgrid marker"
            );
            ensure!(
                previous.is_none_or(|time| beat.seconds > time),
                "beatgrid markers must increase"
            );
            previous = Some(beat.seconds);
        }
        file_uri(&track.path)?;
    }
    let mut playlists = HashMap::new();
    for playlist in &library.playlists {
        ensure!(
            !playlist.id.is_empty() && playlists.insert(&playlist.id, playlist).is_none(),
            "duplicate or empty playlist id"
        );
        ensure!(
            playlist.folder || playlist.track_ids.iter().all(|id| ids.contains(id)),
            "playlist refers to an unknown track"
        );
        ensure!(
            !playlist.folder || playlist.track_ids.is_empty(),
            "playlist folders cannot contain tracks"
        );
    }
    for playlist in &library.playlists {
        let mut visited = HashSet::new();
        visited.insert(&playlist.id);
        let mut parent = playlist.parent_id.as_ref();
        while let Some(id) = parent {
            ensure!(visited.insert(id), "playlist hierarchy contains a cycle");
            let ancestor = playlists.get(id).context("unknown playlist parent")?;
            ensure!(ancestor.folder, "playlist parent is not a folder");
            parent = ancestor.parent_id.as_ref();
        }
    }
    Ok(())
}

const ENTITY_TYPES: [(&str, &str, &str); 4] = [
    ("mediaArtists", "ADCMediaArtist", "artistUUIDs"),
    ("mediaAlbums", "ADCMediaAlbum", "albumUUID"),
    ("mediaGenres", "ADCMediaGenre", "genreUUIDs"),
    ("mediaLabels", "ADCMediaLabel", "labelUUID"),
];
fn entity_ids(
    tx: &Transaction<'_>,
    track: &SyncTrack,
    known: &mut HashMap<(String, String), String>,
) -> Result<Vec<(String, Value)>> {
    let mut result = Vec::new();
    for ((collection, class, field), name) in
        ENTITY_TYPES
            .into_iter()
            .zip([&track.artist, &track.album, &track.genre, &track.label])
    {
        if name.is_empty() {
            continue;
        }
        let map_key = (collection.to_owned(), name.clone());
        let id = if let Some(id) = known.get(&map_key) {
            id.clone()
        } else {
            let id = stable_id(collection, name, false);
            let mut entity = Object::new(class);
            entity.set("uuid", text(&id));
            entity.set("name", text(name));
            upsert(tx, collection, &id, &entity)?;
            known.insert(map_key, id.clone());
            id
        };
        let value = if field.ends_with('s') {
            Value::Array(vec![text(&id)])
        } else {
            text(&id)
        };
        result.push((field.to_owned(), value));
    }
    Ok(result)
}
fn title_id(id: &str, track: &SyncTrack) -> Object {
    let mut value = Object::new("ADCMediaItemTitleID");
    value.set("uuid", text(id));
    value.set("title", text(&track.title));
    value.set("artist", text(&track.artist));
    value.set("duration", Value::Double(track.duration_seconds));
    value
}
fn title_ids(object: &mut Object, id: &str, track: &SyncTrack) {
    object.set("uuid", text(id));
    object.set(
        "titleIDs",
        Value::Array(vec![Value::Class(title_id(id, track))]),
    );
}
fn write_track(
    tx: &Transaction<'_>,
    id: &str,
    track: &SyncTrack,
    uri: &str,
    entity_ids: &[(String, Value)],
    options: &SyncOptions,
) -> Result<()> {
    let mut item = existing(tx, "mediaItems", id, "ADCMediaItem")?;
    item.set("uuid", text(id));
    item.set("titleID", Value::Class(title_id(id, track)));
    item.set("title", text(&track.title));
    for field in ["composer", "grouping"] {
        if let Some(value) = track.attribute_text(field) {
            item.set(field, text(value));
        } else if track.attributes.contains_key(field) {
            item.remove(field);
        }
    }
    if let Some(date) = track.attribute_date("dateAdded") {
        item.set("addedDate", Value::Date(date as f64 / 1000.0 - 978307200.0));
    } else if track.attributes.contains_key("dateAdded") {
        item.remove("addedDate");
    }
    item.set("comments", text(&track.comment));
    item.set("originSourceID", text("file"));
    item.set("contentType", text(content_type(&track.path)));
    item.set("bpm", Value::Double(track.bpm));
    item.set("duration", Value::Double(track.duration_seconds));
    item.set("sampleRate", Value::U32(track.sample_rate));
    for (source, native) in [("bitrate", "bitRate"), ("trackNumber", "albumTrackNumber")] {
        match track.attributes.get(source) {
            None | Some(cuerithm_smartlists::FieldValue::Null) => {}
            Some(cuerithm_smartlists::FieldValue::Number(value)) => {
                ensure!(
                    value.is_finite()
                        && *value >= 0.
                        && *value <= f64::from(u16::MAX)
                        && (source == "bitrate" || value.fract() == 0.),
                    "Invalid djay source {source}"
                );
                // Native UInt16 bitrate truncates fractional kilobits/second.
                item.set(native, Value::U16(value.trunc() as u16));
            }
            Some(_) => bail!("Invalid djay source {source}"),
        }
    }
    if let Some(year) = track.year {
        item.set("year", Value::U16(year));
    } else {
        item.remove("year");
    }
    for (_, _, field) in ENTITY_TYPES {
        item.remove(field);
    }
    for (field, value) in entity_ids {
        item.set(field, value.clone());
    }
    upsert(tx, "mediaItems", id, &item)?;
    upsert(tx, "mediaItemTitleIDs", id, &title_id(id, track))?;

    let mut user = existing(tx, "mediaItemUserData", id, "ADCMediaItemUserData")?;
    title_ids(&mut user, id, track);
    user.set("rating", Value::U16(track.rating.into()));
    if let Some(color) = track.color {
        user.set("colorIndex", Value::U16(color_index(color).into()));
    } else {
        user.remove("colorIndex");
    }
    if let Some(key) = key_index(&track.key) {
        user.set("manualKeySignatureIndex2", Value::U16(key));
    } else {
        user.remove("manualKeySignatureIndex2");
    }
    let cues = track
        .cues
        .iter()
        .map(|cue| {
            let mut value = Object::new("ADCCuePoint");
            value.set("time", Value::Double(cue.start_seconds));
            value.set("endTime", Value::Double(cue.end_seconds.unwrap_or(-1.0)));
            value.set("comment", text(&cue.name));
            value.set("number", Value::Double(f64::from(cue.index) + 1.0));
            if let Some(index) = cue
                .color
                .and_then(|color| cue_color(color, options.nearest_color))
            {
                value.set("colorIndex", Value::Double(f64::from(index)));
                if let Some((_, hex)) = PALETTE.get(index as usize) {
                    value.set("color", text(hex));
                }
            }
            Value::Class(value)
        })
        .collect();
    // Current djay stores saved loops as cue points with a positive endTime.
    user.set("cuePoints", Value::Array(cues));
    user.remove("loopRegions");
    if !options.skip_beatgrids {
        user.remove(GRID);
        user.remove("manualBPM");
        if !track.beatgrid.is_empty() {
            let (bpm, grid) = beatgrid(&track.beatgrid)?;
            user.set("manualBPM", Value::Double(bpm));
            user.set(GRID, Value::Class(grid));
        }
    }
    upsert(tx, "mediaItemUserData", id, &user)?;

    let mut analysis = existing(tx, "mediaItemAnalyzedData", id, "ADCMediaItemAnalyzedData")?;
    title_ids(&mut analysis, id, track);
    if !options.skip_beatgrids {
        analysis.set(
            "bpm",
            Value::Double(track.beatgrid.first().map_or(track.bpm, |beat| beat.bpm)),
        );
        analysis.set("isStraightGrid", Value::U16(1));
    }
    if let Some(key) = key_index(&track.key) {
        analysis.set("keySignatureIndex", Value::U16(key));
    } else {
        analysis.remove("keySignatureIndex");
    }
    upsert(tx, "mediaItemAnalyzedData", id, &analysis)?;

    let mut location = existing(tx, "localMediaItemLocations", id, "ADCMediaItemLocation")?;
    title_ids(&mut location, id, track);
    let represented = matches!(location.get("sourceURIs"), Some(Value::Array(previous))
        if previous.iter().any(|value| matches!(value, Value::Uri(value) | Value::String(value)
            if uri_identity(value) == uri_identity(uri))));
    if !represented {
        let mut uris = vec![Value::Uri(uri.to_owned())];
        if let Some(Value::Array(previous)) = location.get("sourceURIs") {
            for value in previous.iter().skip(1) {
                if !uris.contains(value) {
                    uris.push(value.clone());
                }
            }
        }
        location.set("sourceURIs", Value::Array(uris));
    }
    upsert(tx, "localMediaItemLocations", id, &location)?;
    // Current macOS libraries retain this filename index across the normal
    // version-tag resets. Populate it alongside its local-location record.
    let has_location_index: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='secondaryIndex_mediaItemLocationIndex')",
        [],
        |row| row.get(0),
    )?;
    if has_location_index {
        let rowid: i64 = tx.query_row(
            "SELECT rowid FROM database2 WHERE collection='localMediaItemLocations' AND key=?1",
            [id],
            |row| row.get(0),
        )?;
        let filename = source_filename(&track.path)?;
        tx.execute(
            "INSERT INTO secondaryIndex_mediaItemLocationIndex(rowid,fileName) VALUES(?1,?2) ON CONFLICT(rowid) DO UPDATE SET fileName=excluded.fileName",
            params![rowid, filename],
        )?;
    }
    Ok(())
}

fn beatgrid(beats: &[SyncBeat]) -> Result<(f64, Object)> {
    // Equal-tempo anchors can reset phase and must keep their positions.
    let base = beats[0].bpm;
    let markers = beats;
    let mut grid = Object::new("ADCBeatGridEdits");
    grid.set("nrOfBeatShift", Value::Double(0.0));
    grid.set("fractionalBeatShift", Value::Float(0.0));
    grid.set("firstGridSegmentTempoExponent", Value::Small(0));
    grid.set("lastGridSegmentTempoExponent", Value::Small(0));
    grid.set("useStraightBeats", Value::Double(1.0));
    if markers.len() == 1 {
        grid.set("firstDownbeatPosition", Value::Double(markers[0].seconds));
    } else {
        let mut downbeats = Vec::new();
        for (index, beat) in markers.iter().enumerate() {
            let count = if let Some(next) = markers.get(index + 1) {
                if (beat.bpm - base).abs() < 1e-7 {
                    255
                } else {
                    let bars = (next.seconds - beat.seconds) * beat.bpm / 240.0;
                    ensure!(
                        (bars - bars.round()).abs() < 0.001
                            && (1.0..=254.0).contains(&bars.round()),
                        "djay tempo changes must delimit a whole number of 1-254 bars"
                    );
                    bars.round() as u8
                }
            } else {
                let exponent = (beat.bpm / base).log2();
                ensure!(
                    (exponent - exponent.round()).abs() < 1e-7 && (0.0..=255.0).contains(&exponent),
                    "final djay tempo must equal the initial tempo or a whole positive power of two"
                );
                grid.set("lastGridSegmentTempoExponent", Value::Small(exponent as u8));
                255
            };
            if beat.seconds != 0.0 {
                downbeats.push(Value::Fields(vec![
                    ("position".to_owned(), Value::Float(beat.seconds as f32)),
                    ("numberOfBars".to_owned(), Value::Small(count)),
                ]));
            }
        }
        grid.set("downbeatMarkers", Value::Array(downbeats));
    }
    Ok((base, grid))
}

fn write_playlists(
    tx: &Transaction<'_>,
    library: &SyncLibrary,
    track_ids: &HashMap<String, String>,
    options: &SyncOptions,
) -> Result<()> {
    let ids: HashMap<_, _> = library
        .playlists
        .iter()
        .map(|playlist| {
            (
                playlist.id.clone(),
                stable_id("playlist", &playlist.id, true),
            )
        })
        .collect();
    let parents: HashMap<_, _> = library
        .playlists
        .iter()
        .map(|playlist| {
            (
                ids[&playlist.id].clone(),
                playlist
                    .parent_id
                    .as_ref()
                    .map_or_else(|| ROOT.to_owned(), |id| ids[id].clone()),
            )
        })
        .collect();
    // A partial scope can move a playlist without including its former parent.
    // Remove stale references there while preserving every unrelated child.
    for (id, mut object) in objects(tx, PLAYLISTS)? {
        if let Some(Value::Array(children)) = object.get("childUUIDs") {
            let mut kept = children.clone();
            kept.retain(|child| {
                !matches!(child, Value::String(child)
                if parents.get(child).is_some_and(|parent| parent != &id))
            });
            if kept != *children {
                object.set("childUUIDs", Value::Array(kept));
                upsert(tx, PLAYLISTS, &id, &object)?;
            }
        }
    }
    for playlist in &library.playlists {
        let id = &ids[&playlist.id];
        let mut object = existing(tx, PLAYLISTS, id, "ADCMediaItemPlaylist")?;
        if let Some(Value::Array(previous)) = object.get("itemUUIDs") {
            for entry in previous {
                if let Value::String(link) = entry {
                    tx.execute(
                        "DELETE FROM database2 WHERE collection=?1 AND key=?2",
                        params![LINKS, link],
                    )?;
                }
            }
        }
        object.set("uuid", text(id));
        object.set("name", text(&playlist.name));
        object.set("type", Value::Small(u8::from(playlist.folder)));
        object.set(
            "parentUUID",
            text(
                playlist
                    .parent_id
                    .as_ref()
                    .map_or(ROOT, |parent| &ids[parent]),
            ),
        );
        object.remove("filterSet");
        if let Ok(Some(crate::smart_rules::NativeSmart::Djay(filters))) =
            crate::smart_rules::compile(playlist, library, options, crate::DjSoftware::DjayPro)
        {
            object.set("type", Value::Small(2));
            object.remove("itemUUIDs");
            object.remove("childUUIDs");
            let mut set = Object::new("ADCMediaItemFilterSet");
            set.set("uuid", text(&stable_id("smart-filter", &playlist.id, true)));
            set.set(
                "filters",
                Value::Array(filters.into_iter().map(native_filter).collect()),
            );
            object.set("filterSet", Value::Class(set));
            upsert(tx, PLAYLISTS, id, &object)?;
            continue;
        }
        let mut items = Vec::new();
        if playlist.folder {
            let children = library
                .playlists
                .iter()
                .filter(|child| child.parent_id.as_ref() == Some(&playlist.id))
                .map(|child| text(&ids[&child.id]))
                .collect();
            let children = merge_children(object.get("childUUIDs"), children);
            object.set("childUUIDs", Value::Array(children));
        } else {
            object.remove("childUUIDs");
            for (index, track) in playlist.track_ids.iter().enumerate() {
                let link_id = stable_id("playlist-item", &format!("{}:{index}", playlist.id), true);
                let mut link = Object::new("ADCMediaItemPlaylistItem");
                link.set("uuid", text(&link_id));
                link.set("playlistUUID", text(id));
                link.set("mediaItemUUID", text(&track_ids[track]));
                upsert(tx, LINKS, &link_id, &link)?;
                items.push(text(&link_id));
            }
        }
        object.set("itemUUIDs", Value::Array(items));
        upsert(tx, PLAYLISTS, id, &object)?;
    }
    if !library.playlists.is_empty() {
        let mut root = existing(tx, PLAYLISTS, ROOT, "ADCMediaItemPlaylist")?;
        root.set("uuid", text(ROOT));
        if root.get("name").is_none() {
            root.set("name", text("Playlists"));
        }
        root.set("type", Value::Small(1));
        let children = library
            .playlists
            .iter()
            .filter(|playlist| playlist.parent_id.is_none())
            .map(|playlist| text(&ids[&playlist.id]))
            .collect();
        let children = merge_children(root.get("childUUIDs"), children);
        root.set("childUUIDs", Value::Array(children));
        if root.get("itemUUIDs").is_none() {
            root.set("itemUUIDs", Value::Array(Vec::new()));
        }
        upsert(tx, PLAYLISTS, ROOT, &root)?;
    }
    Ok(())
}
fn native_filter(filter: crate::smart_rules::DjayFilter) -> Value {
    use crate::smart_rules::DjayValue;
    let class = match &filter.value {
        DjayValue::Text(_) => "ADCMediaItemStringFilter",
        DjayValue::Number { .. } => "ADCMediaItemNumberFilter",
        DjayValue::Date { .. } => "ADCMediaItemDateFilter",
        DjayValue::Set(_) => "ADCMediaItemSetFilter",
    };
    let mut object = Object::new(class);
    object.set("name", text(filter.name));
    object.set("keyPath", text(filter.field));
    object.set(
        "keyPathContainsMultipleValues",
        Value::Bool(filter.multiple),
    );
    object.set("enabled", Value::Bool(true));
    match filter.value {
        DjayValue::Text(value) => {
            object.set("operatorType", Value::U16(filter.operator));
            object.set("value", text(&value));
        }
        DjayValue::Number { start, end } => {
            object.set("operatorType", Value::U16(filter.operator));
            object.set("startValue", Value::Double(start));
            if let Some(end) = end {
                object.set("endValue", Value::Double(end));
            }
        }
        DjayValue::Date {
            start,
            end,
            relative,
        } => {
            object.set("operatorType", Value::U16(filter.operator));
            if let Some(start) = start {
                object.set("startDate", Value::Date(start));
            }
            if let Some(end) = end {
                object.set("endDate", Value::Date(end));
            }
            if let Some((count, period)) = relative {
                object.set("periodValue", Value::Double(count as f64));
                object.set("period", Value::Small(period));
            }
        }
        DjayValue::Set(values) => {
            object.set(
                "components",
                Value::Array(
                    values
                        .into_iter()
                        .map(|value| {
                            let mut component = Object::new("ADCMediaItemSetFilterComponent");
                            component.set("value", text(&value));
                            component.set("operatorType", Value::Small(0));
                            Value::Class(component)
                        })
                        .collect(),
                ),
            );
        }
    }
    Value::Class(object)
}

// Replace the selected child slots in order, retaining siblings omitted by a
// playlist/Changed scope. Append newly created children after existing siblings.
fn merge_children(previous: Option<&Value>, desired: Vec<Value>) -> Vec<Value> {
    let Some(Value::Array(previous)) = previous else {
        return desired;
    };
    let mut next = desired.iter();
    let mut result = Vec::new();
    for child in previous {
        if desired.contains(child) {
            if let Some(replacement) = next.next() {
                result.push(replacement.clone());
            }
        } else {
            result.push(child.clone());
        }
    }
    result.extend(next.cloned());
    result
}
fn stable_id(namespace: &str, value: &str, hyphens: bool) -> String {
    let bytes = Sha256::digest(format!("cuerithm:djay:{namespace}:{value}"));
    let mut uuid = [0; 16];
    uuid.copy_from_slice(&bytes[..16]);
    uuid[6] = (uuid[6] & 15) | 0x50;
    uuid[8] = (uuid[8] & 63) | 0x80;
    let uuid = uuid::Uuid::from_bytes(uuid);
    if hyphens {
        uuid.hyphenated().to_string().to_uppercase()
    } else {
        uuid.simple().to_string()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum UriIdentity {
    Windows(String),
    Literal(String),
}
fn windows_drive(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && matches!(bytes.get(2), Some(b'/' | b'\\'))
}
fn windows_path(path: &str) -> bool {
    windows_drive(path) || path.starts_with("\\\\") || path.starts_with("//")
}
fn source_filename(path: &Path) -> Result<&str> {
    let path = path.to_str().context("djay requires a Unicode filename")?;
    let name = if windows_path(path) {
        path.rsplit(['/', '\\']).next()
    } else {
        path.rsplit('/').next()
    };
    name.filter(|name| !name.is_empty())
        .context("djay source path has no filename")
}
fn percent_decoded(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        let decoded = if byte == b'%' {
            let high = char::from(input.next()?).to_digit(16)?;
            let low = char::from(input.next()?).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        };
        if decoded == 0 {
            return None;
        }
        bytes.push(decoded);
    }
    String::from_utf8(bytes).ok()
}
fn windows_uri_identity(uri: &str) -> Option<String> {
    if !uri.get(..7)?.eq_ignore_ascii_case("file://") {
        return None;
    }
    let raw = &uri[7..];
    if raw.contains(['?', '#']) {
        return None;
    }
    let decoded = percent_decoded(raw)?;
    let normalized = if raw.starts_with('/') {
        let local = decoded.strip_prefix('/')?;
        if !windows_drive(local) && !local.starts_with("\\\\") && !local.starts_with("//") {
            return None;
        }
        local.replace('\\', "/")
    } else {
        let (authority, _) = raw.split_once('/')?;
        if authority.is_empty() || authority.contains(['@', ':', '\\', '%']) {
            return None;
        }
        format!("//{}", decoded.replace('\\', "/"))
    };
    if normalized.split('/').any(|part| matches!(part, "." | "..")) {
        return None;
    }
    if windows_drive(&normalized) {
        return Some(format!(
            "{}{}",
            normalized[..1].to_ascii_uppercase(),
            &normalized[1..]
        ));
    }
    let unc = normalized.strip_prefix("//")?;
    let (host, path) = unc.split_once('/')?;
    let share = path.split('/').next()?;
    if host.is_empty()
        || host.contains(['@', ':', '%', '?', '#', '*', '"', '<', '>', '|'])
        || share.is_empty()
        || share.contains(':')
    {
        return None;
    }
    Some(format!("//{}/{path}", host.to_ascii_lowercase()))
}
fn uri_identity(uri: &str) -> UriIdentity {
    windows_uri_identity(uri)
        .map(UriIdentity::Windows)
        .unwrap_or_else(|| UriIdentity::Literal(uri.to_owned()))
}
fn file_uri(path: &Path) -> Result<String> {
    let path = path.to_str().context("djay requires a UTF-8 path")?;
    ensure!(!path.contains('\0'), "NUL in djay source path");
    let normalized = if windows_path(path) {
        path.replace('\\', "/")
    } else {
        path.to_owned()
    };
    ensure!(
        !normalized.starts_with("//./"),
        "djay cannot export Windows device namespace paths"
    );
    // Rust canonicalization adds the verbatim prefix on Windows. File URLs
    // address the same drive or UNC share without that filesystem-only prefix.
    let normalized = if let Some(unc) = normalized.strip_prefix("//?/UNC/") {
        format!("//{unc}")
    } else if let Some(drive) = normalized.strip_prefix("//?/") {
        drive.to_owned()
    } else {
        normalized
    };
    let bytes = normalized.as_bytes();
    let windows = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && bytes.get(2) == Some(&b'/');
    ensure!(
        normalized.starts_with('/') || windows,
        "djay requires absolute source paths"
    );
    let mut uri = if windows {
        "file:///".to_owned()
    } else if normalized.starts_with("//") {
        "file:".to_owned()
    } else {
        "file://".to_owned()
    };
    for byte in normalized.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'():/&".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    Ok(uri)
}
fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mp3" => "public.mp3",
        "mp4" => "com.apple.m4v-video",
        "m4a" => "com.apple.m4a-audio",
        "flac" => "org.xiph.flac",
        "wav" => "com.microsoft.waveform-audio",
        _ => "public.data",
    }
}
const PALETTE: [(u32, &str); 8] = [
    (0xcc0000, "#CC0000"),
    (0xcc8800, "#CC8800"),
    (0x0000cc, "#0000CC"),
    (0xcccc00, "#CCCC00"),
    (0x00cc00, "#00CC00"),
    (0xcc00cc, "#CC00CC"),
    (0x00cccc, "#00CCCC"),
    (0x8800cc, "#8800CC"),
];
// The first eight slots also have the explicit legacy hex spelling used by
// current djay; slots 8-15 use their native colorIndex alone.
const CUE_COLORS: [u32; 16] = [
    0xe60e0e, 0xe88915, 0x0028ab, 0xf5d000, 0x7fe80e, 0xaa00bd, 0x099486, 0xaa3bff, 0x2fa8ed,
    0xc68cf3, 0x12de78, 0x12deca, 0xf5f50c, 0xef590f, 0xde105c, 0xe60ede,
];
fn cue_color(color: u32, nearest: bool) -> Option<u8> {
    if color == 0x1058de {
        return Some(2);
    }
    if color == 0x9e1006 {
        return Some(0);
    }
    if nearest {
        match color {
            0xf2665c => return Some(0),
            0xffe194 => return Some(12),
            0xbae815 | 0xadf55f | 0x4b8c06 => return Some(4),
            0x8400ff => return Some(7),
            _ => {}
        }
    }
    if let Some(index) = PALETTE.iter().position(|(rgb, _)| *rgb == color) {
        return Some(index as u8);
    }
    if !nearest {
        return CUE_COLORS
            .iter()
            .position(|rgb| *rgb == color)
            .map(|index| index as u8);
    }
    CUE_COLORS
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| {
            [0, 8, 16]
                .iter()
                .map(|shift| {
                    let difference =
                        ((color >> shift) & 255) as i32 - ((**candidate >> shift) & 255) as i32;
                    difference * difference
                })
                .sum::<i32>()
        })
        .map(|(index, _)| index as u8)
}
fn color_index(color: u32) -> u8 {
    PALETTE
        .iter()
        .enumerate()
        .min_by_key(|(_, (candidate, _))| {
            [0, 8, 16]
                .iter()
                .map(|shift| {
                    let difference =
                        ((color >> shift) & 255) as i32 - ((candidate >> shift) & 255) as i32;
                    difference * difference
                })
                .sum::<i32>()
        })
        .map_or(0, |(index, _)| index as u8)
}
fn key_index(key: &str) -> Option<u16> {
    let key = key
        .trim()
        .replace('♯', "#")
        .replace('♭', "b")
        .to_lowercase();
    let camelot_minor = [23, 13, 3, 17, 7, 21, 11, 1, 15, 5, 19, 9];
    let camelot_major = [22, 12, 2, 16, 6, 20, 10, 0, 14, 4, 18, 8];
    if key.ends_with('a') || key.ends_with('b') {
        let index = key[..key.len() - 1].parse::<usize>().ok()?.checked_sub(1)?;
        return if key.ends_with('a') {
            camelot_minor.get(index)
        } else {
            camelot_major.get(index)
        }
        .copied();
    }
    if (key.ends_with('m') || key.ends_with('d'))
        && let Ok(open) = key[..key.len() - 1].parse::<usize>()
    {
        let open_major = [0, 14, 4, 18, 8, 22, 12, 2, 16, 6, 20, 10];
        return open_major
            .get(open.checked_sub(1)?)
            .map(|index| index + u16::from(key.ends_with('m')));
    }
    let minor = key.ends_with('m') || key.ends_with("min") || key.ends_with("minor");
    let tonic = key
        .trim_end_matches("minor")
        .trim_end_matches("min")
        .trim_end_matches("major")
        .trim_end_matches("maj")
        .trim_end_matches('m')
        .trim();
    let pitch: u16 = match tonic {
        "c" => 0,
        "c#" | "db" => 1,
        "d" => 2,
        "d#" | "eb" => 3,
        "e" => 4,
        "f" => 5,
        "f#" | "gb" => 6,
        "g" => 7,
        "g#" | "ab" => 8,
        "a" => 9,
        "a#" | "bb" => 10,
        "b" => 11,
        _ => return None,
    };
    // The minor index follows its relative major (A minor follows C major).
    Some(if minor {
        ((pitch + 3) % 12) * 2 + 1
    } else {
        pitch * 2
    })
}

fn objects(tx: &Transaction<'_>, collection: &str) -> Result<Vec<(String, Object)>> {
    let mut query = tx.prepare("SELECT key,data FROM database2 WHERE collection=?1")?;
    let rows = query.query_map([collection], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let mut result = Vec::new();
    for row in rows {
        let (key, data) = row?;
        result.push((
            key.clone(),
            decode(&data).with_context(|| format!("unsupported djay object {collection}/{key}"))?,
        ));
    }
    Ok(result)
}
fn existing(tx: &Transaction<'_>, collection: &str, key: &str, class: &str) -> Result<Object> {
    let bytes = tx
        .query_row(
            "SELECT data FROM database2 WHERE collection=?1 AND key=?2",
            params![collection, key],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()?;
    match bytes {
        Some(bytes) => decode(&bytes),
        None => Ok(Object::new(class)),
    }
}
fn upsert(tx: &Transaction<'_>, collection: &str, key: &str, object: &Object) -> Result<()> {
    let data = encode(object)?;
    let updated = tx.execute(
        "UPDATE database2 SET data=?1 WHERE collection=?2 AND key=?3",
        params![data, collection, key],
    )?;
    if updated == 0 {
        tx.execute(
            "INSERT INTO database2(collection,key,data,metadata) VALUES(?1,?2,?3,NULL)",
            params![collection, key, data],
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct Object {
    class: String,
    fields: Vec<(String, Value)>,
}
impl Object {
    fn new(class: &str) -> Self {
        Self {
            class: class.to_owned(),
            fields: Vec::new(),
        }
    }
    fn get(&self, key: &str) -> Option<&Value> {
        self.fields
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }
    fn set(&mut self, key: &str, value: Value) {
        self.remove(key);
        self.fields.push((key.to_owned(), value));
    }
    fn remove(&mut self, key: &str) {
        self.fields.retain(|(name, _)| name != key);
    }
}
#[derive(Clone, Debug, PartialEq)]
enum Value {
    String(String),
    Uri(String),
    Double(f64),
    Float(f32),
    Date(f64),
    U16(u16),
    U32(u32),
    U64(u64),
    Small(u8),
    Bool(bool),
    Null,
    Binary(Vec<u8>),
    Class(Object),
    Array(Vec<Value>),
    Fields(Vec<(String, Value)>),
}
fn text(value: &str) -> Value {
    Value::String(value.to_owned())
}

struct Encoder {
    bytes: Vec<u8>,
    strings: u32,
}
impl Encoder {
    fn aligned(&mut self, tag: u8, alignment: usize, value: &[u8]) {
        self.bytes.push(tag);
        self.bytes
            .resize(self.bytes.len().next_multiple_of(alignment), 0);
        self.bytes.extend_from_slice(value);
    }
    fn string(&mut self, value: &str) -> Result<()> {
        ensure!(!value.contains('\0'), "djay strings cannot contain NUL");
        self.strings += 1;
        self.bytes.push(8);
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(0);
        Ok(())
    }
    fn fields(&mut self, fields: &[(String, Value)]) -> Result<()> {
        for (name, value) in fields {
            self.value(value)?;
            self.string(name)?;
        }
        Ok(())
    }
    fn value(&mut self, value: &Value) -> Result<()> {
        match value {
            Value::String(value) => self.string(value)?,
            Value::Uri(value) => {
                self.bytes.push(33);
                self.string(value)?;
                self.bytes.push(0);
            }
            Value::Double(value) => self.aligned(20, 8, &value.to_le_bytes()),
            Value::Date(value) => self.aligned(48, 8, &value.to_le_bytes()),
            Value::Float(value) => self.aligned(19, 4, &value.to_le_bytes()),
            Value::U16(value) => self.aligned(16, 2, &value.to_le_bytes()),
            Value::U32(value) => self.aligned(17, 4, &value.to_le_bytes()),
            Value::U64(value) => self.aligned(18, 8, &value.to_le_bytes()),
            Value::Small(0) => self.bytes.push(46),
            Value::Small(1) => self.bytes.push(45),
            Value::Small(value) => self.bytes.extend_from_slice(&[15, *value]),
            Value::Bool(value) => self.bytes.push(if *value { 13 } else { 14 }),
            Value::Null => self.bytes.push(0),
            Value::Binary(value) => {
                self.aligned(21, 4, &u32::try_from(value.len())?.to_le_bytes());
                self.bytes.extend_from_slice(value);
                self.bytes.resize(self.bytes.len().next_multiple_of(4), 0);
            }
            Value::Class(object) => {
                self.bytes.push(43);
                self.string(&object.class)?;
                self.fields(&object.fields)?;
                self.bytes.push(0);
            }
            Value::Array(values) => {
                self.aligned(
                    if values.iter().all(|value| matches!(value, Value::String(_))) {
                        27
                    } else {
                        10
                    },
                    4,
                    &u32::try_from(values.len())?.to_le_bytes(),
                );
                for value in values {
                    self.value(value)?;
                }
            }
            Value::Fields(fields) => {
                self.aligned(24, 4, &u32::try_from(fields.len())?.to_le_bytes());
                self.fields(fields)?;
            }
        }
        Ok(())
    }
}
fn encode(object: &Object) -> Result<Vec<u8>> {
    let mut encoder = Encoder {
        bytes: vec![0; 20],
        strings: 0,
    };
    encoder.bytes[..4].copy_from_slice(b"TSAF");
    encoder.bytes[4..8].copy_from_slice(&[3, 0, 3, 0]);
    encoder.value(&Value::Class(object.clone()))?;
    encoder.bytes[16..20].copy_from_slice(&encoder.strings.to_le_bytes());
    Ok(encoder.bytes)
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    strings: Vec<String>,
    numbers: Vec<Value>,
}
impl Decoder<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8]> {
        let end = self
            .offset
            .checked_add(count)
            .context("djay object size overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .context("truncated djay object")?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn aligned<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.offset = self.offset.next_multiple_of(N);
        Ok(self.take(N)?.try_into().expect("fixed size"))
    }
    fn count(&mut self) -> Result<usize> {
        Ok(u32::from_le_bytes(self.aligned()?) as usize)
    }
    fn string(&mut self) -> Result<String> {
        let size = self
            .bytes
            .get(self.offset..)
            .context("truncated string")?
            .iter()
            .position(|byte| *byte == 0)
            .context("unterminated djay string")?;
        let value =
            String::from_utf8(self.take(size)?.to_owned()).context("non UTF-8 djay string")?;
        self.take(1)?;
        self.strings.push(value.clone());
        Ok(value)
    }
    fn fields(&mut self, count: Option<usize>, depth: usize) -> Result<Vec<(String, Value)>> {
        let mut fields = Vec::new();
        while count.is_none_or(|count| fields.len() < count) {
            if count.is_none() && self.bytes.get(self.offset) == Some(&0) {
                self.offset += 1;
                break;
            }
            let value = self.value(depth + 1)?;
            let Value::String(name) = self.value(depth + 1)? else {
                bail!("invalid djay field name")
            };
            fields.push((name, value));
        }
        Ok(fields)
    }
    fn value(&mut self, depth: usize) -> Result<Value> {
        ensure!(depth < 64, "djay object nesting exceeds 64 levels");
        Ok(match self.byte()? {
            0 => Value::Null,
            8 => Value::String(self.string()?),
            5 => {
                let index = self.byte()? as usize;
                text(self.strings.get(index).context("invalid string pointer")?)
            }
            2 => {
                let index = self.byte()? as usize;
                self.numbers
                    .get(index)
                    .context("invalid number pointer")?
                    .clone()
            }
            20 => Value::Double(f64::from_le_bytes(self.aligned()?)),
            19 => {
                let value = Value::Float(f32::from_le_bytes(self.aligned()?));
                self.numbers.push(value.clone());
                value
            }
            48 => {
                let value = Value::Date(f64::from_le_bytes(self.aligned()?));
                self.numbers.push(value.clone());
                value
            }
            16 => Value::U16(u16::from_le_bytes(self.aligned()?)),
            17 => Value::U32(u32::from_le_bytes(self.aligned()?)),
            18 => Value::U64(u64::from_le_bytes(self.aligned()?)),
            13 => Value::Bool(true),
            14 => Value::Bool(false),
            46 => Value::Small(0),
            45 => Value::Small(1),
            15 => Value::Small(self.byte()?),
            33 => {
                ensure!(self.byte()? == 8, "invalid djay URI string");
                let value = self.string()?;
                self.strings.pop();
                ensure!(self.byte()? == 0, "invalid djay URI terminator");
                Value::Uri(value)
            }
            21 => {
                let size = self.count()?;
                let value = self.take(size)?.to_owned();
                self.offset = self.offset.next_multiple_of(4);
                Value::Binary(value)
            }
            43 => {
                let Value::String(class) = self.value(depth + 1)? else {
                    bail!("invalid djay class name")
                };
                Value::Class(Object {
                    class,
                    fields: self.fields(None, depth + 1)?,
                })
            }
            24 => {
                let count = self.count()?;
                ensure!(count <= self.bytes.len(), "invalid djay field count");
                Value::Fields(self.fields(Some(count), depth + 1)?)
            }
            27 | 12 | 10 | 11 | 26 => {
                let count = self.count()?;
                ensure!(count <= self.bytes.len(), "invalid djay array length");
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(self.value(depth + 1)?);
                }
                Value::Array(values)
            }
            tag => bail!("unsupported current djay object tag {tag:#x}"),
        })
    }
}
fn decode(bytes: &[u8]) -> Result<Object> {
    ensure!(
        bytes.get(..8) == Some(&b"TSAF\x03\x00\x03\x00"[..]),
        "unsupported djay object header (current TSAF required)"
    );
    ensure!(bytes.len() >= 21, "truncated djay object header");
    let mut decoder = Decoder {
        bytes,
        offset: 20,
        strings: Vec::new(),
        numbers: Vec::new(),
    };
    let Value::Class(object) = decoder.value(0)? else {
        bail!("djay object root is not a class")
    };
    ensure!(
        decoder.offset == bytes.len(),
        "trailing data after djay object"
    );
    Ok(object)
}

#[cfg(test)]
#[path = "djay_pro/tests.rs"]
mod tests;

fn reference_objects(tx: &Transaction<'_>, source: &Path) -> Result<Vec<(String, Object)>> {
    let uri = uri_identity(&file_uri(source)?);
    Ok(objects(tx,"localMediaItemLocations")?.into_iter().filter(|(_,object)| matches!(object.get("sourceURIs"),Some(Value::Array(values)) if values.iter().any(|v|matches!(v,Value::Uri(v)|Value::String(v) if uri_identity(v)==uri)))).collect())
}
pub(crate) fn reference_id(database: &Path, source: &Path) -> Result<String> {
    let mut conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let tx = conn.transaction()?;
    let found = reference_objects(&tx, source)?;
    ensure!(
        found.len() == 1,
        "djay source reference is missing or ambiguous"
    );
    Ok(found[0].0.clone())
}
pub(crate) fn repair_reference(
    database: &Path,
    source: &Path,
    destination: &Path,
    expected: &str,
) -> Result<()> {
    let mut conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    let tx = conn.transaction()?;
    let mut found = reference_objects(&tx, source)?;
    ensure!(
        found.len() == 1 && found[0].0 == expected,
        "djay source reference changed"
    );
    ensure!(
        reference_objects(&tx, destination)?
            .iter()
            .all(|(id, _)| id == expected),
        "djay destination already has a native track"
    );
    let (id, mut object) = found.remove(0);
    let from = uri_identity(&file_uri(source)?);
    let to = file_uri(destination)?;
    let Some(Value::Array(previous)) = object.get("sourceURIs") else {
        anyhow::bail!("djay source URI record is missing")
    };
    let mut changed = previous.clone();
    for value in &mut changed {
        match value {
            Value::Uri(uri) | Value::String(uri) if uri_identity(uri) == from => *uri = to.clone(),
            _ => {}
        }
    }
    object.set("sourceURIs", Value::Array(changed));
    upsert(&tx, "localMediaItemLocations", &id, &object)?;
    let index:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='secondaryIndex_mediaItemLocationIndex')",[],|row|row.get(0))?;
    if index {
        let filename = source_filename(destination)?;
        tx.execute("UPDATE secondaryIndex_mediaItemLocationIndex SET fileName=?1 WHERE rowid=(SELECT rowid FROM database2 WHERE collection='localMediaItemLocations' AND key=?2)",params![filename,id])?;
    }
    for extension in INDEXES {
        tx.execute(
            "UPDATE yap2 SET data=0 WHERE extension=?1 AND key='versionTag'",
            [extension],
        )?;
    }
    tx.commit()?;
    Ok(())
}

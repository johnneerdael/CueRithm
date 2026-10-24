//! Shared observations from authenticated direct SQLite snapshots; no XML transport.
use super::*;
use cuerithm_rekordbox::{self as rb, DatabaseKey, ReadOnlyDatabase, TargetValue};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeSet;

/// Backend-only runtime key, never deserialized, persisted or exposed in diagnostics.
pub struct RekordboxReader {
    key: Option<DatabaseKey>,
    source_options: Option<rb::CollectionReadOptions>,
}
impl RekordboxReader {
    pub const fn unconfigured() -> Self {
        Self {
            key: None,
            source_options: None,
        }
    }
    pub fn with_provisioned_key(key: DatabaseKey, options: rb::CollectionReadOptions) -> Self {
        Self {
            key: Some(key),
            source_options: Some(options),
        }
    }
    pub fn from_selected_installation(
        installation: &rb::TargetInstallation,
        environment: &rb::TargetEnvironment,
        platform: rb::DesktopPlatform,
        options_file: &Path,
        options: rb::CollectionReadOptions,
        cancel: &CancellationToken,
    ) -> Result<Self> {
        let key = rb::resolve_database_key(
            &rb::NativeKeySource::for_environment(installation, environment, platform)?,
            options_file,
            rb::KeyProviderOptions::default(),
            cancel,
        )?;
        Ok(Self::with_provisioned_key(key, options))
    }
    fn key(&self) -> Result<&DatabaseKey> {
        self.key.as_ref().context(
            "Choose a supported rekordbox installation to provision runtime direct-library access",
        )
    }
    fn open(&self, snapshot: &ImportSnapshot) -> Result<(ReadOnlyDatabase, SourceProfile)> {
        ensure!(
            snapshot.provider == SourceProvider::Rekordbox,
            "Mismatched direct library provider"
        );
        let databases = snapshot
            .artifacts
            .iter()
            .filter(|a| matches!(a.kind, ArtifactKind::Sqlite))
            .collect::<Vec<_>>();
        ensure!(
            databases.len() == 1,
            "Choose one coherent direct master.db snapshot"
        );
        let id = &databases[0].id;
        ensure!(
            snapshot
                .original_artifact_path(id)?
                .file_name()
                .is_some_and(|n| n == "master.db"),
            "Unsupported direct library artifact"
        );
        let db = ReadOnlyDatabase::open(snapshot.artifact(id)?, self.key()?)?;
        let schema = db.schema()?;
        ensure!(
            schema.user_version == 0 && schema.application_id == 0,
            "Unsupported direct collection database headers"
        );
        for table in rb::COLLECTION_IMPORT_TABLES {
            ensure!(
                schema.objects.iter().any(|o| o.name == *table
                    && o.kind == "table"
                    && !o.sql.to_ascii_uppercase().contains("VIRTUAL TABLE")),
                "Unsupported direct collection music-table profile"
            );
        }
        let mut profile = profile(
            SourceProvider::Rekordbox,
            "rekordbox-direct-collection-read-v1",
            &[
                "descriptive_tags",
                "key",
                "bpm",
                "beatgrid",
                "hot_cues",
                "memory_cues",
                "markers",
                "loops",
                "ratings",
                "collections",
            ],
        );
        profile.schema_fingerprint = schema.fingerprint;
        profile.origin.insert("read_boundary".into(),"Existing authenticated direct collection layout; read-only D2 fixtures, native write acceptance remains separate".into());
        Ok((db, profile))
    }
}
fn scalar(value: &Value) -> Presence<Value> {
    if value.is_null() {
        Presence::Null
    } else if value.as_str() == Some("") {
        Presence::Empty(value.clone())
    } else {
        Presence::Value(value.clone())
    }
}
fn get(db: &Connection, table: &str, id: &str) -> Result<Option<Value>> {
    db.query_row(
        "SELECT raw FROM records WHERE kind=?1 AND id=?2",
        params![table, id],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|s| serde_json::from_str(&s).map_err(Into::into))
    .transpose()
}
fn foreign(
    db: &Connection,
    raw: &Value,
    column: &str,
    table: &str,
    name: &str,
    track: &mut TrackObservation,
) -> Result<Presence<Value>> {
    let Some(id) = raw.get(column) else {
        return Ok(Presence::Absent);
    };
    if id.is_null() {
        return Ok(Presence::Null);
    }
    let id = id
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| id.to_string());
    if let Some(row) = get(db, table, &id)? {
        Ok(row.get(name).map(scalar).unwrap_or(Presence::Absent))
    } else {
        issue(track, "missing_rekordbox_foreign_record", column);
        Ok(Presence::Absent)
    }
}
fn values_for_location(raw: &Value) -> rb::TargetRecord {
    let mut values = BTreeMap::new();
    for name in ["FolderPath", "OrgFolderPath", "ServiceID"] {
        if let Some(value) = raw.get(name) {
            values.insert(
                name.into(),
                match value {
                    Value::Null => TargetValue::Null,
                    Value::String(v) => TargetValue::Text(v.clone()),
                    _ => TargetValue::Integer(value.as_i64().unwrap_or(-1)),
                },
            );
        }
    }
    rb::TargetRecord {
        table: "djmdContent".into(),
        id: String::new(),
        values,
        fingerprint: String::new(),
    }
}
fn children(db: &Connection, table: &str, id: &str, options: &ReadOptions) -> Result<Vec<Value>> {
    let mut statement =
        db.prepare("SELECT raw FROM records WHERE kind=?1 AND parent=?2 ORDER BY ordering,id")?;
    let mut rows = statement.query(params![table, id])?;
    let mut result = Vec::new();
    let mut total = 0u64;
    while let Some(row) = rows.next()? {
        let raw: String = row.get(0)?;
        total += raw.len() as u64;
        ensure!(
            total <= options.limits.blob_bytes && result.len() < 100_000,
            "Direct per-track child budget"
        );
        result.push(serde_json::from_str(&raw)?);
    }
    Ok(result)
}
fn analysis(
    snapshot: &ImportSnapshot,
    raw: &Value,
    options: &ReadOptions,
    track: &mut TrackObservation,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<()> {
    let Some(reference) = raw["AnalysisDataPath"].as_str().filter(|s| !s.is_empty()) else {
        issue(track, "missing_rekordbox_analysis", "beatgrid");
        return Ok(());
    };
    let relative = reference.replace('\\', "/");
    let candidate = snapshot
        .artifacts
        .iter()
        .filter(|a| matches!(a.kind, ArtifactKind::StableFile))
        .find(|a| a.id == reference || a.id == relative.trim_start_matches('/'));
    let Some(artifact) = candidate else {
        issue(track, "missing_rekordbox_analysis", "beatgrid");
        return Ok(());
    };
    let reference_path = Path::new(relative.trim_start_matches('/'));
    if !reference_path
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
        || !snapshot
            .original_artifact_path(&artifact.id)?
            .ends_with(reference_path)
    {
        issue(track, "mismatched_rekordbox_analysis_reference", "beatgrid");
        return Ok(());
    }
    cancel.check()?;
    ensure!(
        artifact.bytes <= options.limits.blob_bytes,
        "Direct ANLZ byte limit"
    );
    let data = fs::read(snapshot.artifact(&artifact.id)?)?;
    let raw_analysis = sink.put_blob(&data, None)?;
    track
        .extensions
        .insert("raw_analysis".into(), serde_json::to_value(&raw_analysis)?);
    let limits = rb::anlz::ParseLimits {
        max_bytes: usize::try_from(options.limits.blob_bytes)?,
        ..Default::default()
    };
    let decoded = rb::anlz::AnlzFile::parse(&data, limits).and_then(|file| {
        let grids = file
            .tags()
            .iter()
            .filter(|t| t.fourcc() == *b"PQTZ")
            .collect::<Vec<_>>();
        ensure!(grids.len() == 1, "Missing/ambiguous primary beat grid");
        grids[0].beats()
    });
    match decoded {
        Ok(beats) => {
            track.grid = Some(GridObservation {
                representation: "rekordbox_pqtz_explicit".into(),
                original_time_base: Some("unsigned milliseconds".into()),
                native_first_beat_seconds: beats.first().map(|b| f64::from(b.time_ms) / 1000.),
                bpm: beats.first().map(|b| f64::from(b.tempo) / 100.),
                downbeat_known: beats.iter().all(|b| (1..=4).contains(&b.number)),
                clock_status: ClockStatus::UnresolvedNative,
                raw: Some(raw_analysis),
                points: beats
                    .into_iter()
                    .map(|b| GridPoint {
                        native_seconds: f64::from(b.time_ms) / 1000.,
                        bpm: Some(f64::from(b.tempo) / 100.),
                        original: json!({"number":b.number,"tempo":b.tempo,"time_ms":b.time_ms}),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            });
        }
        Err(_) => issue(track, "unsupported_rekordbox_analysis", "beatgrid"),
    }
    Ok(())
}
struct TrackReadContext<'a> {
    reader: &'a RekordboxReader,
    snapshot: &'a ImportSnapshot,
    db: &'a Connection,
    options: &'a ReadOptions,
    cancel: &'a CancellationToken,
}
impl TrackReadContext<'_> {
    fn track(
        &self,
        id: &str,
        raw: Value,
        sink: &mut dyn ObservationSink,
    ) -> Result<TrackObservation> {
        let Self {
            reader,
            snapshot,
            db,
            options,
            cancel,
        } = *self;
        let mut track = TrackObservation {
            external_id: id.into(),
            ..Default::default()
        };
        for (source, target) in [
            ("Title", "title"),
            ("Commnt", "comment"),
            ("Subtitle", "version"),
            ("Lyricist", "lyricist"),
            ("ReleaseYear", "year"),
            ("ReleaseDate", "release_date"),
            ("TrackNo", "track_number"),
            ("DiscNo", "disc_number"),
        ] {
            if let Some(value) = raw.get(source) {
                track.metadata.insert(target.into(), scalar(value));
            }
        }
        for (column, table, field, target) in [
            ("ArtistID", "djmdArtist", "Name", "artist"),
            ("AlbumID", "djmdAlbum", "Name", "album"),
            ("GenreID", "djmdGenre", "Name", "genre"),
            ("LabelID", "djmdLabel", "Name", "label"),
            ("RemixerID", "djmdArtist", "Name", "remixer"),
            ("ComposerID", "djmdArtist", "Name", "composer"),
        ] {
            let value = foreign(db, &raw, column, table, field, &mut track)?;
            track.metadata.insert(target.into(), value);
        }
        if let Some(album) = raw["AlbumID"]
            .as_str()
            .map(|id| get(db, "djmdAlbum", id))
            .transpose()?
            .flatten()
        {
            let value = foreign(
                db,
                &album,
                "AlbumArtistID",
                "djmdArtist",
                "Name",
                &mut track,
            )?;
            track.metadata.insert("album_artist".into(), value);
        }
        let key = foreign(db, &raw, "KeyID", "djmdKey", "ScaleName", &mut track)?;
        if !matches!(key, Presence::Absent) {
            let text = match &key {
                Presence::Value(v) | Presence::Empty(v) => v.as_str().map(str::to_owned),
                _ => None,
            };
            track.key = Some(MusicalKey {
                presence: if matches!(key, Presence::Null) {
                    FieldPresence::Null
                } else {
                    FieldPresence::Value
                },
                camelot: text
                    .as_deref()
                    .and_then(|s| crate::formatting::parse_key(s).ok())
                    .map(|k| k.camelot()),
                raw: text,
                provenance: Some("rekordbox djmdKey".into()),
                ..Default::default()
            });
        }
        if let Some(value) = raw.get("BPM") {
            track.bpm = Some(ScalarBpm {
                presence: if value.is_null() {
                    FieldPresence::Null
                } else {
                    FieldPresence::Value
                },
                value: value.as_f64().filter(|n| *n > 0.).map(|n| n / 100.),
                provenance: Some("rekordbox BPM hundredths".into()),
                ..Default::default()
            });
        }
        if let Some(value) = raw.get("Rating") {
            track.metadata.insert(
                "rating".into(),
                if value.is_null() {
                    Presence::Null
                } else {
                    Presence::Value(json!({"raw":value,"scale":5}))
                },
            );
        }
        let source_options = reader.source_options.clone().unwrap_or_default();
        match rb::resolved_recorded_path(&values_for_location(&raw), &source_options)? {
            Some(path) => {
                track.locations = location(&path, None, options)?;
                if track.locations.availability != "available" {
                    issue(&mut track, "unavailable_rekordbox_media", "location");
                }
            }
            None => {
                track.locations.original = raw["FolderPath"].as_str().map(str::to_owned);
                track.locations.availability = "unresolved".into();
                issue(&mut track, "unresolved_rekordbox_location", "location");
            }
        }
        for cue in children(db, "djmdCue", id, options)? {
            if cue["rb_local_deleted"].as_i64() != Some(0) {
                continue;
            }
            let kind = cue["Kind"].as_i64();
            let slot = match kind {
                Some(1..=3) => kind.map(|k| (k - 1) as i32),
                Some(5..=17) => kind.map(|k| (k - 2) as i32),
                _ => None,
            };
            let memory = matches!(kind, Some(0 | 4));
            let family = if memory {
                CueFamily::Memory
            } else if slot.is_some() {
                CueFamily::Hot
            } else {
                CueFamily::Unknown
            };
            if family == CueFamily::Unknown {
                issue(&mut track, "unknown_rekordbox_hotcue_occupancy", "cues");
            }
            let start = cue["InMsec"].as_f64().map(|v| v / 1000.);
            let end = cue["OutMsec"]
                .as_f64()
                .filter(|v| *v != -1.)
                .map(|v| v / 1000.);
            if start.is_none() {
                issue(&mut track, "missing_rekordbox_cue_start", "cues");
            }
            track.cues.push(CueObservation {
                external_id: cue["ID"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| cue["ID"].to_string()),
                family,
                source_slot: slot,
                name: cue["Comment"].as_str().map(str::to_owned),
                native_start_seconds: start,
                native_end_seconds: end,
                clock_status: ClockStatus::UnresolvedNative,
                flags: BTreeMap::from([(
                    "active_loop".into(),
                    json!(
                        kind == Some(4)
                            || end.is_some() && cue["ActiveLoop"].as_i64().is_some_and(|v| v != 0)
                    ),
                )]),
                original: cue,
                ..Default::default()
            });
        }
        let cached = children(db, "contentCue", id, options)?;
        if !cached.is_empty() {
            track.cached_tags.insert(
                "rekordbox_content_cue".into(),
                Presence::Value(json!(cached)),
            );
        }
        let mut tags = Vec::new();
        for link in children(db, "djmdSongMyTag", id, options)? {
            if link["rb_local_deleted"].as_i64() != Some(0) {
                continue;
            }
            if let Some(tag_id) = link["MyTagID"].as_str() {
                if let Some(tag) = get(db, "djmdMyTag", tag_id)? {
                    tags.push(json!({"id":tag_id,"name":tag["Name"],"parent_id":tag["ParentID"],"source_row":tag,"source_link":link}));
                } else {
                    issue(&mut track, "missing_rekordbox_my_tag", "my_tags");
                }
            }
        }
        track.extensions.insert("my_tags".into(), json!(tags));
        track.extensions.insert("native_identity".into(),json!({"content_id":id,"master_song_id":raw["MasterSongID"],"uuid":raw["UUID"],"local_usn":raw["rb_local_usn"]}));
        track
            .extensions
            .insert("native_deleted".into(), raw["rb_local_deleted"].clone());
        if raw["rb_local_deleted"].as_i64() != Some(0) {
            issue(&mut track, "deleted_or_unknown_rekordbox_track", "source");
        }
        if let Some(value) = raw["Length"].as_f64().filter(|n| *n > 0.) {
            track
                .extensions
                .insert("duration_seconds".into(), json!(value));
        }
        analysis(snapshot, &raw, options, &mut track, sink, cancel)?;
        track.extensions.insert("source_row".into(), raw);
        Ok(track)
    }
}
fn parent(db: &Connection, id: &str, options: &ReadOptions) -> Result<(Option<String>, bool)> {
    let raw = get(db, "djmdPlaylist", id)?.context("Missing playlist record")?;
    let first = raw["ParentID"]
        .as_str()
        .filter(|s| !s.is_empty() && *s != "root")
        .map(str::to_owned);
    let mut next = first.clone();
    let mut seen = BTreeSet::from([id.to_owned()]);
    while let Some(id) = next {
        if !seen.insert(id.clone()) || seen.len() > options.limits.hierarchy_depth {
            return Ok((None, true));
        }
        next = get(db, "djmdPlaylist", &id)?.and_then(|r| {
            r["ParentID"]
                .as_str()
                .filter(|s| !s.is_empty() && *s != "root")
                .map(str::to_owned)
        });
    }
    Ok((first, false))
}
impl SourceAdapter for RekordboxReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::Rekordbox
    }
    fn capture(
        &self,
        request: &SourceCaptureRequest,
        workspace: &Path,
        limits: &ImportLimits,
        cancel: &CancellationToken,
    ) -> Result<ImportSnapshot> {
        ensure!(
            request.provider == self.provider(),
            "Mismatched direct capture provider"
        );
        let key = self.key()?;
        super::super::snapshot::capture_source_with_sqlite(
            request,
            workspace,
            limits,
            cancel,
            |input, output, _, cancel| {
                let db = ReadOnlyDatabase::open(input, key)?;
                let copy = db.capture_with_progress(
                    output.parent().context("Missing private snapshot root")?,
                    key,
                    cancel,
                    |_| {},
                )?;
                fs::copy(&copy.path, output)?;
                fs::File::options().write(true).open(output)?.sync_all()?;
                Ok(())
            },
        )
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        Ok(self.open(snapshot)?.1)
    }
    fn read(
        &self,
        snapshot: &ImportSnapshot,
        options: &ReadOptions,
        sink: &mut dyn ObservationSink,
        cancel: &CancellationToken,
    ) -> Result<ObservationHeader> {
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        let (native, profile) = self.open(snapshot)?;
        let temp = tempfile::tempdir()?;
        let db = Connection::open(temp.path().join("direct-import.sqlite"))?;
        db.pragma_update(None, "cache_size", -2048)?;
        db.execute_batch("CREATE TABLE records(kind TEXT,id TEXT,parent TEXT,ordering REAL,raw TEXT,PRIMARY KEY(kind,id));CREATE INDEX children ON records(kind,parent,ordering,id);BEGIN;")?;
        let mut pages = Vec::new();
        let mut page = Vec::new();
        let mut page_bytes = 0usize;
        native.visit_collection_records(rb::CollectionRecordLimits{max_rows:2_000_000,text_bytes:usize::try_from(options.limits.text_bytes)?,blob_bytes:usize::try_from(options.limits.blob_bytes)?},cancel,|record|{
            let mut raw=serde_json::Map::new();for (name,value) in record.values {raw.insert(name,match value {TargetValue::Null=>Value::Null,TargetValue::Integer(v)=>json!(v),TargetValue::Real(v)=>json!(v),TargetValue::Text(v)=>json!(v),TargetValue::Blob(bytes)=>json!({"blob":sink.put_blob(&bytes,None)?})});}let raw=Value::Object(raw);
            let parent=if record.table=="djmdPlaylist"{raw["ParentID"].as_str()}else if record.table=="djmdSongPlaylist"{raw["PlaylistID"].as_str()}else{raw["ContentID"].as_str()};
            let ordering=raw[if record.table=="djmdPlaylist"{"Seq"}else if record.table=="djmdSongPlaylist"{"TrackNo"}else{"InMsec"}].as_f64();
            let json=serde_json::to_string(&raw)?;ensure!(json.len() as u64<=options.limits.text_bytes,"Direct source record text budget");
            db.execute("INSERT INTO records VALUES(?1,?2,?3,?4,?5)",params![record.table,record.id,parent,ordering,json])?;
            let row=json!({"table":record.table,"id":record.id,"raw":raw,"fingerprint":record.fingerprint});let size=serde_json::to_vec(&row)?.len();
            if !page.is_empty() && (page.len()>=options.limits.page_size.max(1) || page_bytes+size>options.limits.text_bytes as usize){pages.push(sink.put_blob(&serde_json::to_vec(&page)?,Some("application/json"))?);page.clear();page_bytes=0;}
            page_bytes+=size;page.push(row);Ok(())
        })?;
        if !page.is_empty() {
            pages.push(sink.put_blob(&serde_json::to_vec(&page)?, Some("application/json"))?);
        }
        let properties: i64 = db.query_row(
            "SELECT count(*) FROM records WHERE kind='djmdProperty'",
            [],
            |r| r.get(0),
        )?;
        ensure!(
            properties == 1,
            "Missing/ambiguous native database identity"
        );
        let (database_id, raw_property): (String, String) = db.query_row(
            "SELECT id,raw FROM records WHERE kind='djmdProperty'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        // Missing native references stay represented, without guessing a matching path.
        db.execute_batch("INSERT OR IGNORE INTO records SELECT 'djmdContent',parent,NULL,NULL,'{\"missing_source_track\":true}' FROM records WHERE kind IN('djmdCue','contentCue','djmdSongMyTag') AND parent IS NOT NULL;INSERT OR IGNORE INTO records SELECT 'djmdContent',json_extract(raw,'$.ContentID'),NULL,NULL,'{\"missing_source_track\":true}' FROM records WHERE kind='djmdSongPlaylist' AND json_extract(raw,'$.ContentID') IS NOT NULL;INSERT OR IGNORE INTO records SELECT 'djmdPlaylist',parent,NULL,NULL,'{\"missing_source_playlist\":true}' FROM records WHERE kind IN('djmdPlaylist','djmdSongPlaylist') AND parent IS NOT NULL AND parent!='root' AND parent!='';COMMIT;")?;
        let tracks: i64 = db.query_row(
            "SELECT count(*) FROM records WHERE kind='djmdContent'",
            [],
            |r| r.get(0),
        )?;
        let collections: i64 = db.query_row(
            "SELECT count(*) FROM records WHERE kind='djmdPlaylist'",
            [],
            |r| r.get(0),
        )?;
        let members:i64=db.query_row("SELECT count(*) FROM records WHERE kind='djmdSongPlaylist' AND json_extract(raw,'$.rb_local_deleted')=0",[],|r|r.get(0))?;
        let mut header = header(
            snapshot,
            options,
            &profile,
            Some(&database_id),
            tracks as u64,
            collections as u64,
            members as u64,
        );
        header.product_version = "rekordbox direct collection".into();
        header.extensions.insert(
            "native_database_identity".into(),
            serde_json::from_str(&raw_property)?,
        );
        header
            .extensions
            .insert("raw_record_pages".into(), serde_json::to_value(pages)?);
        let mut statement =
            db.prepare("SELECT id,raw FROM records WHERE kind='djmdContent' ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: String = row.get(0)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
            let context = TrackReadContext {
                reader: self,
                snapshot,
                db: &db,
                options,
                cancel,
            };
            let mut value = context.track(&id, raw, sink)?;
            value
                .extensions
                .insert("native_database_id".into(), json!(database_id));
            emit_track(value, options, sink, cancel)?;
        }
        let mut statement = db
            .prepare("SELECT id,raw FROM records WHERE kind='djmdPlaylist' ORDER BY ordering,id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: String = row.get(0)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
            let (parent_id, cycle) = parent(&db, &id, options)?;
            if cycle || raw["missing_source_playlist"] == true {
                sink.push(ObservationRecord::Issue(ImportIssue {
                    code: if cycle {
                        "rekordbox_playlist_cycle"
                    } else {
                        "missing_rekordbox_playlist"
                    }
                    .into(),
                    record_id: Some(id.clone()),
                    message: "Native playlist relationship is unresolved; source row retained"
                        .into(),
                    ..Default::default()
                }))?;
            }
            sink.push(ObservationRecord::Collection(CollectionObservation {
                external_id: id,
                parent_id,
                kind: match raw["Attribute"].as_i64() {
                    Some(1) => CollectionKind::Folder,
                    Some(4) => CollectionKind::Smart,
                    _ => CollectionKind::Playlist,
                },
                name: raw["Name"].as_str().unwrap_or("").into(),
                extensions: BTreeMap::from([
                    ("source_row".into(), raw.clone()),
                    ("display_order".into(), raw["Seq"].clone()),
                    ("smart_rule".into(), raw["SmartList"].clone()),
                    ("native_deleted".into(), raw["rb_local_deleted"].clone()),
                ]),
                ..Default::default()
            }))?;
        }
        let mut statement=db.prepare("SELECT id,parent,raw FROM records WHERE kind='djmdSongPlaylist' AND json_extract(raw,'$.rb_local_deleted')=0 ORDER BY parent,ordering,id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(2)?)?;
            let content = raw["ContentID"]
                .as_str()
                .context("Missing native playlist content reference")?;
            sink.push(ObservationRecord::Membership {
                collection_id: row.get(1)?,
                occurrence: MembershipObservation {
                    external_occurrence_id: row.get(0)?,
                    track_external_id: content.into(),
                    position: raw["TrackNo"].as_i64(),
                    extensions: BTreeMap::from([("source_row".into(), raw)]),
                },
            })?;
        }
        let mut invalid=db.prepare("SELECT kind,id FROM records WHERE kind IN('djmdCue','contentCue','djmdSongMyTag') AND (parent IS NULL OR parent='') ORDER BY kind,id")?;
        let mut rows = invalid.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let kind: String = row.get(0)?;
            let id: String = row.get(1)?;
            sink.push(ObservationRecord::Issue(ImportIssue {
                code: "unlinked_rekordbox_child".into(),
                record_id: Some(format!("{kind}:{id}")),
                field: Some("source".into()),
                message:
                    "Native child has no song relationship; raw record retained in source pages"
                        .into(),
            }))?;
            header.accounting.unrepresented_records += 1;
        }
        if header.accounting.unrepresented_records > 0 {
            header.completeness = Completeness::PartialWithErrors;
        }
        snapshot.validate_source(cancel)?;
        Ok(header)
    }
}

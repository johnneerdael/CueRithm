//! Streaming v26 adapter. Only one source row or one bounded musical record is resident.
use super::*;
use rusqlite::{OptionalExtension, params};
use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

#[derive(Debug, Clone)]
pub struct ImportPreviewSpooled {
    pub archive: SpoolArchive,
    pub report: ImportReport,
}

struct QueryClock {
    started: Instant,
    deadline: Arc<AtomicU64>,
}
impl QueryClock {
    fn install(conn: &Connection) -> Result<Self> {
        let started = Instant::now();
        let deadline = Arc::new(AtomicU64::new(60_000));
        let shared = deadline.clone();
        conn.progress_handler(
            10_000,
            Some(move || started.elapsed().as_millis() > shared.load(Ordering::Relaxed) as u128),
        )?;
        Ok(Self { started, deadline })
    }
    fn renew(&self) {
        self.deadline.store(
            self.started.elapsed().as_millis().min(u64::MAX as u128) as u64 + 60_000,
            Ordering::Relaxed,
        );
    }
}
struct Sink<'a> {
    conn: Connection,
    limits: &'a ArchiveLimits,
    directory: &'a Path,
    writes: Cell<u64>,
}
impl Sink<'_> {
    fn encode(&self, r: &LogicalRecord) -> Result<String> {
        let text = serde_json::to_string(r)?;
        ensure!(
            text.len() <= self.limits.record_bytes,
            "Lexicon logical record exceeds byte limit"
        );
        // The same bounded parser enforces nested extension allocation limits.
        parse_strict_json(text.as_bytes(), self.limits.depth)?;
        let n = self.writes.get();
        self.writes.set(n + 1);
        if n.is_multiple_of(256) {
            ensure!(
                fs2::available_space(self.directory)? >= 256 << 20,
                "Insufficient Lexicon staging space: retain 256 MiB reserve"
            );
        }
        Ok(text)
    }
    fn insert(&self, kind: &str, r: &LogicalRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO records VALUES(?1,?2,?3)",
            params![kind, r.id, self.encode(r)?],
        )?;
        Ok(())
    }
    fn insert_same(&self, kind: &str, r: &LogicalRecord) -> Result<()> {
        if let Some(old) = self.get(kind, &r.id)? {
            ensure!(old == *r, "Conflicting source identity");
            Ok(())
        } else {
            self.insert(kind, r)
        }
    }
    fn replace(&self, kind: &str, r: &LogicalRecord) -> Result<()> {
        ensure!(
            self.conn.execute(
                "UPDATE records SET json=?3 WHERE kind=?1 AND id=?2",
                params![kind, r.id, self.encode(r)?]
            )? == 1,
            "Missing staging record"
        );
        Ok(())
    }
    fn get(&self, kind: &str, id: &str) -> Result<Option<LogicalRecord>> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM records WHERE kind=?1 AND id=?2",
                params![kind, id],
                |r| r.get(0),
            )
            .optional()?;
        text.map(|s| serde_json::from_str(&s).map_err(Into::into))
            .transpose()
    }
    fn exists(&self, kind: &str, id: &str) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM records WHERE kind=?1 AND id=?2)",
            params![kind, id],
            |r| r.get(0),
        )?)
    }
}
fn empty_records(manifest: &Manifest) -> LogicalArchive {
    LogicalArchive {
        manifest: manifest.clone(),
        records: KINDS.iter().map(|(k, _)| (k.to_string(), vec![])).collect(),
        optional_payloads: BTreeMap::new(),
    }
}

fn visit_rows(
    source: (&Connection, &Value),
    table: &str,
    order: &str,
    limits: &ArchiveLimits,
    clock: &QueryClock,
    control: &dyn Fn(&str, u64) -> Result<()>,
    mut visit: impl FnMut(Value) -> Result<()>,
) -> Result<u64> {
    let (conn, profile) = source;
    let columns = profile["tables"][table]["columns"]
        .as_array()
        .context("Missing table profile")?;
    let names = columns
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    // Table and ordering expressions are internal constants, never source SQL.
    let sql = format!(
        "SELECT {} FROM \"{table}\" ORDER BY {order}",
        names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    clock.renew();
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        control("read_tables", count)?;
        clock.renew();
        ensure!(
            count
                < if table == "Track" {
                    limits.tracks as u64
                } else {
                    limits.relations as u64
                },
            "Lexicon source row limit"
        );
        let mut object = serde_json::Map::new();
        let mut bytes = 0usize;
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => {
                    ensure!(
                        !columns[index]["not_null"].as_bool().unwrap(),
                        "NULL required source value in {table}.{name}"
                    );
                    Value::Null
                }
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(n) => {
                    ensure!(n.is_finite(), "Nonfinite source number");
                    json!(n)
                }
                ValueRef::Text(text) => {
                    bytes = bytes.saturating_add(text.len());
                    ensure!(bytes <= limits.record_bytes, "Source record size limit");
                    json!(std::str::from_utf8(text)?)
                }
                ValueRef::Blob(_) => bail!("Unexpected source blob in {table}.{name}"),
            };
            object.insert((*name).into(), value);
        }
        visit(Value::Object(object))?;
        count += 1;
    }
    Ok(count)
}
fn save_musical(sink: &Sink<'_>, mut record: LogicalRecord) -> Result<()> {
    for field in ["cues", "tempo_markers"] {
        record.value[field].as_array_mut().unwrap().sort_by(|a, b| {
            a["start_seconds"]
                .as_f64()
                .unwrap()
                .total_cmp(&b["start_seconds"].as_f64().unwrap())
                .then(a["id"].as_str().cmp(&b["id"].as_str()))
        });
    }
    sink.replace("musical_revisions", &record)
}

fn bound_warnings(report: &mut ImportReport, omitted: &mut u64) {
    if report.warnings.len() > 1024 {
        *omitted += (report.warnings.len() - 1024) as u64;
        report.warnings.truncate(1024);
    }
}

pub fn read_lexicon_spooled(
    path: &Path,
    time_zone: &str,
    l: &ArchiveLimits,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<ImportPreviewSpooled> {
    let timezone: chrono_tz::Tz = time_zone.parse()?;
    let mut zip = open_zip(path, l)?;
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        ensure!(
            entry.name()?.as_ref() == "main.db"
                || entry.is_dir()
                || entry.name()?.starts_with("__MACOSX/"),
            "Unexpected active Lexicon archive entry"
        );
    }
    let scratch = tempfile::tempdir()?;
    let db = scratch.path().join("source.sqlite");
    {
        let mut source = zip.by_name("main.db")?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&db)?;
        let mut bytes = 0u64;
        let mut buffer = [0; 64 * 1024];
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            control("extract", bytes)?;
            ensure!(
                fs2::available_space(scratch.path())? >= 256 << 20,
                "Insufficient source extraction space: retain 256 MiB reserve"
            );
            ensure!(bytes <= l.entry_bytes, "Source DB expanded limit");
            file.write_all(&buffer[..n])?;
        }
        file.sync_all()?;
    }
    let mut magic = [0; 16];
    std::fs::File::open(&db)?.read_exact(&mut magic)?;
    ensure!(&magic == b"SQLite format 3\0", "Invalid SQLite magic");
    let conn = Connection::open_with_flags(
        &db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.set_limit(
        rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH,
        l.record_bytes.min(i32::MAX as usize) as i32,
    )?;
    conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_SQL_LENGTH, 1 << 20)?;
    conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_COLUMN, 128)?;
    conn.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_EXPR_DEPTH, 32)?;
    let progress_clock = QueryClock::install(&conn)?;
    conn.execute_batch(
        "PRAGMA trusted_schema=OFF; PRAGMA query_only=ON; PRAGMA temp_store=FILE; PRAGMA cache_size=-8192;",
    )?;
    let p = schema(&conn)?;
    let quick: String = conn.query_row("PRAGMA quick_check(1)", [], |r| r.get(0))?;
    ensure!(quick == "ok", "Lexicon integrity check failed");
    ensure!(
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))? == 0,
        "Lexicon foreign key check failed"
    );
    ensure!(
        conn.query_row("SELECT COUNT(*) FROM Database", [], |r| r.get::<_, u64>(0))? == 1,
        "Exactly one Lexicon database identity required"
    );
    let database = rows(&conn, &p, "Database", l)?;
    ensure!(
        database.len() == 1 && database[0]["version"] == 26,
        "Unsupported Lexicon database version"
    );
    let opaque = string(&database[0]["uuid"])?;
    ensure!(
        !opaque.is_empty() && opaque.len() <= 4096,
        "Missing Lexicon source identity"
    );
    let ns = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("urn:cuerithm:lexicon-library:{opaque}").as_bytes(),
    );
    let digest = {
        let mut file = zip.into_inner();
        file.rewind()?;
        let mut h = Sha256::new();
        let mut b = [0; 65536];
        let mut read = 0u64;
        loop {
            let n = file.read(&mut b)?;
            if n == 0 {
                break;
            }
            read += n as u64;
            control("source_digest", read)?;
            h.update(&b[..n]);
        }
        format!("{:x}", h.finalize())
    };
    let mut initial = LogicalArchive::empty(time_zone)?;
    initial.manifest.library_id = ns.to_string();
    let source_id = ns.to_string();
    initial.push("import_sources",LogicalRecord::new(&source_id,json!({"vendor":"lexicon","source_library_id":opaque,"profile":"lexicon-main-db-v26","archive_sha256":digest,"imported_at_ms":crate::model::now(),"source_schema":{"version":26}})));
    let manifest = initial.manifest.clone();
    let mut archive = SpoolArchive::create(manifest.clone())?;
    let sink = Sink {
        conn: archive.connection()?,
        limits: l,
        directory: archive.temp_path(),
        writes: Cell::new(0),
    };
    sink.conn.execute_batch("PRAGMA cache_size=-8192;PRAGMA temp_store=FILE;BEGIN;CREATE TEMP TABLE membership_seen(playlist TEXT NOT NULL,track TEXT NOT NULL,PRIMARY KEY(playlist,track));CREATE TEMP TABLE tag_seen(track TEXT NOT NULL,tag TEXT NOT NULL,PRIMARY KEY(track,tag));")?;
    for (kind, records) in initial.records {
        for record in records {
            sink.insert(&kind, &record)?;
        }
    }
    let mut report = ImportReport::default();
    let mut omitted = 0;
    report.warnings.push(format!(
        "Offsetless source dates interpreted in {time_zone}; original dates retained"
    ));
    report
        .warnings
        .push("No source key-mixing preference validated; harmonic mode selected".into());
    let count = visit_rows(
        (&conn, &p),
        "Track",
        "id",
        l,
        &progress_clock,
        control,
        |row| {
            let tid = id(&ns, "Track", &row["id"])?;
            let lid = id(&ns, "Locator", &row["id"])?;
            let mut locations = empty_records(&manifest);
            locator(&mut locations, lid.clone(), &row)?;
            for kind in ["roots", "locators"] {
                for record in locations.records.remove(kind).unwrap() {
                    sink.insert_same(kind, &record)?;
                }
            }
            let created = timestamp(&row["dateAdded"], timezone, &mut report).unwrap_or(0);
            let mut v = track_value(
                &lid,
                "lexicon",
                Some(&source_id),
                Some(&row["id"].to_string()),
                created,
            );
            v["updated_at_ms"] =
                json!(timestamp(&row["dateModified"], timezone, &mut report).unwrap_or(created));
            let incoming = row["incoming"].as_i64().context("Invalid incoming flag")?;
            let archived = row["archived"].as_i64().context("Invalid archived flag")?;
            ensure!(
                [0, 1].contains(&incoming) && [0, 1].contains(&archived),
                "Invalid track boolean"
            );
            v["state"] = json!(if incoming == 1 { "incoming" } else { "ready" });
            v["archived"] = json!(archived == 1);
            v["archived_at_ms"] = json!(timestamp(&row["archivedSince"], timezone, &mut report));
            for (from, to) in [
                ("title", "title"),
                ("artist", "artist"),
                ("albumTitle", "album"),
                ("genre", "genre"),
                ("comment", "comment"),
                ("remixer", "remixer"),
                ("mix", "mix"),
                ("label", "label"),
                ("composer", "composer"),
                ("producer", "producer"),
                ("grouping", "grouping"),
                ("lyricist", "lyricist"),
            ] {
                let text = string(&row[from])?;
                v["metadata"][to] = if text.is_empty() {
                    json!([])
                } else {
                    json!([text])
                };
            }
            for (from, to) in [
                ("rating", "rating"),
                ("playCount", "play_count"),
                ("year", "release_year"),
                ("trackNumber", "track_number"),
            ] {
                let n = row[from].as_i64().context("Invalid integer attribute")?;
                v["attributes"][to] = if n >= 0
                    && (from != "rating" || n <= 5)
                    && (from == "rating" || from == "playCount" || n > 0)
                {
                    json!(n)
                } else {
                    report
                        .warnings
                        .push(format!("Invalid {from} retained as source evidence"));
                    Value::Null
                };
            }
            for (from, to) in [
                ("color", "color"),
                ("extra1", "extra1"),
                ("extra2", "extra2"),
                ("releaseDate", "release_date"),
            ] {
                let text = row[from].as_str();
                v["attributes"][to] = json!(text.filter(|s| !s.is_empty()));
            }
            v["attributes"]["last_played_at_ms"] =
                json!(timestamp(&row["lastPlayed"], timezone, &mut report));
            v["content"]["size_bytes"] = if row["sizeBytes"].as_u64().is_some_and(|n| n > 0) {
                row["sizeBytes"].clone()
            } else {
                Value::Null
            };
            v["media"]["sample_rate"] = optional_positive(&row["sampleRate"])?;
            v["media"]["bitrate_kbps"] = optional_positive(&row["bitrate"])?;
            let mid = id(&ns, "MusicalRevision", &row["id"])?;
            v["selected_musical_revision_id"] = json!(mid);
            let energy = row["energy"].as_u64().filter(|n| (1..=10).contains(n));
            let duration = number(&row["duration"])?;
            let key = string(&row["key"])?;
            let normalized_key = key_value((!key.is_empty()).then_some(key));
            if !key.is_empty() && normalized_key["pitch_class"].is_null() {
                report.warnings.push(
                    "Unrecognized source key retained as text; harmonic matching is unavailable."
                        .into(),
                );
            }
            let mv = json!({"track_id":tid,"origin":"lexicon","created_at_ms":created,"selected":true,"key":normalized_key,"bpm":optional_positive(&row["bpm"])? ,"energy":energy,"duration_seconds":if duration>0.0{Some(duration)}else{None},"cues":[],"tempo_markers":[],"source_revision":"26"});
            sink.insert("musical_revisions", &LogicalRecord::new(mid, mv))?;
            let mut record = LogicalRecord::new(tid, v);
            let mut evidence = row.clone();
            evidence["data"] = source_object(&row["data"], l)?;
            record
                .extensions
                .insert("com.lexicondj.track".into(), evidence);
            sink.insert("tracks", &record)?;
            bound_warnings(&mut report, &mut omitted);
            Ok(())
        },
    )?;
    report.counts.insert("Track".into(), count);
    for (table, field) in [("Cuepoint", "cues"), ("Tempomarker", "tempo_markers")] {
        let mut current: Option<LogicalRecord> = None;
        let mut child_bytes = 0usize;
        let count = visit_rows(
            (&conn, &p),
            table,
            "trackId,startTime,id",
            l,
            &progress_clock,
            control,
            |row| {
                let mid = id(&ns, "MusicalRevision", &row["trackId"])?;
                if current.as_ref().is_none_or(|r| r.id != mid) {
                    if let Some(record) = current.take() {
                        save_musical(&sink, record)?;
                    }
                    current = Some(
                        sink.get("musical_revisions", &mid)?
                            .context("Orphan musical source record")?,
                    );
                    child_bytes = serde_json::to_vec(current.as_ref().unwrap())?.len();
                }
                let child = if table == "Cuepoint" {
                    let kind = match string(&row["type"])? {
                        "1" => "cue",
                        "2" => "fade_in",
                        "3" => "fade_out",
                        "4" => "load",
                        "5" => "loop",
                        _ => "unknown",
                    };
                    let start = number(&row["startTime"])?;
                    let end = if row["endTime"].is_null() {
                        None
                    } else {
                        Some(number(&row["endTime"])?)
                    };
                    let mut source = source_object(&row["data"], l)?;
                    let position = row["position"].as_i64().context("Invalid cue position")?;
                    let active = if row["activeLoop"].is_null() {
                        false
                    } else {
                        let n = row["activeLoop"].as_i64().context("Invalid activeLoop")?;
                        ensure!([0, 1].contains(&n), "Invalid activeLoop flag");
                        n == 1
                    };
                    let valid_loop = kind == "loop" && end.is_some_and(|e| e > start);
                    if kind == "unknown" || kind == "loop" && !valid_loop {
                        report
                            .warnings
                            .push("Unsupported cue kind or invalid loop retained inert".into());
                    }
                    if position < 0 {
                        source["source_position"] = json!(position);
                    }
                    let cue = json!({"id":id(&ns,"Cuepoint",&row["id"])? ,"kind":kind,"start_seconds":start,"end_seconds":end,"slot":u32::try_from(position).ok(),"memory":source["rekordbox"]["originalNum"]==-1,"name":row["name"],"color":row["color"],"active_loop":active&&valid_loop,"source_data":source});
                    cue
                } else {
                    let bpm = number(&row["bpm"])?;
                    ensure!(bpm > 0., "Invalid source tempo");
                    json!({"id":id(&ns,"Tempomarker",&row["id"])? ,"start_seconds":number(&row["startTime"])? ,"bpm":bpm,"beat_number":null,"beats_per_bar":null,"source_data":source_object(&row["data"],l)?})
                };
                child_bytes = child_bytes.saturating_add(serde_json::to_vec(&child)?.len());
                ensure!(child_bytes <= l.record_bytes, "Musical record byte limit");
                let children = current.as_mut().unwrap().value[field]
                    .as_array_mut()
                    .unwrap();
                ensure!(
                    children.len() < l.per_track,
                    "Per-track musical marker limit"
                );
                children.push(child);
                bound_warnings(&mut report, &mut omitted);
                Ok(())
            },
        )?;
        if let Some(record) = current {
            save_musical(&sink, record)?;
        }
        report.counts.insert(table.into(), count);
    }
    let mut parent_key = None;
    let mut position = 0u64;
    let count = visit_rows(
        (&conn, &p),
        "Playlist",
        "parentId,position,id",
        l,
        &progress_clock,
        control,
        |row| {
            let pid = id(&ns, "Playlist", &row["id"])?;
            let kind = match string(&row["type"])? {
                "1" => "folder",
                "2" => "playlist",
                "3" => "smartlist",
                _ => bail!("Unsupported required playlist type"),
            };
            let parent = if row["parentId"].is_null() {
                None
            } else {
                Some(id(&ns, "Playlist", &row["parentId"])?)
            };
            if parent_key.as_ref() != Some(&parent) {
                parent_key = Some(parent.clone());
                position = 0;
            }
            let pos = row["position"]
                .as_i64()
                .context("Invalid playlist position")?;
            let raw = if row["smartlist"].is_null() {
                Value::Null
            } else {
                source_object(&row["smartlist"], l)?
            };
            let definition = if kind == "smartlist" {
                smart(&row["smartlist"], &pid, &mut report, l)?
            } else {
                None
            };
            let mut record = LogicalRecord::new(
                &pid,
                json!({"parent_id":parent,"kind":kind,"name":string(&row["name"])? ,"position":position,"favorite_position":null,"deleted":false,"deleted_by":null,"definition":definition,"source_definition":raw,"definition_status":if kind=="smartlist"&&definition.is_none(){"unsupported"}else{"active"}}),
            );
            record.extensions.insert("com.lexicondj.playlist".into(),json!({"position":pos,"folderType":row["folderType"],"dateAdded":row["dateAdded"],"dateModified":row["dateModified"],"data":source_object(&row["data"],l)?}));
            sink.insert("playlists", &record)?;
            position += 1;
            bound_warnings(&mut report, &mut omitted);
            Ok(())
        },
    )?;
    report.counts.insert("Playlist".into(), count);
    let mut playlist: Option<LogicalRecord> = None;
    let mut position = 0u64;
    let mut snapshot_bytes = 0usize;
    let count = visit_rows(
        (&conn, &p),
        "LinkTrackPlaylist",
        "playlistId,position,id",
        l,
        &progress_clock,
        control,
        |row| {
            let pid = id(&ns, "Playlist", &row["playlistId"])?;
            let tid = id(&ns, "Track", &row["trackId"])?;
            ensure!(sink.exists("tracks", &tid)?, "Orphan playlist track");
            if playlist.as_ref().is_none_or(|r| r.id != pid) {
                if let Some(record) = playlist.take()
                    && record.value["kind"] == "smartlist"
                {
                    sink.replace("playlists", &record)?;
                }
                playlist = Some(
                    sink.get("playlists", &pid)?
                        .context("Orphan playlist relation")?,
                );
                position = 0;
                snapshot_bytes = serde_json::to_vec(playlist.as_ref().unwrap())?.len();
            }
            let record = playlist.as_mut().unwrap();
            match record.value["kind"].as_str().unwrap() {
                "folder" => bail!("Folder contains track relation"),
                "smartlist" => {
                    let value = json!({"track_id":tid,"position":row["position"]});
                    snapshot_bytes =
                        snapshot_bytes.saturating_add(serde_json::to_vec(&value)?.len());
                    ensure!(
                        snapshot_bytes <= l.record_bytes,
                        "Smart membership snapshot record byte limit"
                    );
                    record
                        .extensions
                        .entry("com.lexicondj.membership_snapshot")
                        .or_insert_with(|| json!([]))
                        .as_array_mut()
                        .unwrap()
                        .push(value);
                }
                _ => {
                    if sink.conn.execute(
                        "INSERT OR IGNORE INTO membership_seen VALUES(?1,?2)",
                        params![pid, tid],
                    )? == 0
                    {
                        report.warnings.push(
                            "Repeated playlist membership collapsed to first occurrence".into(),
                        );
                    } else {
                        sink.insert(
                            "playlist_members",
                            &LogicalRecord::new(
                                id(&ns, "LinkTrackPlaylist", &row["id"])?,
                                json!({"playlist_id":pid,"track_id":tid,"position":position}),
                            ),
                        )?;
                        position += 1;
                    }
                }
            };
            bound_warnings(&mut report, &mut omitted);
            Ok(())
        },
    )?;
    if let Some(record) = playlist
        && record.value["kind"] == "smartlist"
    {
        sink.replace("playlists", &record)?;
    }
    report.counts.insert("LinkTrackPlaylist".into(), count);
    let count = visit_rows(
        (&conn, &p),
        "TagCategory",
        "id",
        l,
        &progress_clock,
        control,
        |row| {
            sink.insert("tag_categories",&LogicalRecord::new(id(&ns,"TagCategory",&row["id"])? ,json!({"name":string(&row["label"])? ,"position":row["position"].as_i64().context("Invalid category position")?,"color":row["color"]})))
        },
    )?;
    report.counts.insert("TagCategory".into(), count);
    let count = visit_rows(
        (&conn, &p),
        "Tag",
        "id",
        l,
        &progress_clock,
        control,
        |row| {
            sink.insert("tags",&LogicalRecord::new(id(&ns,"Tag",&row["id"])? ,json!({"category_id":id(&ns,"TagCategory",&row["categoryId"])? ,"name":string(&row["label"])? ,"position":row["position"].as_i64().context("Invalid tag position")?,"shortcut":row["shortcut"]})))
        },
    )?;
    report.counts.insert("Tag".into(), count);
    let count = visit_rows(
        (&conn, &p),
        "LinkTagTrack",
        "id",
        l,
        &progress_clock,
        control,
        |row| {
            let track = id(&ns, "Track", &row["trackId"])?;
            let tag = id(&ns, "Tag", &row["tagId"])?;
            if sink.conn.execute(
                "INSERT OR IGNORE INTO tag_seen VALUES(?1,?2)",
                params![track, tag],
            )? == 0
            {
                report
                    .warnings
                    .push("Repeated custom tag assignment collapsed".into());
            } else {
                sink.insert(
                    "track_tags",
                    &LogicalRecord::new(
                        id(&ns, "LinkTagTrack", &row["id"])?,
                        json!({"track_id":track,"tag_id":tag}),
                    ),
                )?;
            };
            bound_warnings(&mut report, &mut omitted);
            Ok(())
        },
    )?;
    report.counts.insert("LinkTagTrack".into(), count);
    // Never read Setting.data. Report bounded key samples while retaining the exact count.
    progress_clock.renew();
    let mut setting_stmt = conn.prepare("SELECT feature,name FROM Setting ORDER BY id")?;
    let mut settings = setting_stmt.query([])?;
    let mut count = 0u64;
    let mut keys = vec![];
    let mut key_bytes = 0usize;
    while let Some(row) = settings.next()? {
        control("settings", count)?;
        progress_clock.renew();
        ensure!(count < l.relations as u64, "Setting row limit");
        let feature: String = row.get(0)?;
        let name: String = row.get(1)?;
        let key = json!({"feature":feature,"name":name});
        let bytes = serde_json::to_vec(&key)?.len();
        if keys.len() < 512 && key_bytes.saturating_add(bytes) < l.manifest_bytes as usize / 4 {
            key_bytes += bytes;
            keys.push(key);
        }
        count += 1;
    }
    report.counts.insert("Setting".into(), count);
    let truncated = count > keys.len() as u64;
    let mut exclusion =
        json!({"domain":"Setting","reason":"no_validated_safe_adapter","count":count,"keys":keys});
    if truncated {
        exclusion["keys_truncated"] = true.into();
    }
    report.excluded.push(exclusion);
    drop(settings);
    drop(setting_stmt);
    for table in [
        "Waveform",
        "AlbumartPreview",
        "HistorySession",
        "HistoryTrack",
        "ChartItem",
        "CloudFile",
    ] {
        control("source_history", 0)?;
        progress_clock.renew();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |r| r.get(0),
        )?;
        if !exists {
            continue;
        }
        let count: u64 = conn.query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |r| {
            r.get(0)
        })?;
        if table.starts_with("History") {
            let mut source = sink.get("import_sources", &source_id)?.unwrap();
            let history = bounded_history(
                &conn,
                table,
                l,
                &progress_clock,
                control,
                serde_json::to_vec(&source)?.len(),
            )?;
            source
                .extensions
                .entry("com.lexicondj.history")
                .or_insert_with(|| json!({}))[table] = Value::Array(history);
            sink.replace("import_sources", &source)?;
            report.counts.insert(table.into(), count);
            if count > 0 {
                report.warnings.push(format!(
                    "{table} preserved as inert history; playback-history UI is unavailable"
                ));
            }
        } else {
            report
                .excluded
                .push(json!({"domain":table,"reason":"cache_or_operational","count":count}));
        }
    }
    sink.conn.execute_batch("COMMIT")?;
    drop(sink);
    if omitted > 0 {
        report.warnings.push(format!(
            "{omitted} further compatibility warnings omitted from this bounded report."
        ));
    }
    archive.manifest.excluded.extend(report.excluded.clone());
    ensure!(
        serde_json::to_vec(&archive.manifest)?.len() as u64 <= l.manifest_bytes,
        "Import manifest byte limit"
    );
    archive.validate(l, control)?;
    Ok(ImportPreviewSpooled { archive, report })
}

fn bounded_history(
    conn: &Connection,
    table: &str,
    l: &ArchiveLimits,
    clock: &QueryClock,
    control: &dyn Fn(&str, u64) -> Result<()>,
    mut bytes: usize,
) -> Result<Vec<Value>> {
    let mut info = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let names = info
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(names.len() <= 128, "History column limit");
    let sql = format!(
        "SELECT {} FROM \"{table}\"",
        names
            .iter()
            .map(|n| format!("\"{}\"", n.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(",")
    );
    clock.renew();
    let mut statement = conn.prepare(&sql)?;
    let mut rows = statement.query([])?;
    let mut result = vec![];
    while let Some(row) = rows.next()? {
        control("source_history", result.len() as u64)?;
        clock.renew();
        ensure!(result.len() < l.relations, "History row limit");
        let mut object = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => {
                    if name.to_ascii_lowercase().ends_with("id") {
                        json!(n.to_string())
                    } else {
                        json!(n)
                    }
                }
                ValueRef::Real(n) => {
                    ensure!(n.is_finite(), "Nonfinite history value");
                    json!(n)
                }
                ValueRef::Text(s) => {
                    ensure!(s.len() <= l.record_bytes, "History text limit");
                    json!(std::str::from_utf8(s)?)
                }
                ValueRef::Blob(s) => {
                    use base64::{Engine, engine::general_purpose::STANDARD};
                    ensure!(s.len() <= l.record_bytes, "History blob limit");
                    json!({"encoding":"base64","data":STANDARD.encode(s)})
                }
            };
            object.insert(name.clone(), value);
        }
        let value = Value::Object(object);
        bytes = bytes.saturating_add(serde_json::to_vec(&value)?.len());
        ensure!(
            bytes <= l.record_bytes,
            "Inert history exceeds logical record limit"
        );
        result.push(value);
    }
    Ok(result)
}

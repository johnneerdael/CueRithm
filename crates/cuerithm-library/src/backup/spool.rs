//! Disk-backed logical records. Large archives never require a library-sized JSON tree.
use super::{container::open_zip, *};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};
#[derive(Debug)]
struct Storage {
    // Close the cached handle before removing task storage on Windows.
    lookup: Mutex<Connection>,
    directory: tempfile::TempDir,
    path: PathBuf,
}
#[derive(Debug, Clone)]
pub struct SpoolArchive {
    pub manifest: Manifest,
    storage: Arc<Storage>,
}
impl SpoolArchive {
    pub fn create(manifest: Manifest) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("records.sqlite");
        let conn = Connection::open(&path)?;
        conn.execute_batch(PRAGMA_RECORDS_SQL)?;
        conn.execute_batch("PRAGMA cache_size=-2048;PRAGMA temp_store=FILE;")?;
        conn.execute_batch(r#"
            CREATE INDEX record_selected ON records(kind,json_extract(json,'$.value.selected'),json_extract(json,'$.value.track_id'));
            CREATE INDEX record_native_cue_history ON records(json_extract(json,'$.value.track_id'),json_extract(json,'$.value.created_at_ms'),json_extract(json,'$.extensions."com.cuerithm.source_order"'),json_extract(json,'$.value.selected'),id)
            WHERE kind='musical_revisions' AND json_extract(json,'$.extensions."com.cuerithm.cue_provenance"') IS NOT NULL;
        "#)?;
        Ok(Self {
            manifest,
            storage: Arc::new(Storage {
                lookup: Mutex::new(conn),
                directory,
                path,
            }),
        })
    }
    pub(crate) fn connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.storage.path)?;
        conn.execute_batch(
            "PRAGMA cache_size=-2048;PRAGMA synchronous=OFF;PRAGMA temp_store=FILE;",
        )?;
        Ok(conn)
    }
    fn lookup(&self) -> Result<MutexGuard<'_, Connection>> {
        self.storage
            .lookup
            .lock()
            .map_err(|_| anyhow::anyhow!("Archive lookup interrupted"))
    }
    pub fn insert(&self, kind: &str, record: &LogicalRecord) -> Result<()> {
        self.lookup()?.execute(
            "INSERT INTO records VALUES(?1,?2,?3)",
            params![kind, record.id, serde_json::to_string(record)?],
        )?;
        Ok(())
    }
    pub fn upsert(&self, kind: &str, record: &LogicalRecord) -> Result<()> {
        self.lookup()?.execute(
            "INSERT OR REPLACE INTO records VALUES(?1,?2,?3)",
            params![kind, record.id, serde_json::to_string(record)?],
        )?;
        Ok(())
    }
    pub fn count(&self, kind: &str) -> Result<u64> {
        Ok(self
            .lookup()?
            .prepare_cached("SELECT COUNT(*) FROM records WHERE kind=?1")?
            .query_row([kind], |r| r.get(0))?)
    }
    pub fn record(&self, kind: &str, id: &str) -> Result<Option<LogicalRecord>> {
        let text: Option<String> = {
            let connection = self.lookup()?;
            connection
                .prepare_cached("SELECT json FROM records WHERE kind=?1 AND id=?2")?
                .query_row(params![kind, id], |r| r.get(0))
                .optional()?
        };
        text.map(|s| serde_json::from_str(&s).map_err(Into::into))
            .transpose()
    }
    pub fn for_each(
        &self,
        kind: &str,
        callback: &mut dyn FnMut(LogicalRecord) -> Result<()>,
    ) -> Result<()> {
        let conn = self.connection()?;
        let mut stmt =
            conn.prepare("SELECT json FROM records WHERE kind=?1 ORDER BY id COLLATE BINARY")?;
        let mut rows = stmt.query([kind])?;
        while let Some(row) = rows.next()? {
            let text: String = row.get(0)?;
            callback(serde_json::from_str(&text)?)?;
        }
        Ok(())
    }
    pub fn validate(
        &self,
        l: &ArchiveLimits,
        control: &dyn Fn(&str, u64) -> Result<()>,
    ) -> Result<()> {
        let mut header = LogicalArchive::empty("UTC")?;
        header.manifest = self.manifest.clone();
        validate_logical_archive(&header, l)?;
        let conn = self.connection()?;
        let mut seen = 0u64;
        for (kind, _) in KINDS {
            let count = self.count(kind)?;
            ensure!(
                count
                    <= if *kind == "tracks" {
                        l.tracks as u64
                    } else {
                        l.relations as u64
                    },
                "Spool record count limit"
            );
            self.for_each(kind, &mut |r| {
                control("validate", seen)?;
                seen += 1;
                super::validate::validate_record_from_lookup(kind, &r, l, &|kind, id| {
                    record_from_connection(&conn, kind, id)
                })
            })?;
        }
        ensure!(conn.query_row("SELECT COUNT(*) FROM records WHERE kind='settings' AND json_extract(json,'$.value.selected')=1",[],|r|r.get::<_,i64>(0))?==1,"Exactly one selected settings revision required");
        ensure!(
            self.count("preferences")? == 1
                && record_from_connection(&conn, "preferences", "library")?.is_some(),
            "Exactly one library preferences required"
        );
        for kind in ["musical_revisions", "analysis_revisions"] {
            control("validate_summary", seen)?;
            let duplicate: bool = conn.query_row(SELECT_RECORDS_SQL, [kind], |r| r.get(0))?;
            ensure!(!duplicate, "Multiple selected revisions");
        }
        self.for_each("tracks", &mut |track| {
            control("validate_selection", seen)?;
            seen += 1;
            for (kind, field) in [
                ("musical_revisions", "selected_musical_revision_id"),
                ("analysis_revisions", "selected_analysis_revision_id"),
            ] {
                if let Some(id) = track.value[field].as_str() {
                    let r = record_from_connection(&conn, kind, id)?
                        .context("Missing selected revision")?;
                    ensure!(
                        r.value["track_id"] == track.id && r.value["selected"] == true,
                        "Selected revision mismatch"
                    );
                } else {
                    let selected: bool =
                        conn.query_row(SELECT_RECORDS_SQL_2, params![kind, track.id], |r| {
                            r.get(0)
                        })?;
                    ensure!(!selected, "Unreferenced selected revision");
                }
            }
            Ok(())
        })?;
        for (kind, fields) in [
            ("playlist_members", ("playlist_id", "track_id")),
            ("playlist_members", ("playlist_id", "position")),
            ("track_tags", ("track_id", "tag_id")),
        ] {
            control("validate_relationships", seen)?;
            let sql = format!(
                "SELECT EXISTS(SELECT 1 FROM records WHERE kind=?1 GROUP BY json_extract(json,'$.value.{}'),json_extract(json,'$.value.{}') HAVING COUNT(*)>1)",
                fields.0, fields.1
            );
            ensure!(
                !conn.query_row(&sql, [kind], |r| r.get::<_, bool>(0))?,
                "Duplicate domain relationship/position"
            );
        }
        conn.execute_batch("CREATE TEMP TABLE validated_playlists(id TEXT PRIMARY KEY)")?;
        self.for_each("playlists", &mut |p| {
            control("validate_tree", seen)?;
            seen += 1;
            let mut current = p;
            let mut chain = std::collections::BTreeSet::new();
            while let Some(parent) = current.value["parent_id"].as_str() {
                control("validate_tree", seen)?;
                if conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM validated_playlists WHERE id=?1)",
                    [&current.id],
                    |r| r.get::<_, bool>(0),
                )? {
                    break;
                }
                ensure!(
                    chain.len() as u64 * 256 < l.memory_bytes,
                    "Playlist depth memory limit"
                );
                ensure!(chain.insert(current.id.clone()), "Playlist parent cycle");
                let parent = self
                    .record("playlists", parent)?
                    .context("Missing playlist parent")?;
                ensure!(
                    parent.value["kind"] == "folder",
                    "Playlist parent must be folder"
                );
                current = parent;
            }
            for id in chain {
                conn.execute("INSERT OR IGNORE INTO validated_playlists VALUES(?1)", [id])?;
            }
            Ok(())
        })?;
        Ok(())
    }
    pub(crate) fn temp_path(&self) -> &Path {
        self.storage.directory.path()
    }
}
pub fn read_portable_spooled(
    path: &Path,
    l: &ArchiveLimits,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<SpoolArchive> {
    let mut zip = open_zip(path, l)?;
    let manifest: Manifest = {
        let file = zip.by_name("manifest.json")?;
        ensure!(file.size() <= l.manifest_bytes, "Manifest limit");
        let mut bytes = vec![];
        file.take(l.manifest_bytes + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= l.manifest_bytes,
            "Manifest actual byte limit"
        );
        serde_json::from_value(parse_strict_json(&bytes, l.depth)?)?
    };
    let spool = SpoolArchive::create(manifest)?;
    let mut names = std::collections::BTreeSet::from(["manifest.json".to_string()]);
    let mut kinds = std::collections::BTreeSet::new();
    let mut conn = spool.connection()?;
    let tx = conn.transaction()?;
    let mut total = 0u64;
    for descriptor in &spool.manifest.entries {
        ensure!(
            names.insert(descriptor.name.clone()) && kinds.insert(descriptor.kind.clone()),
            "Duplicate manifest payload"
        );
        super::validate::digest(&descriptor.sha256)?;
        ensure!(
            descriptor.schema == "cuerithm-logical-v1",
            "Unsupported payload schema"
        );
        let known = KINDS.iter().find(|(kind, _)| *kind == descriptor.kind);
        if let Some((_, name)) = known {
            ensure!(
                *name == descriptor.name && descriptor.required,
                "Invalid required descriptor"
            );
        } else {
            ensure!(!descriptor.required, "Unknown required payload");
        }
        let file = zip.by_name(&descriptor.name)?;
        ensure!(
            file.size() == descriptor.bytes && file.size() <= l.entry_bytes,
            "Payload length mismatch"
        );
        let mut reader = BufReader::new(file);
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut count = 0u64;
        loop {
            control("read", total)?;
            let mut line = vec![];
            let n = reader
                .by_ref()
                .take(l.record_bytes as u64 + 1)
                .read_until(b'\n', &mut line)?;
            if n == 0 {
                break;
            }
            ensure!(
                n <= l.record_bytes && line.len() > 1 && line.last() == Some(&b'\n'),
                "Record size/LF limit"
            );
            hash.update(&line);
            bytes += n as u64;
            total += n as u64;
            ensure!(
                bytes <= descriptor.bytes && total <= l.expanded_bytes,
                "Actual expanded size limit"
            );
            count += 1;
            ensure!(
                count
                    <= if descriptor.kind == "tracks" {
                        l.tracks as u64
                    } else {
                        l.relations as u64
                    },
                "Record count limit"
            );
            if known.is_some() {
                let record: LogicalRecord =
                    serde_json::from_value(parse_strict_json(&line[..n - 1], l.depth)?)?;
                tx.execute(
                    "INSERT INTO records VALUES(?1,?2,?3)",
                    params![
                        descriptor.kind,
                        record.id,
                        std::str::from_utf8(&line[..n - 1])?
                    ],
                )?;
            } else {
                parse_strict_json(&line[..n - 1], l.depth)?;
                tx.execute(
                    "INSERT INTO optional_lines VALUES(?1,?2,?3)",
                    params![descriptor.name, count, &line],
                )?;
            }
            if count.is_multiple_of(1024) {
                ensure!(
                    fs2::available_space(spool.temp_path())? >= 256 << 20,
                    "Insufficient staging disk reserve"
                );
            }
        }
        ensure!(
            bytes == descriptor.bytes
                && count == descriptor.records
                && format!("{:x}", hash.finalize()) == descriptor.sha256,
            "Payload digest/count mismatch"
        );
    }
    ensure!(names.len() == zip.len(), "Unexpected archive entries");
    for index in 0..zip.len() {
        ensure!(
            names.contains(zip.by_index(index)?.name()?.as_ref()),
            "Unlisted archive entry"
        );
    }
    for (kind, _) in KINDS {
        ensure!(kinds.contains(*kind), "Missing required payload");
    }
    tx.commit()?;
    drop(conn);
    spool.validate(l, control)?;
    Ok(spool)
}
pub fn write_portable_spooled(
    path: &Path,
    a: &SpoolArchive,
    l: &ArchiveLimits,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<Manifest> {
    a.validate(l, control)?;
    ensure!(!path.exists(), "Backup destination exists");
    let parent = path.parent().unwrap_or(Path::new("."));
    ensure!(
        fs2::available_space(parent)? >= 256 << 20,
        "Insufficient output disk reserve"
    );
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut manifest = a.manifest.clone();
    manifest.archive_id = uuid::Uuid::new_v4().to_string();
    manifest.entries.clear();
    {
        let mut writer = ZipWriter::new(temporary.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .large_file(true);
        let mut total = 0u64;
        for (kind, name) in KINDS {
            writer.start_file(*name, options)?;
            let mut hash = Sha256::new();
            let mut bytes = 0u64;
            let mut records = 0u64;
            a.for_each(kind, &mut |r| {
                control("write", total)?;
                let mut line = serde_json::to_vec(&r)?;
                line.push(b'\n');
                ensure!(line.len() <= l.record_bytes, "Record size limit");
                bytes += line.len() as u64;
                total += line.len() as u64;
                records += 1;
                ensure!(
                    bytes <= l.entry_bytes && total <= l.expanded_bytes,
                    "Output expanded limit"
                );
                hash.update(&line);
                writer.write_all(&line)?;
                if records.is_multiple_of(1024) {
                    ensure!(
                        fs2::available_space(parent)? >= 256 << 20,
                        "Insufficient output disk reserve"
                    );
                }
                Ok(())
            })?;
            manifest.entries.push(EntryDescriptor {
                name: name.to_string(),
                kind: kind.to_string(),
                bytes,
                records,
                sha256: format!("{:x}", hash.finalize()),
                schema: "cuerithm-logical-v1".into(),
                required: true,
            });
        }
        let conn = a.connection()?;
        for descriptor in a
            .manifest
            .entries
            .iter()
            .filter(|d| !d.required && !KINDS.iter().any(|(k, _)| *k == d.kind))
        {
            writer.start_file(&descriptor.name, options)?;
            let mut q =
                conn.prepare("SELECT line FROM optional_lines WHERE name=?1 ORDER BY ordinal")?;
            for row in q.query_map([&descriptor.name], |r| r.get::<_, Vec<u8>>(0))? {
                control("write_optional", total)?;
                let line = row?;
                total += line.len() as u64;
                ensure!(total <= l.expanded_bytes, "Optional payload expanded limit");
                writer.write_all(&line)?;
            }
            manifest.entries.push(descriptor.clone());
        }
        let bytes = serde_json::to_vec(&manifest)?;
        ensure!(bytes.len() as u64 <= l.manifest_bytes, "Manifest limit");
        writer.start_file("manifest.json", options)?;
        writer.write_all(&bytes)?;
        writer.finish()?;
    }
    temporary.as_file().sync_all()?;
    read_portable_spooled(temporary.path(), l, control)?;
    control("publish", 0)?;
    temporary.persist_noclobber(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    std::fs::File::open(parent)?.sync_all()?;
    Ok(manifest)
}
impl SpoolArchive {
    pub fn retain_selected_history(&self) -> Result<()> {
        self.connection()?.execute(DELETE_RECORDS_SQL, [])?;
        Ok(())
    }
    pub(crate) fn copy_records(&self, a: &LogicalArchive, kinds: &[&str]) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        {
            let mut insert = tx.prepare("INSERT OR REPLACE INTO records VALUES(?1,?2,?3)")?;
            for kind in kinds {
                for r in &a.records[*kind] {
                    insert.execute(params![kind, r.id, serde_json::to_string(r)?])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
}
impl crate::Library {
    pub fn portable_snapshot_spooled(
        &self,
        l: &ArchiveLimits,
        control: &dyn Fn(&str, u64) -> Result<()>,
    ) -> Result<SpoolArchive> {
        let scratch = tempfile::tempdir_in(self.data_dir())?;
        let mut snapshot = Connection::open(scratch.path().join("snapshot.sqlite"))?;
        {
            let source = self.conn()?;
            crate::library_backup::quiet_database(&source)?;
            source.execute(
                "INSERT OR IGNORE INTO portable_library_meta VALUES('library_id',?1)",
                [uuid::Uuid::new_v4().to_string()],
            )?;
            let backup = rusqlite::backup::Backup::new(&source, &mut snapshot)?;
            loop {
                control("snapshot", 0)?;
                ensure!(
                    fs2::available_space(scratch.path())? >= 256 << 20,
                    "Insufficient snapshot disk reserve"
                );
                match backup.step(256)? {
                    rusqlite::backup::StepResult::Done => break,
                    rusqlite::backup::StepResult::More => {}
                    _ => anyhow::bail!("SQLite snapshot busy; retry after mutations stop"),
                }
            }
        }
        super::projection::prepare_native_root_projection(&snapshot, control)?;
        snapshot.execute_batch(CREATE_PORTABLE_PAGE_IDS_SQL)?;
        let mut spool = SpoolArchive::create(LogicalArchive::empty("UTC")?.manifest)?;
        let mut cursor = String::new();
        let mut first = true;
        let mut count = 0u64;
        loop {
            let mut query = snapshot.prepare(SELECT_ACCEPTED_MUSICAL_REVISIONS_SQL)?;
            let mut rows = query.query([&cursor])?;
            let mut ids = vec![];
            let mut raw_bytes = 0u64;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let bytes = row.get::<_, u64>(1)?.saturating_add(4096);
                ensure!(
                    bytes <= l.record_bytes as u64 * 4,
                    "One track history exceeds bounded snapshot page"
                );
                if !ids.is_empty() && raw_bytes + bytes > l.memory_bytes / 64 {
                    break;
                }
                raw_bytes += bytes;
                ids.push(id);
            }
            drop(rows);
            drop(query);
            if ids.is_empty() && !first {
                break;
            }
            snapshot.execute("DELETE FROM portable_page_ids", [])?;
            for id in &ids {
                snapshot.execute("INSERT INTO portable_page_ids VALUES(?1)", [id])?;
            }
            control("project", count)?;
            let page = loop {
                match super::projection::snapshot_connection(&snapshot, control, true, false, false)
                {
                    Ok(page) => break page,
                    Err(error)
                        if error
                            .downcast_ref::<super::projection::SnapshotPageLimit>()
                            .is_some()
                            && ids.len() > 1 =>
                    {
                        ids.truncate(ids.len() / 2);
                        snapshot.execute("DELETE FROM portable_page_ids", [])?;
                        for id in &ids {
                            snapshot.execute("INSERT INTO portable_page_ids VALUES(?1)", [id])?;
                        }
                        control("resize_snapshot_page", count)?;
                    }
                    Err(error) => return Err(error),
                }
            };
            if first {
                spool.manifest = page.manifest.clone();
                spool.copy_records(&page, &["settings", "preferences"])?;
                first = false;
            }
            spool.copy_records(
                &page,
                &[
                    "roots",
                    "locators",
                    "tracks",
                    "musical_revisions",
                    "analysis_revisions",
                ],
            )?;
            count += ids.len() as u64;
            ensure!(count <= l.tracks as u64, "Track count limit");
            if let Some(last) = ids.last() {
                cursor = last.clone();
            } else {
                break;
            }
        }
        snapshot.execute_batch("DROP VIEW temp.tracks;DROP TABLE temp.portable_page_ids;")?;
        project_shared_spooled(&snapshot, &mut spool, l, control)?;
        spool.validate(l, control)?;
        Ok(spool)
    }
}
fn saved_record(conn: &Connection, kind: &str, r: &LogicalRecord) -> Result<Option<LogicalRecord>> {
    let found: Option<String> = conn
        .query_row(
            "SELECT json FROM portable_domain_records WHERE kind=?1 AND id=?2",
            params![kind, r.id],
            |r| r.get(0),
        )
        .optional()?;
    let found = if found.is_some() {
        found
    } else if kind == "playlist_members" || kind == "track_tags" {
        let (left, right) = if kind == "playlist_members" {
            ("playlist_id", "track_id")
        } else {
            ("track_id", "tag_id")
        };
        let sql = format!(
            "SELECT json FROM portable_domain_records WHERE kind=?1 AND json_extract(json,'$.value.{left}')=?2 AND json_extract(json,'$.value.{right}')=?3 LIMIT 1"
        );
        conn.query_row(
            &sql,
            params![kind, r.value[left].as_str(), r.value[right].as_str()],
            |r| r.get(0),
        )
        .optional()?
    } else {
        None
    };
    found
        .map(|s| serde_json::from_str(&s).map_err(Into::into))
        .transpose()
}
fn project_shared_spooled(
    conn: &Connection,
    spool: &mut SpoolArchive,
    l: &ArchiveLimits,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<()> {
    conn.execute_batch(CREATE_SCHEMA_SQL)?;
    let target = spool.connection()?;
    let mut sink = spool.connection()?;
    let mut old = conn.prepare(SELECT_PORTABLE_DOMAIN_RECORDS_SQL)?;
    for row in old.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })? {
        let (kind, id, json) = row?;
        control("history", 0)?;
        target.execute(
            "INSERT OR IGNORE INTO records VALUES(?1,?2,?3)",
            params![kind, id, json],
        )?;
    }
    let domains = [
        "custom_tag_categories",
        "custom_tags",
        "track_custom_tags",
        "playlists",
        "playlist_members",
        "native_targets",
        "native_track_links",
        "sync_links",
    ];
    conn.execute_batch(CREATE_PORTABLE_SHARED_PAGE_SQL)?;
    for domain in domains {
        conn.execute_batch(&format!("CREATE TEMP VIEW {domain} AS SELECT * FROM main.{domain} WHERE (SELECT name FROM portable_shared_kind)='{domain}' AND rowid IN(SELECT rowid_value FROM portable_shared_page)"))?;
    }
    let empty: std::collections::BTreeMap<String, Vec<LogicalRecord>> =
        KINDS.iter().map(|(k, _)| (k.to_string(), vec![])).collect();
    for domain in domains {
        let mut cursor = 0i64;
        loop {
            let mut statement = conn.prepare(&format!(
                "SELECT rowid FROM main.{domain} WHERE rowid>?1 ORDER BY rowid LIMIT 1"
            ))?;
            let ids = statement
                .query_map([cursor], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if ids.is_empty() {
                break;
            }
            conn.execute("DELETE FROM portable_shared_page", [])?;
            for id in &ids {
                conn.execute("INSERT INTO portable_shared_page VALUES(?1)", [id])?;
            }
            conn.execute("UPDATE portable_shared_kind SET name=?1", [domain])?;
            control("project_shared", cursor as u64)?;
            let mut page = LogicalArchive::empty("UTC")?;
            if domain.starts_with("native_") || domain == "sync_links" {
                super::projection::project_external(conn, &mut page)?;
            } else {
                super::projection::project_tables(conn, &mut page, &empty)?;
            }
            let tx = sink.transaction()?;
            for (kind, _) in KINDS {
                if matches!(*kind, "settings" | "preferences") {
                    continue;
                }
                for live in &page.records[*kind] {
                    let mut r = if let Some(mut old) = saved_record(conn, kind, live)? {
                        let mut value = old.value.clone();
                        for (key, v) in live.value.as_object().context("Domain object")? {
                            if matches!(key.as_str(), "color" | "shortcut") && v.is_null() {
                                continue;
                            }
                            if key == "source_definition" && v.is_null() {
                                continue;
                            }
                            if key == "definition_status"
                                && old.value["definition_status"] == "unsupported"
                                && live.value["definition"].is_null()
                            {
                                continue;
                            }
                            value[key] = v.clone();
                        }
                        old.value = value;
                        old
                    } else {
                        live.clone()
                    };
                    if *kind == "external_links" {
                        r.value["binding_status"] = json!("inactive");
                    }
                    tx.execute(
                        "INSERT OR REPLACE INTO records VALUES(?1,?2,?3)",
                        params![kind, r.id, serde_json::to_string(&r)?],
                    )?;
                }
            }
            tx.commit()?;
            spool
                .manifest
                .excluded
                .extend(page.manifest.excluded.into_iter().filter(|v| {
                    v["domain"] != "music"
                        && v["domain"] != "credentials"
                        && v["domain"] != "operational_state"
                }));
            cursor = *ids.last().unwrap();
            ensure!(
                fs2::available_space(spool.temp_path())? >= 256 << 20,
                "Insufficient projected archive disk reserve"
            );
        }
    }
    for domain in domains {
        conn.execute_batch(&format!("DROP VIEW temp.{domain}"))?;
    }
    conn.execute_batch(
        "DROP TABLE temp.portable_shared_page;DROP TABLE temp.portable_shared_kind;",
    )?;
    spool.manifest.excluded.sort_by_key(Value::to_string);
    spool.manifest.excluded.dedup();
    let optional_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='portable_optional_lines')",
        [],
        |r| r.get(0),
    )?;
    if optional_exists {
        let mut q = conn.prepare(
            "SELECT name,ordinal,line FROM portable_optional_lines ORDER BY name,ordinal",
        )?;
        let mut insert = target.prepare("INSERT INTO optional_lines VALUES(?1,?2,?3)")?;
        for row in q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u64>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })? {
            let (name, ordinal, line) = row?;
            insert.execute(params![name, ordinal, line])?;
        }
    }
    ensure!(
        spool.count("playlists")? <= l.relations as u64,
        "Playlist count limit"
    );
    Ok(())
}
impl crate::Library {
    pub fn restore_spooled(
        &self,
        a: &SpoolArchive,
        l: &ArchiveLimits,
        control: &dyn Fn(&str, u64) -> Result<()>,
    ) -> Result<()> {
        a.validate(l, control)?;
        let mut conn = self.conn()?;
        ensure!(
            conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, i64>(0))? == 0,
            "Restore requires empty staging generation"
        );
        let tx = conn.transaction()?;
        let source = a.connection()?;
        let mut done = 0u64;
        a.for_each("tracks", &mut |record| {
            control("restore_tracks", done)?;
            done += 1;
            if done.is_multiple_of(1024) {
                ensure!(
                    fs2::available_space(self.data_dir())? >= 256 << 20,
                    "Insufficient restore disk reserve"
                );
            }
            let mut related: std::collections::BTreeMap<String, Vec<LogicalRecord>> =
                KINDS.iter().map(|(k, _)| (k.to_string(), vec![])).collect();
            let locator = record_from_connection(
                &source,
                "locators",
                record.value["locator_id"]
                    .as_str()
                    .context("Track locator")?,
            )?
            .context("Missing track locator")?;
            if let Some(root) = locator.value["root_id"].as_str() {
                related
                    .get_mut("roots")
                    .unwrap()
                    .push(record_from_connection(&source, "roots", root)?.context("Missing root")?);
            }
            related.get_mut("locators").unwrap().push(locator);
            if let Some(id) = record.value["selected_musical_revision_id"].as_str() {
                related.get_mut("musical_revisions").unwrap().push(
                    record_from_connection(&source, "musical_revisions", id)?
                        .context("Missing selected musical data")?,
                );
            }
            let index: super::projection::DomainIndex = related
                .iter()
                .map(|(kind, rows)| {
                    (
                        kind.as_str(),
                        rows.iter().map(|r| (r.id.as_str(), r)).collect(),
                    )
                })
                .collect();
            let track = super::projection::to_portable_track(&index, &record)?;
            track.validate()?;
            let now = record.value["created_at_ms"].as_i64();
            tx.execute(
                INSERT_TRACKS_SQL,
                params![
                    track.id,
                    track.source_sha256,
                    track.title,
                    track.artist,
                    track.album,
                    track.genre,
                    format!(
                        "{} {} {} {}",
                        track.title, track.artist, track.album, track.genre
                    )
                    .to_lowercase(),
                    track.title.to_lowercase(),
                    track.artist.to_lowercase(),
                    record.value["state"].as_str(),
                    track.archived,
                    serde_json::to_string(&track.metadata)?,
                    track.key,
                    track.bpm,
                    track.energy,
                    track.duration,
                    now,
                    record.value["updated_at_ms"].as_i64(),
                    serde_json::to_string(&track.locator)?,
                    track.origin,
                    track.analysis_status.as_str(),
                    track.musical.as_ref().map(|m| &m.id)
                ],
            )?;
            if let Some(m) = track.musical {
                tx.execute(
                    "INSERT INTO accepted_musical_revisions VALUES(?1,?2,?3,?4)",
                    params![m.id, record.id, serde_json::to_string(&m)?, now],
                )?;
            }
            if let Some(id) = record.value["selected_analysis_revision_id"].as_str() {
                let analysis = record_from_connection(&source, "analysis_revisions", id)?
                    .context("Missing selected analysis")?;
                if analysis.value["schema"] == "cuerithm-file-analysis-v1" {
                    let result: camelot_core::FileAnalysis =
                        serde_json::from_value(analysis.value["result"].clone())?;
                    tx.execute(
                        "UPDATE tracks SET analysis_json=?1 WHERE id=?2",
                        params![serde_json::to_string(&result)?, record.id],
                    )?;
                }
            }
            restore_curation(&tx, &record)?;
            Ok(())
        })?;
        for (kind, _) in KINDS {
            a.for_each(kind,&mut|r|{
control("restore_domains",done)?;
done+=1;
if done.is_multiple_of(1024){
ensure!(fs2::available_space(self.data_dir())?>=256<<20,"Insufficient restore disk reserve");

}
tx.execute("INSERT INTO portable_domain_records VALUES(?1,?2,?3)",params![kind,r.id,serde_json::to_string(&r)?])?;
let v=&r.value;
match *kind{
"tag_categories"=>{
tx.execute("INSERT INTO custom_tag_categories(id,name) VALUES(?1,?2)",params![r.id,v["name"].as_str()])?;
tx.execute("INSERT OR REPLACE INTO curation_group_properties(category_id,position) VALUES(?1,?2)",params![r.id,v["position"].as_i64()])?;

}
,"tags"=>{
tx.execute("INSERT INTO custom_tags(id,category_id,name) VALUES(?1,?2,?3)",params![r.id,v["category_id"].as_str(),v["name"].as_str()])?;
tx.execute("INSERT OR REPLACE INTO curation_value_properties(tag_id,position) VALUES(?1,?2)",params![r.id,v["position"].as_i64()])?;

}
,"track_tags"=>{
tx.execute("INSERT INTO track_custom_tags VALUES(?1,?2)",params![v["track_id"].as_str(),v["tag_id"].as_str()])?;

}
,"playlists"=>{
let definition=if v["definition_status"]=="active"&&!v["definition"].is_null(){
Some(serde_json::to_string(&v["definition"])?)
}
else{
None
}
;
tx.execute(INSERT_PLAYLISTS_SQL,params![r.id,v["parent_id"].as_str(),v["kind"].as_str(),v["name"].as_str(),v["position"].as_i64(),v["favorite_position"].as_i64(),definition,v["deleted"].as_bool()])?;
if v["kind"]=="smartlist"{
tx.execute("INSERT INTO smart_playlist_status(playlist_id,state,error) VALUES(?1,?2,?3)",params![r.id,if definition.is_some(){
"stale"
}
else{
"error"
}
,if definition.is_none(){
Some("Unsupported imported definition")
}
else{
None
}
])?;

}
}
,"playlist_members"=>{
tx.execute("INSERT INTO playlist_members VALUES(?1,?2,?3)",params![v["playlist_id"].as_str(),v["track_id"].as_str(),v["position"].as_i64()])?;

}
,"preferences"=>{
tx.execute("UPDATE library_preferences SET json=?1 WHERE singleton=1",[serde_json::to_string(&json!({
"key_mixing_mode":v["key_mixing_mode"],"time_zone":v["time_zone"]
}
))?])?;

}
,_=>{
}
}
Ok(())
}
)?;
        }
        let mut history = source.prepare(SELECT_RECORDS_SQL_3)?;
        let mut rows = history.query([])?;
        while let Some(row) = rows.next()? {
            let text: String = row.get(0)?;
            let r: LogicalRecord = serde_json::from_str(&text)?;
            let mut mini = LogicalArchive::empty("UTC")?;
            if let Some(track) = record_from_connection(
                &source,
                "tracks",
                r.value["track_id"].as_str().context("History track")?,
            )? {
                if let Some(id) = track.value["selected_analysis_revision_id"].as_str() {
                    mini.push(
                        "analysis_revisions",
                        record_from_connection(&source, "analysis_revisions", id)?
                            .context("History analysis")?,
                    );
                }
                mini.push("tracks", track);
            }
            mini.push("musical_revisions", r);
            super::projection::restore_native_cue_history(&tx, &mini)?;
        }
        let selected_record = a
            .record("settings", &selected_settings_id(a)?)?
            .context("Selected settings")?;
        super::projection::restore_template_shelf(&tx, &selected_record)?;
        tx.execute(
            "INSERT OR REPLACE INTO portable_library_meta VALUES('library_id',?1)",
            [&a.manifest.library_id],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO portable_library_meta VALUES('archive_metadata',?1)",
            [json!({
            "manifest":a.manifest,"payloads":{
            }
            }
            )
            .to_string()],
        )?;
        tx.execute_batch(CREATE_PORTABLE_OPTIONAL_LINES_SQL)?;
        let mut optional =
            source.prepare("SELECT name,ordinal,line FROM optional_lines ORDER BY name,ordinal")?;
        let mut rows = optional.query([])?;
        while let Some(row) = rows.next()? {
            tx.execute(
                "INSERT INTO portable_optional_lines VALUES(?1,?2,?3)",
                params![
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, Vec<u8>>(2)?
                ],
            )?;
        }
        ensure!(
            tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, i64>(
                0
            ))? == 0,
            "Staged referential integrity failed"
        );
        tx.execute(
            "UPDATE meta SET value=value+1 WHERE key IN('revision','smart_input_revision')",
            [],
        )?;
        tx.commit()?;
        drop(conn);
        self.save_settings(super::projection::restored_settings_from_record(
            &a.record("settings", &selected_settings_id(a)?)?
                .context("Selected settings")?,
        )?)?;
        Ok(())
    }
}
fn selected_settings_id(a: &SpoolArchive) -> Result<String> {
    Ok(a.connection()?.query_row(
        "SELECT id FROM records WHERE kind='settings' AND json_extract(json,'$.value.selected')=1",
        [],
        |r| r.get(0),
    )?)
}
pub(crate) fn record_from_connection(
    conn: &Connection,
    kind: &str,
    id: &str,
) -> Result<Option<LogicalRecord>> {
    let text: Option<String> = conn
        .prepare_cached("SELECT json FROM records WHERE kind=?1 AND id=?2")?
        .query_row(params![kind, id], |r| r.get(0))
        .optional()?;
    text.map(|s| serde_json::from_str(&s).map_err(Into::into))
        .transpose()
}
fn restore_curation(conn: &Connection, r: &LogicalRecord) -> Result<()> {
    if let Some(value) = r.extensions.get("com.cuerithm.curation") {
        let values: crate::CurationValues = serde_json::from_value(value.clone())?;
        ensure!(
            values.user_rating.is_none_or(|rating| rating <= 5) && values.notes.len() <= 1 << 20,
            "Invalid imported curation values"
        );
        let json = serde_json::to_string(&values)?;
        let input=json!({
"path":null,"source_hash":r.value["content"]["sha256"],"path_revision":0,"metadata_revision":0,"current_job":null,"state":r.value["state"]
}
).to_string();
        conn.execute(
            INSERT_CURATION_TRACKS_SQL,
            params![
                r.id,
                json,
                input,
                r.value["updated_at_ms"].as_i64(),
                values.notes.to_lowercase()
            ],
        )?;
    }
    Ok(())
}

const PRAGMA_RECORDS_SQL: &str = "PRAGMA journal_mode=DELETE; PRAGMA synchronous=OFF; CREATE TABLE records(kind TEXT NOT NULL,id TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id));CREATE INDEX record_tracks ON records(kind,json_extract(json,'$.value.track_id'));CREATE TABLE optional_lines(name TEXT NOT NULL,ordinal INTEGER NOT NULL,line BLOB NOT NULL,PRIMARY KEY(name,ordinal));";
const SELECT_RECORDS_SQL: &str = "SELECT EXISTS(SELECT 1 FROM records WHERE kind=?1 AND json_extract(json,'$.value.selected')=1 GROUP BY json_extract(json,'$.value.track_id') HAVING COUNT(*)>1)";
const SELECT_RECORDS_SQL_2: &str = "SELECT EXISTS(SELECT 1 FROM records WHERE kind=?1 AND json_extract(json,'$.value.track_id')=?2 AND json_extract(json,'$.value.selected')=1)";
const DELETE_RECORDS_SQL: &str = "DELETE FROM records WHERE kind IN('musical_revisions','analysis_revisions') AND json_extract(json,'$.value.selected')!=1";
const CREATE_PORTABLE_PAGE_IDS_SQL: &str = "CREATE INDEX IF NOT EXISTS portable_domain_track_lookup ON portable_domain_records(kind,json_extract(json,'$.value.track_id'));CREATE TEMP TABLE portable_page_ids(id TEXT PRIMARY KEY); CREATE TEMP VIEW tracks AS SELECT t.* FROM portable_page_ids p CROSS JOIN main.tracks t ON t.id=p.id;";
const SELECT_ACCEPTED_MUSICAL_REVISIONS_SQL: &str = "SELECT id,LENGTH(metadata_json)+COALESCE(LENGTH(analysis_json),0)+COALESCE((SELECT SUM(LENGTH(json)) FROM accepted_musical_revisions m WHERE m.track_id=t.id),0)+COALESCE((SELECT SUM(LENGTH(json)) FROM cue_revisions c WHERE c.track_id=t.id),0)+COALESCE((SELECT SUM(LENGTH(json)) FROM portable_domain_records d WHERE d.kind='tracks' AND d.id=t.id),0)+COALESCE((SELECT SUM(LENGTH(json)) FROM portable_domain_records d WHERE d.kind IN('musical_revisions','analysis_revisions') AND json_extract(d.json,'$.value.track_id')=t.id),0)+COALESCE((SELECT LENGTH(json) FROM portable_domain_records d WHERE d.kind='locators' AND d.id=json_extract(t.source_locator_json,'$.id')),0) FROM main.tracks t WHERE id>?1 ORDER BY id LIMIT 128";
const CREATE_SCHEMA_SQL: &str = "CREATE INDEX IF NOT EXISTS portable_member_lookup ON portable_domain_records(kind,json_extract(json,'$.value.playlist_id'),json_extract(json,'$.value.track_id')); CREATE INDEX IF NOT EXISTS portable_tag_lookup ON portable_domain_records(kind,json_extract(json,'$.value.track_id'),json_extract(json,'$.value.tag_id'));";
const SELECT_PORTABLE_DOMAIN_RECORDS_SQL: &str = "SELECT kind,id,json FROM portable_domain_records WHERE kind IN('roots','locators','import_sources','external_links') ORDER BY kind,id";
const CREATE_PORTABLE_SHARED_PAGE_SQL: &str = "CREATE TEMP TABLE portable_shared_page(rowid_value INTEGER PRIMARY KEY); CREATE TEMP TABLE portable_shared_kind(name TEXT);INSERT INTO portable_shared_kind VALUES('');";
const INSERT_TRACKS_SQL: &str = "INSERT INTO tracks(id,path,source_hash,title,artist,album,genre,search_text,sort_title,sort_artist,state,archived,metadata_json,musical_key,bpm,energy,duration,created,updated,source_locator_json,origin,analysis_status,selected_musical_revision) VALUES(?1,NULL,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)";
const INSERT_PLAYLISTS_SQL: &str = "INSERT INTO playlists(id,parent_id,kind,name,position,favorite_position,definition_json,deleted,deleted_by,last_edit_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,NULL,'imported-provenance')";
const SELECT_RECORDS_SQL_3: &str = "SELECT json FROM records WHERE kind='musical_revisions' AND json_extract(json,'$.extensions.\"com.cuerithm.cue_provenance\"') IS NOT NULL ORDER BY json_extract(json,'$.value.track_id'),json_extract(json,'$.value.created_at_ms'),json_extract(json,'$.extensions.\"com.cuerithm.source_order\"'),json_extract(json,'$.value.selected'),id";
const CREATE_PORTABLE_OPTIONAL_LINES_SQL: &str = "CREATE TABLE IF NOT EXISTS portable_optional_lines(name TEXT NOT NULL,ordinal INTEGER NOT NULL,line BLOB NOT NULL,PRIMARY KEY(name,ordinal));";
const INSERT_CURATION_TRACKS_SQL: &str = "INSERT INTO curation_tracks(track_id,revision,acknowledged_revision,desired_json,committed_json,state,input_json,updated,notes_search) VALUES(?1,1,1,?2,?2,'saved',?3,?4,?5)";

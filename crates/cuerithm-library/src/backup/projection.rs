use super::*;
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;
pub(crate) fn key_value(original: Option<&str>) -> Value {
    match original {
        None => Value::Null,
        Some(s) => {
            let key = cuerithm_smartlists::key_to_camelot(s)
                .and_then(|value| camelot_core::MusicalKey::from_camelot(&value).ok());
            json!({"original":s,"pitch_class":key.map(|k|k.tonic()),"mode":key.map(|k|k.mode())})
        }
    }
}
pub(crate) fn track_value(
    locator_id: &str,
    origin: &str,
    source_id: Option<&str>,
    source_record_id: Option<&str>,
    now: i64,
) -> Value {
    let mut metadata = serde_json::Map::new();
    for k in "title artist album album_artist genre comment grouping label remixer mix producer composer lyricist".split_whitespace(){metadata.insert(k.into(),json!([]));}
    json!({"locator_id":locator_id,"origin":origin,"source_id":source_id,"source_record_id":source_record_id,"state":"ready","archived":false,"archived_at_ms":null,"created_at_ms":now,"updated_at_ms":now,"content":{"sha256":null,"size_bytes":null,"observed_mtime_ns":null},"metadata":metadata,"attributes":{"rating":null,"release_year":null,"release_date":null,"track_number":null,"disc_number":null,"color":null,"play_count":null,"last_played_at_ms":null,"extra1":null,"extra2":null},"media":{"codec":null,"sample_rate":null,"channels":null,"bits_per_sample":null,"bitrate_kbps":null,"tags":{},"properties":{}},"analysis_status":if origin=="lexicon"{"imported"}else{"not_analyzed"},"selected_musical_revision_id":null,"selected_analysis_revision_id":null,"historical_preparation":{"settings_revision":null,"tagging_summary":null,"cleanup_summary":null}})
}
#[derive(Debug)]
pub(crate) struct SnapshotPageLimit;
impl std::fmt::Display for SnapshotPageLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Snapshot page memory budget exceeded")
    }
}
impl std::error::Error for SnapshotPageLimit {}
fn resident_json(v: &Value) -> usize {
    use std::mem::size_of;
    match v {
        Value::String(s) => size_of::<Value>() + s.capacity(),
        Value::Array(a) => {
            size_of::<Value>()
                + a.capacity() * size_of::<Value>()
                + a.iter().map(resident_json).sum::<usize>()
        }
        Value::Object(m) => {
            size_of::<Value>()
                + m.iter()
                    .map(|(k, v)| 128 + k.capacity() + resident_json(v))
                    .sum::<usize>()
        }
        _ => size_of::<Value>(),
    }
}
fn stored(conn: &Connection, a: &mut LogicalArchive, scoped: bool, shared: bool) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='portable_domain_records')",
        [],
        |r| r.get(0),
    )?;
    if exists {
        let scoped_sql = "SELECT kind,json FROM portable_domain_records WHERE kind IN('settings','preferences') UNION ALL SELECT kind,json FROM portable_domain_records WHERE kind='tracks' AND id IN(SELECT id FROM tracks) UNION ALL SELECT kind,json FROM portable_domain_records WHERE kind IN('musical_revisions','analysis_revisions') AND json_extract(json,'$.value.track_id') IN(SELECT id FROM tracks) UNION ALL SELECT kind,json FROM portable_domain_records WHERE kind='locators' AND id IN(SELECT json_extract(source_locator_json,'$.id') FROM tracks) UNION ALL SELECT kind,json FROM portable_domain_records WHERE ?1 AND kind NOT IN('settings','preferences','tracks','musical_revisions','analysis_revisions','locators')";
        let mut stmt=conn.prepare(if scoped{scoped_sql}else{"SELECT kind,json FROM portable_domain_records WHERE (?1 OR kind IN('tracks','musical_revisions','analysis_revisions','locators')) ORDER BY kind,id"})?;
        let mut resident = 0usize;
        for row in stmt.query_map([shared], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (kind, json) = row?;
            let parsed = super::parse_strict_json(json.as_bytes(), 32)?;
            resident = resident
                .checked_add(resident_json(&parsed))
                .context("Snapshot memory accounting overflow")?;
            if resident > 48 << 20 {
                return Err(SnapshotPageLimit.into());
            }
            a.push(&kind, serde_json::from_value(parsed)?);
        }
    }
    Ok(())
}
impl crate::Library {
    pub fn portable_snapshot(&self) -> Result<LogicalArchive> {
        self.portable_snapshot_with_control(&|_, _| Ok(()))
    }
    pub fn portable_snapshot_with_control(
        &self,
        control: &dyn Fn(&str, u64) -> Result<()>,
    ) -> Result<LogicalArchive> {
        let tmp = tempfile::tempdir_in(self.data_dir())?;
        let path = tmp.path().join("snapshot.sqlite");
        let mut dest = Connection::open(&path)?;
        {
            let conn = self.conn()?;
            crate::library_backup::quiet_database(&conn)?;
            conn.execute(
                "INSERT OR IGNORE INTO portable_library_meta(key,value) VALUES('library_id',?1)",
                [Uuid::new_v4().to_string()],
            )?;
            let backup = rusqlite::backup::Backup::new(&conn, &mut dest)?;
            loop {
                control("snapshot", 0)?;
                match backup.step(128)? {
                    rusqlite::backup::StepResult::Done => break,
                    rusqlite::backup::StepResult::More => {}
                    rusqlite::backup::StepResult::Busy | rusqlite::backup::StepResult::Locked => {
                        anyhow::bail!("Library snapshot could not acquire database pages")
                    }
                    _ => anyhow::bail!("Unknown SQLite snapshot state"),
                }
            }
        }
        ensure_import_records_supported(&dest)?;
        snapshot_connection(&dest, control, false, true, true)
    }
    pub fn restore_logical_archive(&self, a: &LogicalArchive) -> Result<()> {
        validate_logical_archive(a, &ArchiveLimits::default())?;
        ensure!(
            self.conn()?
                .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, i64>(0))?
                == 0,
            "Restore requires an empty staging generation"
        );
        let index: DomainIndex = a
            .records
            .iter()
            .map(|(kind, rows)| {
                (
                    kind.as_str(),
                    rows.iter().map(|r| (r.id.as_str(), r)).collect(),
                )
            })
            .collect();
        let tracks = a.records["tracks"]
            .iter()
            .map(|r| to_portable_track(&index, r))
            .collect::<Result<Vec<_>>>()?;
        self.import_portable_tracks(&tracks)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS portable_domain_records(kind TEXT NOT NULL,id TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id)); CREATE TABLE IF NOT EXISTS portable_library_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);")?;
        tx.execute(
            "INSERT OR REPLACE INTO portable_library_meta VALUES('library_id',?1)",
            [&a.manifest.library_id],
        )?;
        let opaque = json!({"manifest":a.manifest,"payloads":a.optional_payloads});
        tx.execute(
            "INSERT OR REPLACE INTO portable_library_meta VALUES('archive_metadata',?1)",
            [serde_json::to_string(&opaque)?],
        )?;
        for (kind, rows) in &a.records {
            for r in rows {
                tx.execute(
                    "INSERT OR REPLACE INTO portable_domain_records VALUES(?1,?2,?3)",
                    params![kind, r.id, serde_json::to_string(r)?],
                )?;
            }
        }
        for r in &a.records["tracks"] {
            tx.execute(
                "UPDATE tracks SET state=?1,created=?2,updated=?3 WHERE id=?4",
                params![
                    r.value["state"].as_str(),
                    r.value["created_at_ms"].as_i64(),
                    r.value["updated_at_ms"].as_i64(),
                    r.id
                ],
            )?;
            if let Some(id) = r.value["selected_analysis_revision_id"].as_str() {
                let analysis = a.records["analysis_revisions"]
                    .iter()
                    .find(|r| r.id == id)
                    .context("Analysis reference")?;
                if analysis.value["schema"] == "cuerithm-file-analysis-v1" {
                    let parsed: camelot_core::FileAnalysis =
                        serde_json::from_value(analysis.value["result"].clone())?;
                    tx.execute(
                        "UPDATE tracks SET analysis_json=?1 WHERE id=?2",
                        params![serde_json::to_string(&parsed)?, r.id],
                    )?;
                }
            }
        }
        restore_native_cue_history(&tx, a)?;
        if let Some(settings) = a.records["settings"]
            .iter()
            .find(|r| r.value["selected"] == true)
        {
            restore_template_shelf(&tx, settings)?;
        }
        for r in &a.records["tag_categories"] {
            tx.execute(
                "INSERT INTO custom_tag_categories(id,name) VALUES(?1,?2)",
                params![r.id, r.value["name"].as_str()],
            )?;
            tx.execute("INSERT OR REPLACE INTO curation_group_properties(category_id,position) VALUES(?1,?2)",params![r.id,r.value["position"].as_i64()])?;
        }
        for r in &a.records["tags"] {
            tx.execute(
                "INSERT INTO custom_tags(id,category_id,name) VALUES(?1,?2,?3)",
                params![
                    r.id,
                    r.value["category_id"].as_str(),
                    r.value["name"].as_str()
                ],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO curation_value_properties(tag_id,position) VALUES(?1,?2)",
                params![r.id, r.value["position"].as_i64()],
            )?;
        }
        for r in &a.records["track_tags"] {
            tx.execute(
                "INSERT INTO track_custom_tags(track_id,tag_id) VALUES(?1,?2)",
                params![r.value["track_id"].as_str(), r.value["tag_id"].as_str()],
            )?;
        }
        for r in &a.records["playlists"] {
            let v = &r.value;
            let definition = if v["definition_status"] == "active" && !v["definition"].is_null() {
                Some(serde_json::to_string(&v["definition"])?)
            } else {
                None
            };
            tx.execute("INSERT INTO playlists(id,parent_id,kind,name,position,favorite_position,definition_json,deleted,deleted_by,last_edit_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,NULL,'imported-provenance')",params![r.id,v["parent_id"].as_str(),v["kind"].as_str(),v["name"].as_str(),v["position"].as_i64(),v["favorite_position"].as_i64(),definition,v["deleted"].as_bool()])?;
            if v["kind"] == "smartlist" {
                tx.execute(
                    "INSERT INTO smart_playlist_status(playlist_id,state,error) VALUES(?1,?2,?3)",
                    params![
                        r.id,
                        if definition.is_some() {
                            "stale"
                        } else {
                            "error"
                        },
                        if definition.is_none() {
                            Some("Unsupported imported definition")
                        } else {
                            None
                        }
                    ],
                )?;
            }
        }
        for r in &a.records["playlist_members"] {
            tx.execute(
                "INSERT INTO playlist_members(playlist_id,track_id,position) VALUES(?1,?2,?3)",
                params![
                    r.value["playlist_id"].as_str(),
                    r.value["track_id"].as_str(),
                    r.value["position"].as_i64()
                ],
            )?;
        }
        let p = &a.records["preferences"][0].value;
        tx.execute(
            "UPDATE library_preferences SET json=?1 WHERE singleton=1",
            [serde_json::to_string(
                &json!({"key_mixing_mode":p["key_mixing_mode"],"time_zone":p["time_zone"]}),
            )?],
        )?;
        ensure!(
            tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, i64>(
                0
            ))? == 0,
            "Staged foreign key check failed"
        );
        tx.commit()?;
        drop(conn);
        let settings = a.records["settings"]
            .iter()
            .find(|r| r.value["selected"] == true)
            .context("Selected settings")?;
        self.save_settings(restored_settings_from_record(settings)?)?;
        Ok(())
    }
}
pub(super) fn ensure_import_records_supported(conn: &Connection) -> Result<()> {
    for table in [
        "import_sources",
        "import_snapshots",
        "import_items",
        "import_collections",
    ] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |r| r.get(0),
        )?;
        if exists {
            let has_records: bool =
                conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table})"), [], |r| {
                    r.get(0)
                })?;
            ensure!(
                !has_records,
                "Portable .cuerithm export cannot preserve DJ-library import records yet, including inspected sources, imported tracks and playlists. Use Library backup > Same-installation recovery snapshots to keep these records."
            );
        }
    }
    Ok(())
}
pub(crate) type DomainIndex<'a> = BTreeMap<&'a str, BTreeMap<&'a str, &'a LogicalRecord>>;
pub(crate) fn to_portable_track(
    index: &DomainIndex<'_>,
    r: &LogicalRecord,
) -> Result<crate::PortableTrack> {
    let v = &r.value;
    let lr = index["locators"]
        .get(v["locator_id"].as_str().unwrap_or(""))
        .context("Missing locator")?;
    let l = &lr.value;
    let locator = if l["kind"] == "local" {
        crate::PortableLocator {
            id: lr.id.clone(),
            flavor: serde_json::from_value(l["source_flavor"].clone())?,
            original: serde_json::from_value(l["original"].clone())?,
            source_root: if let Some(root) = l["root_id"].as_str() {
                Some(serde_json::from_value(
                    index["roots"].get(root).context("Missing root")?.value["source_root"].clone(),
                )?)
            } else {
                None
            },
            components: l["components"]
                .as_array()
                .map(|cs| {
                    cs.iter()
                        .map(|c| {
                            if let Some(s) = c.as_str() {
                                Ok(crate::EncodedPath::utf8(s))
                            } else {
                                Ok(serde_json::from_value(c.clone())?)
                            }
                        })
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?,
        }
    } else {
        crate::PortableLocator {
            id: lr.id.clone(),
            flavor: crate::PathFlavor::Posix,
            original: crate::EncodedPath::utf8(""),
            source_root: None,
            components: None,
        }
    };
    let musical = v["selected_musical_revision_id"]
        .as_str()
        .and_then(|id| index["musical_revisions"].get(id).copied());
    let m = musical.map(|r| &r.value);
    let text = |key: &str| {
        v["metadata"][key]
            .as_array()
            .map(|v| {
                v.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default()
    };
    let mut metadata: BTreeMap<String, Value> = v["metadata"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    metadata.insert("cuerithm.portable_content".into(), v["content"].clone());
    metadata.insert("cuerithm.portable_media".into(), v["media"].clone());
    metadata.insert(
        "cuerithm.portable_attributes".into(),
        v["attributes"].clone(),
    );
    Ok(crate::PortableTrack {
        id: r.id.clone(),
        locator,
        source_sha256: v["content"]["sha256"].as_str().map(str::to_string),
        title: text("title"),
        artist: text("artist"),
        album: text("album"),
        genre: text("genre"),
        metadata,
        key: m.and_then(|v| v["key"]["original"].as_str().map(str::to_string)),
        bpm: m.and_then(|v| v["bpm"].as_f64()),
        energy: m.and_then(|v| v["energy"].as_u64().map(|n| n as u8)),
        duration: m.and_then(|v| v["duration_seconds"].as_f64()),
        archived: v["archived"].as_bool().unwrap_or(false),
        origin: v["origin"].as_str().unwrap().into(),
        analysis_status: serde_json::from_value(v["analysis_status"].clone())?,
        musical: musical
            .map(|r| {
                Ok::<_, anyhow::Error>(crate::AcceptedMusicalData {
                    id: r.id.clone(),
                    source: r.value["origin"].as_str().unwrap_or("manual").into(),
                    cues: serde_json::from_value(r.value["cues"].clone())?,
                    tempo_markers: serde_json::from_value(r.value["tempo_markers"].clone())?,
                })
            })
            .transpose()?,
    })
}
pub(crate) fn snapshot_connection(
    conn: &Connection,
    control: &dyn Fn(&str, u64) -> Result<()>,
    scoped: bool,
    shared: bool,
    validate: bool,
) -> Result<LogicalArchive> {
    prepare_native_root_projection(conn, control)?;
    let mut a = LogicalArchive::empty("UTC")?;
    stored(conn, &mut a, scoped, shared)?;
    let metadata:Option<String>=conn.query_row(if scoped{"SELECT json_object('manifest',json_extract(value,'$.manifest'),'payloads',json('{}')) FROM portable_library_meta WHERE key='archive_metadata'"}else{"SELECT value FROM portable_library_meta WHERE key='archive_metadata'"},[],|r|r.get(0)).optional()?;
    if let Some(text) = metadata {
        let value: Value = serde_json::from_str(&text)?;
        let old: Manifest = serde_json::from_value(value["manifest"].clone())?;
        a.manifest.entries = old.entries;
        a.manifest.excluded = old.excluded;
        a.manifest.extensions = old.extensions;
        a.optional_payloads = serde_json::from_value(value["payloads"].clone())?;
    }

    let library_id: Option<String> = conn
        .query_row(
            "SELECT value FROM portable_library_meta WHERE key='library_id'",
            [],
            |r| r.get(0),
        )
        .optional()
        .unwrap_or(None);
    if let Some(id) = library_id {
        a.manifest.library_id = id;
    }
    let tracks = crate::portable::portable_tracks_from_connection(conn)?;
    let resident = tracks
        .iter()
        .map(|t| {
            t.metadata
                .iter()
                .map(|(key, value)| 128 + key.capacity() + resident_json(value))
                .sum::<usize>()
        })
        .sum::<usize>();
    if resident > 48 << 20 {
        return Err(SnapshotPageLimit.into());
    }
    let saved = a.records.clone();
    for kind in [
        "roots",
        "locators",
        "tracks",
        "musical_revisions",
        "analysis_revisions",
        "tag_categories",
        "tags",
        "track_tags",
        "playlists",
        "playlist_members",
    ] {
        a.records.get_mut(kind).unwrap().clear();
    }
    for t in tracks {
        control("project", a.records["tracks"].len() as u64)?;
        let old = saved["tracks"].iter().find(|r| r.id == t.id);
        let now = crate::model::now();
        let mut v = old
            .map(|r| r.value.clone())
            .unwrap_or_else(|| track_value(&t.locator.id, &t.origin, None, None, now));
        let existing_locator = saved["locators"].iter().find(|r| r.id == t.locator.id);
        v["locator_id"] = json!(t.locator.id);
        let locator = if let Some(r) = existing_locator.filter(|r| {
            if r.value["kind"] == "local" {
                r.value["original"]
                    == serde_json::to_value(&t.locator.original).unwrap_or(Value::Null)
                    && r.value["source_flavor"]
                        == serde_json::to_value(&t.locator.flavor).unwrap_or(Value::Null)
            } else {
                t.locator.original.data.is_empty()
            }
        }) {
            r.clone()
        } else {
            native_locator(&mut a, &aggregated_native_locator(conn, &t.id, &t.locator)?)?
        };
        a.push("locators", locator);
        v["content"]["sha256"] = json!(t.source_sha256);
        v["archived"] = json!(t.archived);
        v["analysis_status"] = json!(t.analysis_status);
        for (key, text) in [
            ("title", t.title),
            ("artist", t.artist),
            ("album", t.album),
            ("genre", t.genre),
        ] {
            let prior = &v["metadata"][key];
            let joined = prior
                .as_array()
                .map(|xs| {
                    xs.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            if joined != text {
                v["metadata"][key] = if text.is_empty() {
                    json!([])
                } else {
                    json!([text])
                };
            }
        }
        let curation = project_live_metadata(conn, &t.id, &t.metadata, &mut v)?;
        v["content"]["sha256"] = json!(t.source_sha256);
        let (state, created, updated, analysis): (String, i64, i64, Option<String>) = conn
            .query_row(
                "SELECT state,created,updated,analysis_json FROM tracks WHERE id=?1",
                [&t.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
        v["state"] = json!(state);
        v["created_at_ms"] = json!(created);
        v["updated_at_ms"] = json!(updated);
        // Preserve every accepted revision, with explicit selection independent of row order.
        let mut ms=conn.prepare("SELECT id,json,created FROM accepted_musical_revisions WHERE track_id=?1 ORDER BY created,id")?;
        let rows = ms
            .query_map([&t.id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut selected = None;
        for (id, data, created) in rows {
            let data: crate::AcceptedMusicalData = serde_json::from_str(&data)?;
            let is_selected = t.musical.as_ref().is_some_and(|m| m.id == id);
            let mut r=saved["musical_revisions"].iter().find(|r|r.id==id).cloned().unwrap_or_else(||LogicalRecord::new(&id,json!({"track_id":t.id,"origin":data.source,"created_at_ms":created,"selected":is_selected,"key":key_value(t.key.as_deref()),"bpm":t.bpm,"energy":t.energy,"duration_seconds":t.duration,"cues":data.cues,"tempo_markers":data.tempo_markers,"source_revision":null})));
            r.value["selected"] = json!(is_selected);
            if is_selected {
                selected = Some(id);
            }
            a.push("musical_revisions", r);
        }
        let mut history = conn.prepare(
            "SELECT id,json,created,rowid FROM cue_revisions WHERE track_id=?1 ORDER BY created,rowid",
        )?;
        let revisions = history
            .query_map([&t.id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let latest = revisions.last().map(|r| r.0.clone());
        for (id, data, created, source_order) in revisions {
            if a.records["musical_revisions"].iter().any(|r| r.id == id) {
                continue;
            }
            let raw: Value = serde_json::from_str(&data)?;
            let (cues, markers) = adapt_publication(&raw, &id)?;
            let is_selected = selected.is_none() && latest.as_ref() == Some(&id);
            if is_selected {
                selected = Some(id.clone());
            }
            let mut record=saved["musical_revisions"].iter().find(|r|r.id==id).cloned().unwrap_or_else(||LogicalRecord::new(&id,json!({"track_id":t.id,"origin":"cuerithm","created_at_ms":created,"selected":is_selected,"key":key_value(t.key.as_deref()),"bpm":t.bpm,"energy":t.energy,"duration_seconds":t.duration,"cues":cues,"tempo_markers":markers,"source_revision":id})));
            record.value["selected"] = json!(is_selected);
            record.value["cues"] = json!(cues);
            record.value["tempo_markers"] = json!(markers);
            record
                .extensions
                .insert("com.cuerithm.source_order".into(), json!(source_order));
            record
                .extensions
                .insert("com.cuerithm.cue_provenance".into(), raw);
            a.push("musical_revisions", record);
        }
        if selected.is_none()
            && (t.key.is_some() || t.bpm.is_some() || t.energy.is_some() || t.duration.is_some())
        {
            let id = Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                format!("urn:cuerithm:musical:{}", t.id).as_bytes(),
            )
            .to_string();
            let timing = analysis
                .as_deref()
                .map(serde_json::from_str::<Value>)
                .transpose()?;
            let (cues, markers) = timing
                .as_ref()
                .and_then(|v| v.get("publication"))
                .filter(|v| !v.is_null())
                .map(|p| adapt_publication(p, &id))
                .transpose()?
                .unwrap_or_default();
            a.push("musical_revisions",LogicalRecord::new(&id,json!({"track_id":t.id,"origin":"cuerithm","created_at_ms":updated,"selected":true,"key":key_value(t.key.as_deref()),"bpm":t.bpm,"energy":t.energy,"duration_seconds":t.duration,"cues":cues,"tempo_markers":markers,"source_revision":null})));
            selected = Some(id);
        }
        for old in &saved["musical_revisions"] {
            if old.value["track_id"] == t.id
                && !a.records["musical_revisions"]
                    .iter()
                    .any(|r| r.id == old.id)
            {
                let mut record = old.clone();
                record.value["selected"] = json!(selected.as_ref() == Some(&record.id));
                a.push("musical_revisions", record);
            }
        }
        v["selected_musical_revision_id"] = json!(selected);
        let current_analysis = analysis
            .as_deref()
            .map(serde_json::from_str::<camelot_core::FileAnalysis>)
            .transpose()?
            .map(serde_json::to_value)
            .transpose()?;
        let mut chosen = None;
        for old in &saved["analysis_revisions"] {
            if old.value["track_id"] == t.id {
                let mut record = old.clone();
                let is_selected = current_analysis
                    .as_ref()
                    .is_some_and(|result| *result == record.value["result"]);
                record.value["selected"] = json!(is_selected);
                if is_selected {
                    chosen = Some(record.id.clone());
                }
                a.push("analysis_revisions", record);
            }
        }
        if let Some(result) = current_analysis
            && chosen.is_none()
        {
            use sha2::{Digest, Sha256};
            let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&result)?));
            let id = Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                format!("urn:cuerithm:analysis:{}:{hash}", t.id).as_bytes(),
            )
            .to_string();
            a.push("analysis_revisions",LogicalRecord::new(&id,json!({"track_id":t.id,"created_at_ms":updated,"schema":"cuerithm-file-analysis-v1","engine_identity":{"name":result["analysis"]["engine"],"version":env!("CARGO_PKG_VERSION"),"implementation_sha256":result["analysis"]["engine_source_sha256"]},"settings_revision":null,"model_identity":[],"result":result,"selected":true})));
            chosen = Some(id);
        }
        let mut cached=conn.prepare("SELECT source_hash,settings_revision,engine_identity,decoder,analysis_json FROM preparation_analysis WHERE source_hash=(SELECT source_hash FROM tracks WHERE id=?1) UNION SELECT p.source_hash,p.settings_revision,p.engine_identity,p.decoder,p.analysis_json FROM jobs j JOIN preparation_analysis p ON p.source_hash=j.source_hash AND p.settings_revision=j.settings_revision WHERE j.track_id=?1 ORDER BY source_hash,settings_revision,engine_identity,decoder")?;
        for row in cached.query_map([&t.id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })? {
            let (source_hash, settings, engine, decoder, text) = row?;
            let parsed: camelot_core::FileAnalysis = serde_json::from_str(&text)?;
            let id = Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                format!(
                    "analysis-history:{}:{source_hash}:{settings}:{engine}:{decoder}",
                    t.id
                )
                .as_bytes(),
            )
            .to_string();
            if !a.records["analysis_revisions"].iter().any(|r| r.id == id) {
                let settings_id = Uuid::new_v5(
                    &Uuid::NAMESPACE_URL,
                    format!("settings:{settings}").as_bytes(),
                )
                .to_string();
                let models = parsed
                    .analysis
                    .models
                    .iter()
                    .map(|(name, sha)| json!({"name":name,"sha256":sha}))
                    .collect::<Vec<_>>();
                a.push("analysis_revisions",LogicalRecord::new(id,json!({"track_id":t.id,"created_at_ms":updated,"schema":"cuerithm-file-analysis-v1","engine_identity":{"name":parsed.analysis.engine,"version":env!("CARGO_PKG_VERSION"),"implementation_sha256":parsed.analysis.engine_source_sha256},"settings_revision":settings_id,"model_identity":models,"result":serde_json::to_value(&parsed)?,"selected":false})));
            }
        }
        v["selected_analysis_revision_id"] = json!(chosen);
        let mut record = old
            .cloned()
            .unwrap_or_else(|| LogicalRecord::new(&t.id, json!({})));
        record.value = v;
        if let Some(curation) = curation {
            record
                .extensions
                .insert("com.cuerithm.curation".into(), curation);
        }
        a.push("tracks", record);
    }
    for locator in &saved["locators"] {
        if !a.records["locators"].iter().any(|r| r.id == locator.id) {
            a.push("locators", locator.clone());
        }
    }
    for root in &saved["roots"] {
        if !a.records["roots"].iter().any(|r| r.id == root.id) {
            a.push("roots", root.clone());
        }
    }
    if shared {
        project_tables(conn, &mut a, &saved)?;
        project_external(conn, &mut a)?;
    }
    project_settings(conn, &mut a)?;
    project_template_shelf(conn, &mut a)?;
    project_preferences(conn, &mut a)?;
    if a.records["settings"].len() > 1
        && a.records["settings"][0].value["historical_revision"].is_null()
    {
        a.records.get_mut("settings").unwrap().remove(0);
    }
    if a.records["preferences"].len() > 1 {
        a.records.get_mut("preferences").unwrap().remove(0);
    }
    if validate {
        validate_logical_archive(&a, &ArchiveLimits::default())?;
    }
    Ok(a)
}
pub(crate) fn project_tables(
    conn: &Connection,
    a: &mut LogicalArchive,
    saved: &BTreeMap<String, Vec<LogicalRecord>>,
) -> Result<()> {
    for (kind, sql, keys) in [
        (
            "tag_categories",
            "SELECT c.id,c.name,COALESCE(p.position,0),NULL FROM custom_tag_categories c LEFT JOIN curation_group_properties p ON p.category_id=c.id ORDER BY c.id",
            vec!["name", "position", "color"],
        ),
        (
            "tags",
            "SELECT c.id,c.category_id,c.name,COALESCE(p.position,0),NULL FROM custom_tags c LEFT JOIN curation_value_properties p ON p.tag_id=c.id ORDER BY c.id",
            vec!["category_id", "name", "position", "shortcut"],
        ),
        (
            "playlists",
            "SELECT id,parent_id,kind,name,position,favorite_position,deleted,NULL,definition_json FROM playlists ORDER BY id",
            vec![
                "parent_id",
                "kind",
                "name",
                "position",
                "favorite_position",
                "deleted",
                "deleted_by",
                "definition",
            ],
        ),
    ] {
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let mut value = saved[kind]
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.value.clone())
                .unwrap_or_else(|| json!({}));
            for (index, key) in keys.iter().enumerate() {
                let raw = row.get_ref(index + 1)?;
                let v = match raw {
                    rusqlite::types::ValueRef::Null => Value::Null,
                    rusqlite::types::ValueRef::Integer(n) => {
                        if *key == "deleted" {
                            json!(n != 0)
                        } else {
                            json!(n)
                        }
                    }
                    rusqlite::types::ValueRef::Real(n) => json!(n),
                    rusqlite::types::ValueRef::Text(s) => {
                        if *key == "definition" {
                            serde_json::from_slice(s)?
                        } else {
                            json!(std::str::from_utf8(s)?)
                        }
                    }
                    _ => anyhow::bail!("Unexpected domain storage"),
                };
                if (*key == "color" || *key == "shortcut")
                    && v.is_null()
                    && value.get(*key).is_some()
                {
                    continue;
                }
                value[*key] = v;
            }
            if kind == "playlists" {
                if value.get("source_definition").is_none() {
                    value["source_definition"] = Value::Null;
                }
                if value.get("definition_status").is_none() {
                    value["definition_status"] = json!("active");
                }
            }
            let mut record = saved[kind]
                .iter()
                .find(|r| r.id == id)
                .cloned()
                .unwrap_or_else(|| LogicalRecord::new(&id, json!({})));
            record.value = value;
            a.push(kind, record);
        }
    }
    for (kind, sql, left, right) in [
        (
            "track_tags",
            "SELECT track_id,tag_id FROM track_custom_tags ORDER BY track_id,tag_id",
            "track_id",
            "tag_id",
        ),
        (
            "playlist_members",
            "SELECT playlist_id,track_id,position FROM playlist_members ORDER BY playlist_id,position",
            "playlist_id",
            "track_id",
        ),
    ] {
        let mut stmt = conn.prepare(sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let l: String = row.get(0)?;
            let r: String = row.get(1)?;
            let mut v = json!({left:l,right:r});
            if kind == "playlist_members" {
                v["position"] = json!(row.get::<_, i64>(2)?);
            }
            a.push(
                kind,
                LogicalRecord::new(
                    Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("{kind}:{l}:{r}").as_bytes())
                        .to_string(),
                    v,
                ),
            );
        }
    }
    Ok(())
}

fn adapt_publication(raw: &Value, revision: &str) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut cues = vec![];
    if let Some(rows) = raw["cues"].as_array() {
        for (index, c) in rows.iter().enumerate() {
            let publication = c.get("startTime").is_some();
            let start = if publication {
                c["startTime"].as_f64()
            } else {
                c["time_seconds"].as_f64()
            }
            .context("Unknown historical cue time representation")?;
            let end = c.get("endTime").cloned().unwrap_or(Value::Null);
            let source = c
                .get("data")
                .filter(|v| v.is_object())
                .cloned()
                .unwrap_or_else(|| json!({}));
            cues.push(json!({"id":Uuid::new_v5(&Uuid::NAMESPACE_URL,format!("cue:{revision}:{index}").as_bytes()).to_string(),"kind":if c["type"]=="5"{"loop"}else{"cue"},"start_seconds":start,"end_seconds":end,"slot":c.get("position").cloned().unwrap_or_else(||json!(index)),"memory":source["rekordbox"]["originalNum"]==-1,"name":c.get("name").cloned().unwrap_or(Value::Null),"color":c.get("color").filter(|v|v.is_string()).cloned().unwrap_or(Value::Null),"active_loop":c["activeLoop"].as_bool().unwrap_or(false),"source_data":source}));
        }
    }
    let mut markers = vec![];
    if let (Some(start), Some(bpm)) = (
        raw["grid"]["startTime"].as_f64(),
        raw["grid"]["bpm"].as_f64(),
    ) {
        markers.push(json!({"id":Uuid::new_v5(&Uuid::NAMESPACE_URL,format!("grid:{revision}").as_bytes()).to_string(),"start_seconds":start,"bpm":bpm,"beat_number":null,"beats_per_bar":null,"source_data":{}}));
    }
    Ok((cues, markers))
}

fn native_locator(a: &mut LogicalArchive, l: &crate::PortableLocator) -> Result<LogicalRecord> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    l.validate()?;
    if let (Some(root), Some(components)) = (&l.source_root, &l.components) {
        let root = serde_json::to_value(root)?;
        let root_id = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("root:{}:{}", serde_json::to_string(&l.flavor)?, root).as_bytes(),
        )
        .to_string();
        if !a.records["roots"].iter().any(|r| r.id == root_id) {
            a.push("roots",LogicalRecord::new(&root_id,json!({"label":"Music root","source_flavor":l.flavor,"source_root":root,"case_rule":"unknown"})));
        }
        let components = components
            .iter()
            .map(|p| {
                if p.encoding == crate::PathEncoding::Utf8 {
                    json!(p.data)
                } else {
                    json!(p)
                }
            })
            .collect::<Vec<_>>();
        return Ok(LogicalRecord::new(
            &l.id,
            json!({"kind":"local","source_flavor":l.flavor,"original":l.original,"root_id":root_id,"components":components}),
        ));
    }
    let bytes = l.original.bytes()?;
    let wide = l.original.encoding == crate::PathEncoding::Base64Utf16le;
    let units: Vec<u16> = if wide {
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect()
    } else {
        bytes.iter().map(|b| *b as u16).collect()
    };
    let windows = !matches!(
        l.flavor,
        crate::PathFlavor::Posix | crate::PathFlavor::OpaquePosix
    );
    if windows
        && units.len() >= 3
        && matches!(units[0], 47 | 92)
        && matches!(units[1], 47 | 92)
        && matches!(units[2], 46 | 63)
    {
        return Ok(LogicalRecord::new(
            &l.id,
            json!({"kind":"local","source_flavor":l.flavor,"original":l.original,"root_id":null,"components":null}),
        ));
    }
    let separator = |u: &u16| *u == 47 || windows && *u == 92;
    let absolute = if windows {
        units.len() > 2 && units[1] == 58 && separator(&units[2])
            || units.len() > 1 && separator(&units[0]) && separator(&units[1])
    } else {
        units.first() == Some(&47)
    };
    if !absolute {
        return Ok(LogicalRecord::new(
            &l.id,
            json!({"kind":"local","source_flavor":l.flavor,"original":l.original,"root_id":null,"components":null}),
        ));
    }
    let parts = units
        .split(separator)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    let count = if windows {
        if units[0] == 47 || units[0] == 92 {
            3
        } else {
            2
        }
    } else if parts
        .first()
        .is_some_and(|p| *p == [86, 111, 108, 117, 109, 101, 115])
    {
        2
    } else {
        1
    };
    let count = count.min(parts.len().saturating_sub(1));
    let encode = |u: &[u16]| -> Value {
        let b: Vec<u8> = if wide {
            u.iter().flat_map(|n| n.to_le_bytes()).collect()
        } else {
            u.iter().map(|n| *n as u8).collect()
        };
        if !wide && let Ok(s) = std::str::from_utf8(&b) {
            return json!({"encoding":"utf8","data":s});
        }
        json!({"encoding":if wide{"base64-utf16le"}else{"base64-bytes"},"data":STANDARD.encode(b)})
    };
    let mut root = vec![];
    if !windows || units.first().is_some_and(separator) {
        root.push(47);
        if windows {
            root.push(47);
        }
    }
    for (i, p) in parts[..count].iter().enumerate() {
        if i > 0 {
            root.push(47);
        }
        root.extend_from_slice(p);
    }
    if windows && root.last() == Some(&58) {
        root.push(47);
    }
    let root = encode(&root);
    let root_id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("root:{}:{}", serde_json::to_string(&l.flavor)?, root).as_bytes(),
    )
    .to_string();
    if !a.records["roots"].iter().any(|r| r.id == root_id) {
        a.push("roots",LogicalRecord::new(&root_id,json!({"label":"Music root","source_flavor":l.flavor,"source_root":root,"case_rule":"unknown"})));
    }
    let components = parts[count..]
        .iter()
        .map(|p| {
            let v = encode(p);
            if v["encoding"] == "utf8" {
                v["data"].clone()
            } else {
                v
            }
        })
        .collect::<Vec<_>>();
    Ok(LogicalRecord::new(
        &l.id,
        json!({"kind":"local","source_flavor":l.flavor,"original":l.original,"root_id":root_id,"components":components}),
    ))
}
fn project_settings(conn: &Connection, a: &mut LogicalArchive) -> Result<()> {
    let selected: Option<String> = conn
        .query_row(
            "SELECT revision FROM selected_settings WHERE singleton=1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let inherited = a.records["settings"]
        .iter()
        .rev()
        .find(|r| r.value["selected"] == true)
        .map(|r| r.value["portable_options"].clone())
        .unwrap_or_else(|| json!({}));
    if selected.is_some() {
        for record in a.records.get_mut("settings").unwrap() {
            record.value["selected"] = json!(false);
        }
    }
    let mut statement =
        conn.prepare("SELECT revision,json,created FROM settings ORDER BY created,revision")?;
    let mut settings_resident = 0usize;
    for row in statement.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })? {
        let (revision, text, created) = row?;
        let source = super::parse_strict_json(text.as_bytes(), 32)?;
        settings_resident = settings_resident
            .checked_add(resident_json(&source))
            .context("Settings history memory accounting overflow")?;
        if settings_resident > 48 << 20 {
            return Err(SnapshotPageLimit.into());
        }
        let settings = &source["settings"];
        let engine = &settings["engine"];
        let id = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("settings:{revision}").as_bytes(),
        )
        .to_string();
        let old = a.records["settings"].iter().find(|r| r.id == id).cloned();
        let unmapped = old
            .as_ref()
            .map(|r| &r.value["portable_options"])
            .unwrap_or(&inherited);
        let analysis = if engine.is_object() {
            json!({"sample_rate":engine["sample_rate"],"config":engine["config"],"structure_features":engine["structure_features"].as_bool().unwrap_or(false),"source_profile_timing":engine["source_profile_timing"].as_bool().unwrap_or(false),"lexicon_timing":engine["lexicon_timing"].as_bool().unwrap_or(false),"lexicon_energy":engine["lexicon_energy"].as_bool().unwrap_or(false),"lexicon_tempo":engine.get("lexicon_tempo").cloned().unwrap_or_else(||unmapped["analysis"]["lexicon_tempo"].clone()),"lexicon_config":engine.get("lexicon_config").cloned().unwrap_or(Value::Null),"cue_template":engine.get("cue_template").cloned().unwrap_or_else(||unmapped["analysis"]["cue_template"].clone())})
        } else {
            Value::Null
        };
        let tagging = if settings["tagging"].is_null() {
            unmapped.get("tagging").cloned().unwrap_or(Value::Null)
        } else {
            portable_tagging(&settings["tagging"])?
        };
        if !settings["tagging"].is_null() {
            a.manifest.excluded.push(json!({"domain":"tagging_machine_options","reason":"only_closed_v1_preference_allowlist_exported","count":1}));
        }
        let models = source["models"]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter(|(name, _)| !name.contains(['/', '\\', ':']))
                    .map(|(name, sha)| json!({"name":name,"sha256":sha}))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let value = json!({"portable_options":{"mik_style_energy":settings.get("mik_style_energy").cloned().unwrap_or_else(||json!(unmapped["mik_style_energy"].as_bool().unwrap_or(false))),"stability_millis":settings["stability_millis"].as_u64().unwrap_or(2000),"analysis":analysis,"cleanup":settings.get("cleanup").cloned().unwrap_or(Value::Null),"tagging":tagging},"historical_revision":revision,"model_identity":models,"created_at_ms":created,"selected":selected.as_ref()==Some(&revision)});
        let mut record = old.unwrap_or_else(|| LogicalRecord::new(&id, json!({})));
        record.value = value;
        if let Some(existing) = a
            .records
            .get_mut("settings")
            .unwrap()
            .iter_mut()
            .find(|r| r.id == id)
        {
            *existing = record;
        } else {
            a.push("settings", record);
        }
    }
    Ok(())
}
fn portable_tagging(source: &Value) -> Result<Value> {
    let options = &source["options"];
    let providers = options["platforms"]
        .as_array()
        .context("Invalid saved tagging providers")?
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| ["beatport", "traxsource"].contains(id))
        .collect::<Vec<_>>();
    let mut result = json!({"providers":providers,"no_match":source.get("no_match").cloned().unwrap_or_else(||json!("keep_existing")),"analysis_tag_source":source.get("analysis_tag_source").cloned().unwrap_or_else(||json!("measured")),"write_energy":source["write_energy"].as_bool().unwrap_or(true),"strictness":options["strictness"].as_f64().unwrap_or(0.7),"match_duration":false,"max_duration_difference":options["maxDurationDifference"].as_u64().unwrap_or(30),"match_by_id":false,"short_title":false,"merge_genres":false,"album_tagging":false,"album_tagging_ratio":options["albumTaggingRatio"].as_f64().unwrap_or(0.5)});
    for (portable, runtime) in [
        ("match_duration", "matchDuration"),
        ("match_by_id", "matchById"),
        ("short_title", "shortTitle"),
        ("merge_genres", "mergeGenres"),
        ("album_tagging", "albumTagging"),
    ] {
        result[portable] = json!(options[runtime].as_bool().unwrap_or(false));
    }
    Ok(result)
}
fn project_template_shelf(conn: &Connection, a: &mut LogicalArchive) -> Result<()> {
    let mut statement =
        conn.prepare("SELECT id,name,json,updated FROM cue_templates ORDER BY id")?;
    let mut templates = vec![];
    let mut bytes = 0usize;
    for row in statement.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })? {
        let (id, name, text, updated) = row?;
        bytes += text.len();
        ensure!(bytes <= 8 << 20, "Cue template shelf exceeds record limit");
        let template: camelot_core::lexicon_publication::CueTemplate = serde_json::from_str(&text)?;
        template.validate()?;
        templates.push(json!({"id":id,"name":name,"template":template,"updated_at_ms":updated}));
    }
    let selected = a
        .records
        .get_mut("settings")
        .unwrap()
        .iter_mut()
        .find(|r| r.value["selected"] == true)
        .context("Missing selected settings")?;
    selected
        .extensions
        .insert("com.cuerithm.cue_templates".into(), json!(templates));
    Ok(())
}
pub(crate) fn restore_template_shelf(conn: &Connection, record: &LogicalRecord) -> Result<()> {
    if let Some(templates) = record.extensions.get("com.cuerithm.cue_templates") {
        for source in templates.as_array().context("Invalid cue template shelf")? {
            let template: camelot_core::lexicon_publication::CueTemplate =
                serde_json::from_value(source["template"].clone())?;
            template.validate()?;
            conn.execute(
                "INSERT OR REPLACE INTO cue_templates VALUES(?1,?2,?3,?4)",
                params![
                    source["id"].as_str().context("Template ID")?,
                    source["name"].as_str().context("Template name")?,
                    serde_json::to_string(&template)?,
                    source["updated_at_ms"].as_i64().context("Template date")?
                ],
            )?;
        }
    }
    Ok(())
}
fn historical_locator(a: &mut LogicalArchive, text: &str) -> String {
    let id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("observed-path:{text}").as_bytes(),
    )
    .to_string();
    if !a.records["locators"].iter().any(|r| r.id == id) {
        let flavor = if text.starts_with("\\\\") || text.starts_with("//") {
            "windows-unc"
        } else if text.as_bytes().get(1) == Some(&b':') {
            "windows-drive"
        } else {
            "posix"
        };
        a.push("locators",LogicalRecord::new(&id,json!({"kind":"local","source_flavor":flavor,"original":{"encoding":"utf8","data":text},"root_id":null,"components":null})));
    }
    id
}
fn observed_cues(value: &Value) -> Value {
    let Some(cues) = value.as_array() else {
        return Value::Null;
    };
    let mut result = vec![];
    for (index, cue) in cues.iter().enumerate() {
        let start = cue["start_seconds"]
            .as_f64()
            .or_else(|| cue["startTime"].as_f64())
            .or_else(|| cue["time_seconds"].as_f64());
        let Some(start) = start else {
            return Value::Null;
        };
        let end = cue
            .get("end_seconds")
            .or_else(|| cue.get("endTime"))
            .cloned()
            .unwrap_or(Value::Null);
        let loop_cue = !end.is_null() || cue["type"] == "5";
        let source = cue
            .get("data")
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}));
        let id = cue["id"].as_str().map(str::to_string).unwrap_or_else(|| {
            Uuid::new_v5(
                &Uuid::NAMESPACE_URL,
                format!("native-observed-cue:{index}:{cue}").as_bytes(),
            )
            .to_string()
        });
        let color = cue["color"]
            .as_str()
            .map(str::to_string)
            .or_else(|| cue["color"].as_i64().map(|n| n.to_string()));
        result.push(json!({"id":id,"kind":if loop_cue{"loop"}else{"cue"},"start_seconds":start,"end_seconds":end,"slot":cue.get("slot").or_else(||cue.get("position")).cloned().unwrap_or(Value::Null),"memory":cue["memory"].as_bool().unwrap_or(false)||source["rekordbox"]["originalNum"]==-1,"name":cue.get("comment").or_else(||cue.get("name")).cloned().unwrap_or(Value::Null),"color":color,"active_loop":loop_cue&&end.as_f64().is_some_and(|end|end>start)&&(cue["active_loop"].as_bool().unwrap_or(false)||cue["activeLoop"].as_bool().unwrap_or(false)),"source_data":source}));
    }
    json!(result)
}
fn canonical_baseline_value(a: &mut LogicalArchive, name: &str, value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    match name {
        "title" | "artist" | "album" | "album_artist" | "genre" | "comment" | "grouping"
        | "label" | "remixer" | "mix" | "producer" | "composer" | "lyricist" => {
            if value.is_array() {
                value.clone()
            } else if let Some(s) = value.as_str() {
                if s.is_empty() { json!([]) } else { json!([s]) }
            } else {
                Value::Null
            }
        }
        "source_path" => value
            .as_str()
            .map(|s| json!(historical_locator(a, s)))
            .unwrap_or(Value::Null),
        "key" => key_value(value.as_str()),
        "cues" => observed_cues(value),
        "rating" | "release_year" | "track_number" | "disc_number" => {
            let number = value
                .as_u64()
                .or_else(|| value.as_str()?.parse::<u64>().ok());
            json!(number.filter(|n| match name {
                "rating" => *n <= 5,
                "release_year" => *n > 0,
                _ => true,
            }))
        }
        _ => value.clone(),
    }
}
pub(crate) fn project_external(conn: &Connection, a: &mut LogicalArchive) -> Result<()> {
    let mut targets = conn.prepare(
        "SELECT id,database_id,schema_fingerprint,created FROM native_targets ORDER BY id",
    )?;
    for row in targets.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })? {
        let (id, database, fingerprint, created) = row?;
        if !a.records["import_sources"].iter().any(|r| r.id == id) {
            a.push("import_sources",LogicalRecord::new(&id,json!({"vendor":"rekordbox","source_library_id":database,"profile":"rekordbox-observation-v1","archive_sha256":null,"imported_at_ms":created,"source_schema":{"fingerprint":fingerprint}})));
        }
    }
    let mut links=conn.prepare("SELECT target_id,content_id,local_track_id,observation_json,baseline_json FROM native_track_links ORDER BY target_id,content_id")?;
    for row in links.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, String>(4)?,
        ))
    })? {
        let (source, record, track, observation, baseline) = row?;
        let raw: Value = serde_json::from_str(&baseline)?;
        let mut output = serde_json::Map::new();
        for (name, value) in raw.as_object().context("Invalid native baseline")? {
            let name = match name.as_str() {
                "mix_name" => "mix",
                "duration_seconds" => "duration",
                "tag_ids" => "tags",
                s => s,
            };
            if !"title artist album album_artist genre comment grouping label remixer mix producer composer lyricist key bpm energy duration rating release_year release_date track_number disc_number source_path cues tags".split_whitespace().any(|s|s==name){a.manifest.excluded.push(json!({"domain":"native_baseline_field","reason":"outside_allowlist","count":1}));continue}
            output.insert(name.into(),json!({"local":canonical_baseline_value(a,name,&value["local"]),"remote":canonical_baseline_value(a,name,&value["remote"]),"owner":value["owner"],"acknowledged":value["acknowledged"]}));
        }
        let mut locator_id = None;
        let observation=observation.map(|text|->Result<Value>{let observed:cuerithm_rekordbox::ImportedTrack=serde_json::from_str(&text)?;let raw=serde_json::to_value(&observed.metadata)?;let mut metadata=serde_json::Map::new();for k in "title artist album album_artist genre label comment remixer composer lyricist mix_name release_year release_date track_number disc_number rating".split_whitespace(){let name=if k=="mix_name"{"mix"}else{k};metadata.insert(name.into(),canonical_baseline_value(a,name,&raw[k]));}locator_id=observed.source.path.as_ref().map(|s|historical_locator(a,s));Ok(json!({"metadata":metadata,"musical":{"key":key_value(Some(&observed.metadata.key)),"bpm":observed.bpm,"duration":observed.duration_seconds,"cues":observed_cues(&serde_json::to_value(&observed.cues)?)},"source_locator_id":locator_id}))}).transpose()?;
        let id = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("observed:{source}:{record}").as_bytes(),
        )
        .to_string();
        a.push("external_links",LogicalRecord::new(id,json!({"track_id":track,"source_id":source,"record_kind":"track","record_id":record,"observation":observation,"baseline":output,"source_locator_id":locator_id,"binding_status":"inactive"})));
    }
    let raw: i64 = conn.query_row("SELECT COUNT(*) FROM native_records", [], |r| r.get(0))?;
    if raw > 0 {
        a.manifest.excluded.push(json!({"domain":"native_records","reason":"no_vetted_raw_record_allowlist","count":raw}));
    }
    let mut legacy = conn.prepare(
        "SELECT track_id,target,external_id,baseline_json FROM sync_links ORDER BY target,track_id",
    )?;
    for row in legacy.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        let (track, target, external, _baseline) = row?;
        let source = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            format!("legacy-sync:{target}").as_bytes(),
        )
        .to_string();
        if !a.records["import_sources"].iter().any(|r| r.id == source) {
            a.push("import_sources",LogicalRecord::new(&source,json!({"vendor":"legacy","source_library_id":source,"profile":"legacy-sync-link","archive_sha256":null,"imported_at_ms":0,"source_schema":{}})));
        }
        a.push("external_links",LogicalRecord::new(Uuid::new_v5(&Uuid::NAMESPACE_URL,format!("legacy-link:{source}:{track}").as_bytes()).to_string(),json!({"track_id":track,"source_id":source,"record_kind":"track","record_id":external,"observation":null,"baseline":{},"source_locator_id":null,"binding_status":"inactive"})));
        a.manifest.excluded.push(
            json!({"domain":"legacy_baseline","reason":"unversioned_legacy_payload","count":1}),
        );
    }
    Ok(())
}
fn project_live_metadata(
    conn: &Connection,
    id: &str,
    metadata: &BTreeMap<String, Value>,
    value: &mut Value,
) -> Result<Option<Value>> {
    if let Some(content) = metadata.get("cuerithm.portable_content") {
        value["content"] = content.clone();
    }
    if let Some(media) = metadata.get("cuerithm.portable_media") {
        value["media"] = media.clone();
    }
    if let Some(attributes) = metadata.get("cuerithm.portable_attributes") {
        value["attributes"] = attributes.clone();
    }
    let has_media = metadata.get("codec").is_some_and(Value::is_string);
    if has_media {
        for key in "album_artist comment grouping label remixer mix producer composer lyricist"
            .split_whitespace()
        {
            value["metadata"][key] = json!([]);
        }
        for key in "rating release_year release_date track_number disc_number color play_count last_played_at_ms extra1 extra2".split_whitespace(){value["attributes"][key]=Value::Null;}
    }
    let tags = metadata.get("tags").and_then(Value::as_object);
    let get_tag = |aliases: &[&str]| -> Option<&str> {
        tags.and_then(|tags| {
            aliases
                .iter()
                .find_map(|name| tags.get(*name).and_then(Value::as_str))
        })
    };
    for key in "title artist album album_artist genre comment grouping label remixer mix producer composer lyricist".split_whitespace(){if let Some(v)=metadata.get(key).filter(|v|v.as_array().is_some_and(|a|a.iter().all(Value::is_string))){value["metadata"][key]=v.clone();}let aliases=match key{"album_artist"=>vec!["album_artist","albumartist","album artist"],"label"=>vec!["label","publisher"],"mix"=>vec!["mix","subtitle","version"],key=>vec![key]};if let Some(text)=get_tag(&aliases){value["metadata"][key]=if text.is_empty(){json!([])}else{json!([text])};}}
    if metadata.get("codec").is_some_and(Value::is_string) {
        for key in [
            "codec",
            "sample_rate",
            "channels",
            "bits_per_sample",
            "tags",
            "properties",
        ] {
            if let Some(v) = metadata.get(key) {
                value["media"][key] = v.clone();
            }
        }
    }
    let properties = metadata.get("properties").and_then(Value::as_object);
    let prop_number = |key: &str| {
        properties
            .and_then(|m| m.get(key))
            .and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse().ok()))
    };
    if let Some(size) =
        prop_number("format.size").filter(|n| *n >= 0.0 && n.fract() == 0.0 && *n < u64::MAX as f64)
    {
        value["content"]["size_bytes"] = json!(size as u64);
    }
    if let Some(bitrate) = prop_number("audio.bit_rate").filter(|n| *n >= 0.0 && n.is_finite()) {
        value["media"]["bitrate_kbps"] = json!(bitrate / 1000.0);
    }
    for (key, aliases) in [
        ("rating", vec!["rating"]),
        ("release_year", vec!["year", "date"]),
        ("track_number", vec!["tracknumber", "track", "track_number"]),
        ("disc_number", vec!["discnumber", "disc", "disc_number"]),
        ("play_count", vec!["playcount", "play_count"]),
    ] {
        if let Some(text) = get_tag(&aliases) {
            let number = if matches!(key, "track_number" | "disc_number") {
                text.split('/').next().unwrap_or(text).parse::<u64>().ok()
            } else if key == "release_year" && text.len() >= 4 {
                text.get(..4).and_then(|s| s.parse::<u64>().ok())
            } else {
                text.parse::<u64>().ok()
            };
            value["attributes"][key] = json!(number.filter(|n| match key {
                "rating" => *n <= 5,
                "release_year" => *n > 0,
                _ => true,
            }));
        }
    }
    for (key, aliases) in [
        (
            "release_date",
            vec![
                "releasedate",
                "release_date",
                "release date",
                "tdrl",
                "date",
            ],
        ),
        ("color", vec!["color"]),
        ("extra1", vec!["extra1"]),
        ("extra2", vec!["extra2"]),
    ] {
        if let Some(text) = get_tag(&aliases) {
            value["attributes"][key] = json!((!text.is_empty()).then_some(text));
        }
    }
    let native: Option<String> = conn.query_row(
        "SELECT native_metadata_json FROM tracks WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    if let Some(text) = native {
        let native: cuerithm_cleanup::Metadata = serde_json::from_str(&text)?;
        native.validate()?;
        for key in "title artist album album_artist genre comment grouping label remixer mix producer composer lyricist".split_whitespace(){value["metadata"][key]=json!([]);}
        for (field, values) in native.fields {
            let name = serde_json::to_value(field)?
                .as_str()
                .context("Metadata field name")?
                .to_string();
            value["metadata"][&name] = json!(values);
        }
    }
    let curation: Option<String> = conn
        .query_row(
            "SELECT desired_json FROM curation_tracks WHERE track_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    curation
        .map(|text| {
            let values: crate::CurationValues = serde_json::from_str(&text)?;
            serde_json::to_value(values).map_err(Into::into)
        })
        .transpose()
}
fn project_preferences(conn: &Connection, a: &mut LogicalArchive) -> Result<()> {
    let text: String = conn.query_row(
        "SELECT json FROM library_preferences WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    let p: Value = serde_json::from_str(&text)?;
    let mut value = a.records["preferences"]
        .last()
        .map(|r| r.value.clone())
        .context("Missing preferences")?;
    value["key_mixing_mode"] = p["key_mixing_mode"].clone();
    value["time_zone"] = match p["time_zone"].as_str() {
        Some(zone) => json!(zone),
        None => json!(iana_time_zone::get_timezone().context(
            "Cannot detect source calendar timezone; choose one in library preferences"
        )?),
    };
    let mut record = a.records["preferences"]
        .last()
        .cloned()
        .context("Missing preferences")?;
    record.value = value;
    a.records.insert("preferences".into(), vec![record]);
    Ok(())
}

pub(crate) fn restore_native_cue_history(conn: &Connection, a: &LogicalArchive) -> Result<()> {
    let mut histories = a.records["musical_revisions"]
        .iter()
        .filter(|r| {
            r.value["origin"] == "cuerithm"
                && r.extensions.get("com.cuerithm.cue_provenance").is_some()
        })
        .collect::<Vec<_>>();
    histories.sort_by(|a, b| {
        a.value["created_at_ms"]
            .as_i64()
            .cmp(&b.value["created_at_ms"].as_i64())
            .then(
                a.extensions
                    .get("com.cuerithm.source_order")
                    .and_then(Value::as_i64)
                    .cmp(
                        &b.extensions
                            .get("com.cuerithm.source_order")
                            .and_then(Value::as_i64),
                    ),
            )
            .then(a.id.cmp(&b.id))
    });
    for r in histories {
        let raw = &r.extensions["com.cuerithm.cue_provenance"];
        conn.execute(
            "INSERT INTO cue_revisions(id,track_id,job_id,json,created) VALUES(?1,?2,NULL,?3,?4)",
            params![
                r.id,
                r.value["track_id"].as_str(),
                serde_json::to_string(raw)?,
                r.value["created_at_ms"].as_i64()
            ],
        )?;
        if r.value["selected"] == true && raw["schema"] == "lexicon-publication-v1" {
            let track = a.records["tracks"]
                .iter()
                .find(|t| t.id == r.value["track_id"].as_str().unwrap_or(""))
                .context("Native history track")?;
            let analysis = track.value["selected_analysis_revision_id"]
                .as_str()
                .and_then(|id| a.records["analysis_revisions"].iter().find(|r| r.id == id));
            if let Some(analysis) =
                analysis.filter(|r| r.value["schema"] == "cuerithm-file-analysis-v1")
            {
                let file: camelot_core::FileAnalysis =
                    serde_json::from_value(analysis.value["result"].clone())?;
                let publication: camelot_core::lexicon_publication::Publication =
                    serde_json::from_value(raw.clone())?;
                let (cues, markers) = adapt_publication(raw, &r.id)?;
                if file.complete
                    && file.publication.as_ref().is_some_and(|p| {
                        serde_json::to_value(p).ok() == serde_json::to_value(&publication).ok()
                    })
                    && r.value["cues"] == json!(cues)
                    && r.value["tempo_markers"] == json!(markers)
                {
                    conn.execute(
                        "UPDATE tracks SET selected_musical_revision=NULL WHERE id=?1",
                        [&track.id],
                    )?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn restored_settings_from_record(
    record: &LogicalRecord,
) -> Result<crate::PreparationSettings> {
    let options = &record.value["portable_options"];
    let mut settings = crate::PreparationSettings {
        stability_millis: options["stability_millis"]
            .as_u64()
            .context("Invalid stability")?,
        ..Default::default()
    };
    let analysis = &options["analysis"];
    if !analysis.is_null() {
        settings.engine.config = serde_json::from_value(analysis["config"].clone())?;
        if let Some(rate) = analysis["sample_rate"]
            .as_u64()
            .filter(|n| [11025, 22050, 44100].contains(n))
        {
            settings.engine.sample_rate = rate as u32;
        }
        settings.engine.structure_features =
            analysis["structure_features"].as_bool().unwrap_or(false);
        settings.engine.source_profile_timing =
            analysis["source_profile_timing"].as_bool().unwrap_or(false);
        settings.engine.lexicon_timing = analysis["lexicon_timing"].as_bool().unwrap_or(false);
        settings.engine.lexicon_energy = analysis["lexicon_energy"].as_bool().unwrap_or(false);
        if !analysis["lexicon_config"].is_null() {
            settings.engine.lexicon_config =
                Some(serde_json::from_value(analysis["lexicon_config"].clone())?);
        }
    }
    if !options["cleanup"].is_null() {
        settings.cleanup = Some(serde_json::from_value(options["cleanup"].clone())?);
    }
    #[cfg(feature = "autotag")]
    if !options["tagging"].is_null() {
        let portable = &options["tagging"];
        let mut config = cuerithm_autotag::AutotagConfig::default();
        config.options.platforms = serde_json::from_value(portable["providers"].clone())?;
        config.no_match = serde_json::from_value(portable["no_match"].clone())?;
        config.analysis_tag_source =
            serde_json::from_value(portable["analysis_tag_source"].clone())?;
        config.write_energy = portable["write_energy"].as_bool().unwrap_or(true);
        config.options.strictness = portable["strictness"].as_f64().unwrap_or(0.7);
        config.options.match_duration = portable["match_duration"].as_bool().unwrap_or(false);
        config.options.max_duration_difference =
            portable["max_duration_difference"].as_u64().unwrap_or(30);
        config.options.match_by_id = portable["match_by_id"].as_bool().unwrap_or(false);
        config.options.short_title = portable["short_title"].as_bool().unwrap_or(false);
        config.options.merge_genres = portable["merge_genres"].as_bool().unwrap_or(false);
        config.options.album_tagging = portable["album_tagging"].as_bool().unwrap_or(false);
        config.options.album_tagging_ratio =
            portable["album_tagging_ratio"].as_f64().unwrap_or(0.5) as f32;
        if !config.options.platforms.is_empty() {
            settings.tagging = Some(config);
        }
    }
    settings.validate()?;
    Ok(settings)
}

fn native_root_rows_sql(conn: &Connection) -> Result<String> {
    // Discover the schema-owned unique path index instead of depending on an
    // SQLite-generated autoindex ordinal. The range skips NULL bindings without
    // reading the potentially large track/analysis rows at all.
    let index:String=conn.query_row(r#"SELECT l.name FROM pragma_index_list('tracks','main') l WHERE l."unique"=1 AND l.partial=0 AND (SELECT COUNT(*) FROM pragma_index_info(l.name,'main'))=1 AND EXISTS(SELECT 1 FROM pragma_index_info(l.name,'main') c WHERE c.name='path') LIMIT 1"#,[],|r|r.get(0))?;
    let index = index.replace('"', "\"\"");
    // Return bound imported rows with a skip flag so progress can advance even
    // when every binding belongs to immutable imported provenance.
    Ok(format!(
        "SELECT id,path,source_locator_json,CASE WHEN origin='cuerithm' THEN EXISTS(SELECT 1 FROM main.portable_domain_records d WHERE d.kind='locators' AND d.id=json_extract(t.source_locator_json,'$.id')) ELSE 1 END FROM main.tracks t INDEXED BY \"{index}\" WHERE path>=x'' "
    ))
}

/// Build once on the scratch snapshot, before the spooled track-page view exists.
/// SQLite holds root memberships; Rust retains only one path/root at a time.
pub(crate) fn prepare_native_root_projection(
    conn: &Connection,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<()> {
    if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_temp_master WHERE name='portable_native_root_members')",
        [],
        |r| r.get::<_, bool>(0),
    )? {
        return Ok(());
    }
    conn.execute_batch("PRAGMA temp_store=FILE;CREATE TEMP TABLE portable_native_root_config(key TEXT PRIMARY KEY,json TEXT NOT NULL);CREATE TEMP TABLE portable_native_root_groups(key TEXT PRIMARY KEY,json TEXT NOT NULL);CREATE TEMP TABLE portable_native_root_members(track_id TEXT PRIMARY KEY,group_key TEXT NOT NULL);")?;
    let insert_config = |locator: crate::PortableLocator| -> Result<()> {
        if let Some(parts) = crate::portable::PortableSourceParts::from_directory_locator(&locator)?
        {
            conn.execute(
                "INSERT OR IGNORE INTO portable_native_root_config VALUES(?1,?2)",
                rusqlite::params![
                    parts.key(parts.parts.len())?,
                    serde_json::to_string(&parts)?
                ],
            )?;
        }
        Ok(())
    };
    let mut roots = conn.prepare("SELECT path FROM main.organization_roots ORDER BY id")?;
    let mut rows = roots.query([])?;
    while let Some(row) = rows.next()? {
        let path: Vec<u8> = row.get(0)?;
        insert_config(crate::PortableLocator::from_native_path_bytes(
            "configured-root",
            &path,
        )?)?;
    }
    drop(rows);
    drop(roots);
    let mut roots=conn.prepare("SELECT j.value FROM main.selected_settings x JOIN main.settings s ON s.revision=x.revision JOIN json_each(s.json,'$.settings.watch_folders') j WHERE j.type='text'")?;
    let mut rows = roots.query([])?;
    while let Some(row) = rows.next()? {
        let path: String = row.get(0)?;
        insert_config(crate::PortableLocator::from_native_path_bytes(
            "configured-root",
            &crate::path_bytes(std::path::Path::new(&path)),
        )?)?;
    }
    drop(rows);
    drop(roots);
    control("mapping_roots", 0)?;
    let sql = native_root_rows_sql(conn)?;
    let mut tracks = conn.prepare(&sql)?;
    let mut rows = tracks.query([])?;
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        if count.is_multiple_of(256) {
            control("mapping_roots", count)?;
        }
        count += 1;
        if row.get::<_, bool>(3)? {
            continue;
        }
        let id: String = row.get(0)?;
        let native: Vec<u8> = row.get(1)?;
        let saved: Option<String> = row.get(2)?;
        let locator = if let Some(json) = saved {
            serde_json::from_str(&json)?
        } else {
            crate::PortableLocator::from_native_path_bytes(format!("locator:{id}"), &native)?
        };
        let Some(parts) = crate::portable::PortableSourceParts::from_locator(&locator)? else {
            continue;
        };
        if parts.parts.len() <= parts.anchors {
            continue;
        }
        let parent_count = parts.parts.len() - 1;
        let mut configured = None;
        for n in (0..=parent_count).rev() {
            let key = parts.key(n)?;
            let root: Option<String> = conn
                .query_row(
                    "SELECT json FROM portable_native_root_config WHERE key=?1",
                    [&key],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(root) = root {
                configured = Some((format!("configured:{key}"), root));
                break;
            }
        }
        let (key, mut root, pinned) = if let Some((key, json)) = configured {
            (
                key,
                serde_json::from_str::<crate::portable::PortableSourceParts>(&json)?,
                true,
            )
        } else {
            let floor = (parts.anchors + 1).min(parent_count);
            let mut root = parts.clone();
            root.parts.truncate(parent_count);
            (format!("common:{}", parts.key(floor)?), root, false)
        };
        if !pinned {
            let previous: Option<String> = conn
                .query_row(
                    "SELECT json FROM portable_native_root_groups WHERE key=?1",
                    [&key],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(previous) = previous {
                let old: crate::portable::PortableSourceParts = serde_json::from_str(&previous)?;
                let common = root.common_prefix_len(&old);
                root.parts.truncate(common);
            }
        }
        conn.execute("INSERT INTO portable_native_root_groups VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET json=excluded.json",rusqlite::params![key,serde_json::to_string(&root)?])?;
        conn.execute(
            "INSERT INTO portable_native_root_members VALUES(?1,?2)",
            rusqlite::params![id, key],
        )?;
    }
    Ok(())
}
fn aggregated_native_locator(
    conn: &Connection,
    id: &str,
    locator: &crate::PortableLocator,
) -> Result<crate::PortableLocator> {
    let root:Option<String>=conn.query_row("SELECT g.json FROM portable_native_root_members m JOIN portable_native_root_groups g ON g.key=m.group_key WHERE m.track_id=?1",[id],|r|r.get(0)).optional()?;
    if let Some(root) = root {
        let root = serde_json::from_str(&root)?;
        if let Some(parts) = crate::portable::PortableSourceParts::from_locator(locator)? {
            return parts.rebase(&root, locator);
        }
    }
    Ok(locator.clone())
}

#[cfg(test)]
mod native_root_query_tests {
    use super::*;
    #[test]
    fn zero_binding_root_scan_uses_a_validated_path_range_index() {
        let directory = tempfile::tempdir().unwrap();
        let conn = Connection::open(directory.path().join("root-plan.sqlite")).unwrap();
        conn.execute_batch("CREATE TABLE tracks(id TEXT PRIMARY KEY,spare TEXT UNIQUE,path BLOB UNIQUE,origin TEXT,source_locator_json TEXT);CREATE TABLE portable_domain_records(kind TEXT,id TEXT,json TEXT,PRIMARY KEY(kind,id));CREATE TABLE organization_roots(id TEXT PRIMARY KEY,path BLOB);CREATE TABLE settings(revision TEXT PRIMARY KEY,json TEXT);CREATE TABLE selected_settings(revision TEXT);WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<100000) INSERT INTO tracks(id,origin,source_locator_json) SELECT printf('%08d',i),'cuerithm',printf('%0512d',i) FROM n;").unwrap();
        let sql = native_root_rows_sql(&conn).unwrap();
        let mut plan = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let details = plan
            .query_map([], |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            details
                .iter()
                .any(|d| d.contains("SEARCH t USING INDEX") && d.contains("(path>")),
            "{details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|d| d.contains("SCAN t") || d.contains("TEMP B-TREE")),
            "{details:?}"
        );
        drop(plan);
        assert!(!conn.prepare(&sql).unwrap().exists([]).unwrap());
        let phases = std::cell::RefCell::new(Vec::new());
        prepare_native_root_projection(&conn, &|phase, count| {
            phases.borrow_mut().push((phase.to_owned(), count));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            phases.borrow().first(),
            Some(&("mapping_roots".to_owned(), 0))
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM portable_native_root_members",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
            0
        );
        conn.execute_batch("DROP TABLE portable_native_root_members;DROP TABLE portable_native_root_groups;DROP TABLE portable_native_root_config;UPDATE tracks SET path=CAST('unix:/Music/'||id AS BLOB),source_locator_json=json_object('id','locator:'||id);INSERT INTO portable_domain_records SELECT 'locators','locator:'||id,'{}' FROM tracks;").unwrap();
        phases.borrow_mut().clear();
        prepare_native_root_projection(&conn, &|phase, count| {
            phases.borrow_mut().push((phase.to_owned(), count));
            Ok(())
        })
        .unwrap();
        assert!(
            phases
                .borrow()
                .last()
                .is_some_and(|(phase, count)| phase == "mapping_roots" && *count > 99000)
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM portable_native_root_members",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
            0
        );
    }
}

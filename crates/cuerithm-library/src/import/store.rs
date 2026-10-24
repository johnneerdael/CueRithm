use super::*;
use crate::{Library, model::now};
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

struct Spool {
    db: Connection,
    limits: ImportLimits,
    bytes: u64,
    sequence: i64,
}
fn record_key(record: &ObservationRecord) -> (&'static str, String, Option<String>) {
    match record {
        ObservationRecord::Track(t) => ("track", t.external_id.clone(), None),
        ObservationRecord::Collection(c) => ("collection", c.external_id.clone(), None),
        ObservationRecord::Membership {
            collection_id,
            occurrence,
        } => (
            "membership",
            occurrence.external_occurrence_id.clone(),
            Some(collection_id.clone()),
        ),
        ObservationRecord::Issue(i) => ("issue", i.record_id.clone().unwrap_or_default(), None),
    }
}
impl ObservationSink for Spool {
    fn put_blob(&mut self, bytes: &[u8], media_type: Option<&str>) -> Result<BlobRef> {
        ensure!(
            bytes.len() as u64 <= self.limits.blob_bytes,
            "Import blob exceeds byte limit"
        );
        let hash = format!("{:x}", Sha256::digest(bytes));
        let exists: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM blobs WHERE sha256=?1)",
            [&hash],
            |r| r.get(0),
        )?;
        if !exists {
            self.bytes = self
                .bytes
                .checked_add(bytes.len() as u64)
                .context("Import spool size overflow")?;
            ensure!(
                self.bytes <= self.limits.snapshot_bytes,
                "Import spool size limit exceeded"
            );
            self.db.execute(
                "INSERT INTO blobs VALUES(?1,?2,?3,?4)",
                params![hash, bytes, bytes.len() as i64, media_type],
            )?;
        }
        Ok(BlobRef {
            sha256: hash,
            bytes: bytes.len() as u64,
            media_type: media_type.map(str::to_owned),
        })
    }
    fn push(&mut self, record: ObservationRecord) -> Result<()> {
        if let ObservationRecord::Track(track) = &record {
            validate_track_observation(track, &self.limits)?;
        }
        let (kind, id, collection) = record_key(&record);
        let encoded = serde_json::to_string(&record)?;
        ensure!(
            encoded.len() as u64 <= self.limits.text_bytes,
            "Import record exceeds byte limit"
        );
        self.bytes = self
            .bytes
            .checked_add(encoded.len() as u64)
            .context("Import spool size overflow")?;
        ensure!(
            self.bytes <= self.limits.snapshot_bytes,
            "Import spool size limit exceeded"
        );
        let hash = format!("{:x}", Sha256::digest(encoded.as_bytes()));
        self.db.execute(
            "INSERT INTO observations VALUES(?1,?2,?3,?4,?5,?6)",
            params![self.sequence, kind, id, collection, encoded, hash],
        )?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .context("Import record count overflow")?;
        Ok(())
    }
}
struct SpoolReader<'a> {
    db: &'a Connection,
    sequence: i64,
    cancel: &'a CancellationToken,
}
impl ObservationReader for SpoolReader<'_> {
    fn next(&mut self) -> Result<Option<ObservationRecord>> {
        self.cancel.check()?;
        let found:Option<(i64,String)>=self.db.query_row("SELECT sequence,json FROM observations WHERE sequence>?1 ORDER BY sequence LIMIT 1",[self.sequence],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        found
            .map(|(sequence, json)| {
                self.sequence = sequence;
                Ok(serde_json::from_str(&json)?)
            })
            .transpose()
    }
}
fn check_blob_references(db: &Connection, cancel: &CancellationToken) -> Result<()> {
    let mut statement =
        db.prepare("SELECT json FROM observations WHERE kind='track' ORDER BY sequence")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        cancel.check()?;
        let record: ObservationRecord = serde_json::from_str(&row.get::<_, String>(0)?)?;
        if let ObservationRecord::Track(track) = record {
            for reference in track
                .file_tags
                .iter()
                .chain(track.locations.raw.iter())
                .chain(track.grid.as_ref().and_then(|g| g.raw.as_ref()))
            {
                let stored: Option<i64> = db
                    .query_row(
                        "SELECT bytes FROM blobs WHERE sha256=?1",
                        [&reference.sha256],
                        |r| r.get(0),
                    )
                    .optional()?;
                ensure!(
                    stored.is_some_and(|n| n as u64 == reference.bytes),
                    "Missing or mismatched import blob reference"
                );
            }
        }
    }
    Ok(())
}
fn provider_key(provider: SourceProvider) -> Result<String> {
    Ok(serde_json::to_string(&provider)?)
}
fn location_key(selection: &ImportSourceSelection) -> Result<String> {
    // Artifact selection is scope, not library identity. Preserve raw native root bytes.
    let root_only = ImportSourceSelection {
        folder_inventory: false,
        root: selection.root.clone(),
        artifacts: Vec::new(),
    };
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&root_only)?)
    ))
}
fn source_id(
    db: &Connection,
    request: &ImportInspectRequest,
    header: &ObservationHeader,
    profile: &SourceProfile,
) -> Result<String> {
    let provider = provider_key(request.provider)?;
    let location = serde_json::to_string(&request.selection)?;
    let root_key = location_key(&request.selection)?;
    let native = &header.source.library_instance_id;
    let id = match &request.binding {
        SourceBindingChoice::NewLibrary => uuid::Uuid::new_v4().to_string(),
        SourceBindingChoice::Replica { source_id } => {
            let current: Option<(String, String)> = db
                .query_row(
                    "SELECT provider,native_instance_id FROM import_sources WHERE id=?1",
                    [source_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            ensure!(
                current == Some((provider.clone(), native.clone())),
                "Replica source identity differs; select a new library"
            );
            source_id.clone()
        }
        SourceBindingChoice::Automatic => {
            let mut query=db.prepare("SELECT id,location_key FROM import_sources WHERE provider=?1 AND native_instance_id=?2 ORDER BY id")?;
            let rows = query
                .query_map(params![provider, native], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let same = rows
                .iter()
                .filter(|(_, l)| l == &root_key)
                .collect::<Vec<_>>();
            ensure!(
                same.len() <= 1,
                "Ambiguous source binding; choose a replica or new library"
            );
            if let Some((id, _)) = same.first() {
                id.clone()
            } else {
                ensure!(
                    rows.is_empty(),
                    "Choose same-source replica or new library for this copied source"
                );
                uuid::Uuid::new_v4().to_string()
            }
        }
    };
    db.execute("INSERT INTO import_sources VALUES(?1,?2,?3,?4,?5,?6,?7,?7) ON CONFLICT(id) DO UPDATE SET location_key=excluded.location_key,location_json=excluded.location_json,profile_json=excluded.profile_json,updated=excluded.updated",params![id,provider,native,root_key,location,serde_json::to_string(profile)?,now()])?;
    Ok(id)
}
#[derive(Serialize, Deserialize)]
struct PageCursor {
    inspection_id: String,
    kind: String,
    sequence: i64,
}
impl Library {
    /// Native commands provision access locally; runtime keys never enter requests.
    pub fn inspect_rekordbox_import(
        &self,
        request: ImportInspectRequest,
        reader: impl Into<Arc<cuerithm_dj_sync::import::readers::rekordbox::RekordboxReader>>,
        cancel: &CancellationToken,
    ) -> Result<ImportInspection> {
        ensure!(
            request.provider == SourceProvider::Rekordbox,
            "Mismatched direct import provider"
        );
        let reader = reader.into();
        let inspection = self.inspect_import_with_reader(request, reader.as_ref(), cancel)?;
        self.inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .rekordbox_readers
            .insert(inspection.source_id.clone(), reader);
        Ok(inspection)
    }
    pub fn inspect_import(
        &self,
        request: ImportInspectRequest,
        cancel: &CancellationToken,
    ) -> Result<ImportInspection> {
        if request.provider == SourceProvider::Rekordbox
            && let SourceBindingChoice::Replica { source_id } = &request.binding
        {
            let reader = self
                .inner
                .import_runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
                .rekordbox_readers
                .get(source_id)
                .cloned();
            if let Some(reader) = reader {
                return self.inspect_import_with_reader(request, reader.as_ref(), cancel);
            }
        }
        let reader = adapter(request.provider);
        self.inspect_import_with_reader(request, reader, cancel)
    }
    pub(crate) fn inspect_import_with_reader(
        &self,
        mut request: ImportInspectRequest,
        reader: &dyn SourceAdapter,
        cancel: &CancellationToken,
    ) -> Result<ImportInspection> {
        cancel.check()?;
        ensure!(
            reader.provider() == request.provider,
            "Import reader/provider mismatch"
        );
        request.selection.root = request
            .selection
            .root
            .canonicalize()
            .context("Selected source root is unavailable")?;
        request.selection.reinventory(request.provider, cancel)?;
        let capture = SourceCaptureRequest::from_selected_root(
            request.provider,
            &request.selection.root,
            request.selection.artifacts.clone(),
        )?;
        let limits = ImportLimits::default();
        let work_root = self.data_dir().join("import-work");
        std::fs::create_dir_all(&work_root)?;
        let snapshot = Arc::new(reader.capture(&capture, &work_root, &limits, cancel)?);
        let profile = reader.inspect(&snapshot)?;
        ensure!(
            profile.provider == request.provider,
            "Import profile/provider mismatch"
        );
        let source_fallback={
            let db=self.conn()?;
            match &request.binding {
                SourceBindingChoice::Replica{source_id}=>db.query_row("SELECT native_instance_id FROM import_sources WHERE id=?1",[source_id],|r|r.get::<_,String>(0)).optional()?,
                SourceBindingChoice::Automatic=>db.query_row("SELECT native_instance_id FROM import_sources WHERE provider=?1 AND location_key=?2 ORDER BY id LIMIT 1",params![provider_key(request.provider)?,location_key(&request.selection)?],|r|r.get::<_,String>(0)).optional()?,
                SourceBindingChoice::NewLibrary=>None,
            }
        }.unwrap_or_else(||uuid::Uuid::new_v4().to_string());
        let options = ReadOptions {
            limits: limits.clone(),
            include_history: request.include_history,
            include_queue: request.include_queue,
            capture_file_tags: request.capture_file_tags,
            origin: request.origin.clone(),
            source_byte_order: None,
            path_mappings: request.path_mappings.clone(),
            library_instance_id: Some(source_fallback),
        };
        let spool_file = tempfile::NamedTempFile::new_in(&work_root)?;
        let db = Connection::open(spool_file.path())?;
        db.execute_batch("PRAGMA cache_size=-2048; CREATE TABLE observations(sequence INTEGER PRIMARY KEY,kind TEXT,record_id TEXT,collection_id TEXT,json TEXT,semantic_hash TEXT); CREATE TABLE blobs(sha256 TEXT PRIMARY KEY,data BLOB,bytes INTEGER,media_type TEXT); BEGIN;")?;
        let mut spool = Spool {
            db,
            limits: limits.clone(),
            bytes: 0,
            sequence: 0,
        };
        let mut header = reader.read(&snapshot, &options, &mut spool, cancel)?;
        ensure!(
            header.source.provider == request.provider
                && header.schema_fingerprint == profile.schema_fingerprint
                && header.snapshot_hash == snapshot.snapshot_hash,
            "Reader returned a mismatched source/profile/snapshot identity"
        );
        spool.db.execute_batch("COMMIT")?;
        let counts = validate_observation(
            &header,
            &mut SpoolReader {
                db: &spool.db,
                sequence: -1,
                cancel,
            },
            &limits,
        )?;
        check_blob_references(&spool.db, cancel)?;
        snapshot.validate_source(cancel)?;
        let inspection_id = uuid::Uuid::new_v4().to_string();
        // Lock order is runtime -> DB throughout import operations.
        let mut runtime = self
            .inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?;
        let mut db = self.conn()?;
        // ATTACH uses the backend-created private path. No UI filename is accepted.
        db.execute(
            "ATTACH DATABASE ?1 AS import_spool",
            [spool_file
                .path()
                .to_str()
                .context("Private spool path is not UTF-8")?],
        )?;
        let stored = (|| -> Result<String> {
            let tx = db.transaction()?;
            let id = source_id(&tx, &request, &header, &profile)?;
            header.extensions.insert(
                "native_library_instance_id".into(),
                serde_json::Value::String(header.source.library_instance_id.clone()),
            );
            header.source.library_instance_id = id.clone();
            tx.execute(
                "INSERT INTO import_snapshots VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    inspection_id,
                    id,
                    serde_json::to_string(&header)?,
                    serde_json::to_string(&profile)?,
                    serde_json::to_string(&request)?,
                    snapshot.snapshot_hash,
                    runtime.nonce,
                    now()
                ],
            )?;
            let last_blob: i64 =
                spool
                    .db
                    .query_row("SELECT COALESCE(MAX(rowid),0) FROM blobs", [], |r| r.get(0))?;
            for row in 1..=last_blob {
                cancel.check()?;
                tx.execute("INSERT INTO import_blobs SELECT sha256,data,bytes,media_type FROM import_spool.blobs WHERE rowid=?1 ON CONFLICT(sha256) DO NOTHING",[row])?;
            }
            let mut sequence = 0;
            while sequence < spool.sequence {
                cancel.check()?;
                let end = sequence.saturating_add(limits.page_size as i64);
                tx.execute("INSERT INTO import_observations SELECT ?1,sequence,kind,record_id,collection_id,json,semantic_hash FROM import_spool.observations WHERE sequence>=?2 AND sequence<?3",params![inspection_id,sequence,end])?;
                sequence = end;
            }
            cancel.check()?;
            tx.commit()?;
            Ok(id)
        })();
        let detached = db.execute_batch("DETACH DATABASE import_spool");
        let source_id = stored?;
        detached?;
        runtime.inspections.insert(inspection_id.clone(), snapshot);
        Ok(ImportInspection {
            inspection_id,
            source_id,
            profile,
            header,
            counts,
        })
    }
    pub fn import_page(&self, inspection_id: &str, query: ImportPageQuery) -> Result<ImportPage> {
        ensure!(
            (1..=1000).contains(&query.limit),
            "Import page size must be in 1..=1000"
        );
        let kind = query.kind.as_str();
        let after = if let Some(cursor) = query.cursor {
            let cursor: PageCursor =
                serde_json::from_str(&cursor).context("Invalid import page cursor")?;
            ensure!(
                cursor.inspection_id == inspection_id
                    && cursor.kind == kind
                    && cursor.sequence >= 0,
                "Import cursor belongs to another inspection"
            );
            cursor.sequence
        } else {
            -1
        };
        let db = self.conn()?;
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_snapshots WHERE id=?1)",
            [inspection_id],
            |r| r.get(0),
        )?;
        ensure!(exists, "Unknown import inspection");
        let total: u64 = db.query_row(
            "SELECT COUNT(*) FROM import_observations WHERE snapshot_id=?1 AND kind=?2",
            params![inspection_id, kind],
            |r| r.get(0),
        )?;
        let mut stmt=db.prepare("SELECT sequence,json FROM import_observations WHERE snapshot_id=?1 AND kind=?2 AND sequence>?3 ORDER BY sequence LIMIT ?4")?;
        let mut rows = stmt.query(params![inspection_id, kind, after, query.limit + 1])?;
        let mut records = Vec::new();
        let mut bytes = 0u64;
        let mut last = after;
        let mut more = false;
        let byte_limit = ImportLimits::default().text_bytes;
        while let Some(row) = rows.next()? {
            let raw: String = row.get(1)?;
            ensure!(
                raw.len() as u64 <= byte_limit,
                "Stored import record exceeds page byte limit"
            );
            if records.len() == query.limit as usize
                || (!records.is_empty() && bytes + raw.len() as u64 > byte_limit)
            {
                more = true;
                break;
            }
            bytes += raw.len() as u64;
            records.push(serde_json::from_str(&raw)?);
            last = row.get(0)?;
        }
        let next_cursor = if more {
            Some(serde_json::to_string(&PageCursor {
                inspection_id: inspection_id.into(),
                kind: kind.into(),
                sequence: last,
            })?)
        } else {
            None
        };
        Ok(ImportPage {
            records,
            total,
            next_cursor,
        })
    }
}

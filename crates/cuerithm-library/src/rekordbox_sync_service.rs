//! Frozen native rekordbox plans, immutable versioned assets and SQLite-last
//! publication. Runtime keys never enter durable operations.
use crate::*;
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::{
    SyncTrack,
    transaction::{
        DirectoryChange, FilePublication, FileTransaction, directory_manifest, file_hash,
    },
};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use cuerithm_rekordbox::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
mod types;
pub use types::*;
mod assets;
mod ledger;
mod planning;
mod publication;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceProof {
    id: String,
    path: PathBuf,
    hash: String,
    epoch: u64,
    local_fields: BTreeMap<String, Value>,
    #[serde(default)]
    native_media: Option<cuerithm_dj_sync::NativeSourceDescriptor>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FieldAcknowledgement {
    local_id: String,
    content_id: String,
    field: String,
    local: Value,
    decision: NativeFieldDecision,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CueOwnershipProof {
    uuid: Option<String>,
    content_uuid: Option<String>,
    raw_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssetSource {
    path: PathBuf,
    hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Operation {
    version: u32,
    id: String,
    target: RekordboxSyncTarget,
    native_target_id: String,
    #[serde(default)]
    native_revision: u64,
    request: RekordboxSyncRequest,
    revision: u64,
    settings_revision: String,
    sources: Vec<SourceProof>,
    asset_sources: Vec<AssetSource>,
    before: NativeDatabaseState,
    after: NativeDatabaseState,
    plan: NativeWritePlan,
    resolved: Vec<NativeResolvedTrack>,
    playlist_plan: Option<NativePlaylistPlan>,
    receipts: Vec<NativePlaylistPublicationReceipt>,
    acknowledgements: Vec<FieldAcknowledgement>,
    preview: RekordboxSyncPreview,
    asset_journal_id: String,
    before_snapshot: Option<PathBuf>,
    before_snapshot_hash: Option<String>,
    ledger_before: ledger::NativeLedger,
    ledger_after_hash: Option<String>,
    status: String,
    created: i64,
    updated: i64,
    outcome: Option<RekordboxSyncOutcome>,
}
fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn root(library: &Library) -> PathBuf {
    library.data_dir().join("sync/rekordbox")
}
fn lock(library: &Library) -> Result<File> {
    crate::dj_sync_service::service_lock(&library.data_dir().join("sync"))
}
fn ensure_not_native_file(target: &Path, candidate: &Path) -> Result<()> {
    if let (Ok(a), Ok(b)) = (fs::metadata(target), fs::metadata(candidate)) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            ensure!(
                a.dev() != b.dev() || a.ino() != b.ino(),
                "Native database cannot be an audio source or analysis asset"
            );
        }
        #[cfg(not(unix))]
        {
            let _ = (a, b);
            ensure!(
                target.canonicalize()? != candidate.canonicalize()?,
                "Native database cannot be an audio source or analysis asset"
            );
        }
    }
    Ok(())
}
fn ensure_closed() -> Result<()> {
    cuerithm_dj_sync::discovery::ensure_closed(cuerithm_dj_sync::DjSoftware::Rekordbox)
}
fn save(connection: &Connection, op: &Operation) -> Result<()> {
    connection.execute("INSERT INTO rekordbox_sync_operations(id,target_id,database_path,status,journal_json,created,updated) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET status=excluded.status,journal_json=excluded.journal_json,updated=excluded.updated",params![op.id,op.target.id,op.target.database.to_str().context("Native database path is not Unicode")?,op.status,serde_json::to_string(op)?,op.created,op.updated])?;
    Ok(())
}
fn load(library: &Library, id: &str) -> Result<Operation> {
    ensure!(
        uuid::Uuid::parse_str(id)?.to_string() == id,
        "Invalid native operation identity"
    );
    let value: String = library.conn()?.query_row(
        "SELECT journal_json FROM rekordbox_sync_operations WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    let op: Operation = serde_json::from_str(&value)?;
    ensure!(
        op.id == id && op.version == 1,
        "Unsupported native operation journal"
    );
    Ok(op)
}
fn ensure_settled(library: &Library, database: &Path) -> Result<()> {
    let busy:bool=library.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM rekordbox_sync_operations WHERE database_path=?1 AND status IN ('prepared','publishing','native_published','restoring'))",[database.to_str().context("Native path is not Unicode")?],|r|r.get(0))?;
    ensure!(
        !busy,
        "Discard the existing preview or recover the interrupted rekordbox operation first"
    );
    Ok(())
}
fn targets(paths: &DiscoveryPaths, cancel: &CancellationToken) -> Result<Vec<RekordboxSyncTarget>> {
    let discovery = discover_targets(paths, cancel)?;
    let Some(database) = discovery
        .database
        .filter(|d| d.state == DatabaseLocationState::Present)
    else {
        return Ok(vec![]);
    };
    let database = database.path.canonicalize()?;
    let asset_root = database
        .parent()
        .context("Native asset root missing")?
        .join("share");
    let options_file = paths.agent_options_file()?;
    discovery
        .installations
        .into_iter()
        .filter(|i| matches!(i.major, 6 | 7))
        .map(|installation| {
            let id = digest(&json!([installation, &database, &options_file]))?;
            Ok(RekordboxSyncTarget {
                id,
                installation,
                database: database.clone(),
                options_file: options_file.clone(),
                asset_root: asset_root.clone(),
            })
        })
        .collect()
}
fn resolve_target(
    id: &str,
    paths: &DiscoveryPaths,
    cancel: &CancellationToken,
) -> Result<RekordboxSyncTarget> {
    targets(paths, cancel)?
        .into_iter()
        .find(|t| t.id == id)
        .context("Selected rekordbox installation or database changed; discover targets again")
}
fn key_for(
    target: &RekordboxSyncTarget,
    platform: DesktopPlatform,
    cancel: &CancellationToken,
) -> Result<DatabaseKey> {
    resolve_database_key(
        &NativeKeySource::for_installation(&target.installation, platform)?,
        &target.options_file,
        KeyProviderOptions::default(),
        cancel,
    )
}
fn require_capability(
    target: &RekordboxSyncTarget,
    schema: &DatabaseSchema,
    cancel: &CancellationToken,
) -> Result<()> {
    ensure!(
        accepted_write_capability_cancellable(&target.installation, schema, cancel)?.can_write(),
        "This installed rekordbox application and database profile has not passed native write acceptance"
    );
    Ok(())
}
fn verify_local_rows(connection: &Connection, op: &Operation) -> Result<()> {
    let revision: u64 =
        connection.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
            r.get(0)
        })?;
    ensure!(
        revision == op.revision,
        "Library changed; create a new rekordbox preview"
    );
    let selected: String = connection.query_row(
        "SELECT revision FROM selected_settings WHERE singleton=1",
        [],
        |r| r.get(0),
    )?;
    ensure!(
        selected == op.settings_revision,
        "Preparation settings changed; create a new rekordbox preview"
    );
    for source in &op.sources {
        let (epoch,ready,path,hash):(u64,bool,Vec<u8>,String)=connection.query_row("SELECT e.epoch,t.state='ready' AND t.archived=0,t.path,t.source_hash FROM tracks t JOIN maintenance_track_epochs e ON e.id=t.id WHERE t.id=?1",[&source.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        ensure!(
            ready
                && hash == source.hash
                && crate::model::bytes_path(&path)? == source.path
                && epoch == source.epoch
                && crate::native_sync::local_fields(connection, &source.id)? == source.local_fields,
            "Prepared track changed; create a new rekordbox preview"
        );
    }
    if let Some(plan) = &op.playlist_plan {
        ensure!(
            crate::playlists::revision(connection)? == plan.playlist_revision
                && crate::native_playlist_sync::source_fingerprint(connection)?
                    == plan.source_fingerprint,
            "Local playlist or MyTag changed; create a new rekordbox preview"
        );
    }
    Ok(())
}
fn verify_native_observation(connection: &Connection, op: &Operation) -> Result<()> {
    let revision: u64 = connection.query_row(
        "SELECT revision FROM native_targets WHERE id=?1",
        [&op.native_target_id],
        |row| row.get(0),
    )?;
    ensure!(
        revision == op.native_revision,
        "Recorded native observation changed; create a new rekordbox preview"
    );
    Ok(())
}
fn verify_source_bytes(op: &Operation, cancel: &CancellationToken) -> Result<()> {
    for source in &op.sources {
        cancel.check()?;
        ensure_not_native_file(&op.target.database, &source.path)?;
        ensure!(
            file_sha256_cancellable(&source.path, cancel)? == source.hash,
            "Audio source changed; create a new rekordbox preview"
        );
    }
    for asset in &op.asset_sources {
        cancel.check()?;
        ensure_not_native_file(&op.target.database, &asset.path)?;
        ensure!(
            file_sha256_cancellable(&asset.path, cancel)? == asset.hash,
            "Original native asset changed; create a new rekordbox preview"
        );
    }
    Ok(())
}
fn verify_sources(library: &Library, op: &Operation, cancel: &CancellationToken) -> Result<()> {
    {
        let connection = library.conn()?;
        verify_local_rows(&connection, op)?;
    }
    verify_source_bytes(op, cancel)
}
impl Library {
    pub fn discover_rekordbox_targets(&self) -> Result<Vec<RekordboxSyncTarget>> {
        targets(&DiscoveryPaths::current()?, &CancellationToken::default())
    }
    pub fn preflight_rekordbox_sync(
        &self,
        target_id: &str,
        cancel: &CancellationToken,
    ) -> Result<RekordboxSyncPreflight> {
        let paths = DiscoveryPaths::current()?;
        let target = resolve_target(target_id, &paths, cancel)?;
        let mut result = RekordboxSyncPreflight {
            target: target.clone(),
            readable: false,
            writable: false,
            restrictions: vec![],
            schema_fingerprint: None,
            identity: None,
        };
        let inspected = (|| -> Result<()> {
            let key = key_for(&target, paths.platform, cancel)?;
            let db = ReadOnlyDatabase::open(&target.database, &key)?;
            let schema = db.schema()?;
            result.schema_fingerprint = Some(schema.fingerprint.clone());
            result.identity = Some(
                db.read_collection(CollectionReadOptions::default(), cancel)?
                    .identity,
            );
            result.readable = true;
            ensure_closed()?;
            ensure_settled(self, &target.database)?;
            require_capability(&target, &schema, cancel)?;
            result.writable = true;
            Ok(())
        })();
        if let Err(error) = inspected {
            cancel.check()?;
            result.restrictions.push(error.to_string());
        }
        Ok(result)
    }
    pub fn preview_rekordbox_sync(
        &self,
        request: RekordboxSyncRequest,
        runtime: &RekordboxSyncRuntime,
        cancel: &CancellationToken,
    ) -> Result<RekordboxSyncPreview> {
        let _lock = lock(self)?;
        cancel.check()?;
        let target = resolve_target(&request.target_id, &runtime.discovery_paths, cancel)?;
        ensure_settled(self, &target.database)?;
        ensure_closed()?;
        let key = key_for(&target, runtime.discovery_paths.platform, cancel)?;
        planning::preview(
            self,
            request,
            target,
            &key,
            runtime.media_runtime.as_ref(),
            cancel,
            planning::PreviewPermit::Production,
        )
    }
    pub fn discard_rekordbox_sync(&self, id: &str) -> Result<()> {
        let _lock = lock(self)?;
        let op = load(self, id)?;
        ensure!(
            op.status == "prepared",
            "Only an unpublished rekordbox preview can be discarded"
        );
        FileTransaction::discard(&root(self).join("assets"), &op.asset_journal_id)?;
        self.conn()?.execute(
            "DELETE FROM rekordbox_sync_operations WHERE id=?1 AND status='prepared'",
            [id],
        )?;
        let path = root(self).join("operations").join(id);
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }
    pub fn history_rekordbox_sync(&self) -> Result<Vec<RekordboxSyncHistoryEntry>> {
        let connection = self.conn()?;
        let mut statement = connection.prepare(
            "SELECT journal_json FROM rekordbox_sync_operations ORDER BY created DESC,id DESC",
        )?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|s| {
                let op: Operation = serde_json::from_str(&s)?;
                Ok(RekordboxSyncHistoryEntry {
                    id: op.id,
                    target_id: op.target.id,
                    status: op.status,
                    created: op.created,
                    updated: op.updated,
                    outcome: op.outcome,
                })
            })
            .collect()
    }
}

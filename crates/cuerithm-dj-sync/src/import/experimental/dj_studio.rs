//! A deliberately unexportable-to-production root capability for generated data.
use super::journal::*;
use anyhow::{Context, Result, bail, ensure};
use cuerithm_media::CancellationToken;
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const DB: &str = "Database/studio.db";
const AUDIO: &str = "Database/audio-library-table/3f/file_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CACHE: &str = "Cache/Database/audio-library-table.json";
const RENDERER: &str = "dbc14cf914187b40bb359ad570ffb4c4a95e83b8";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExistingRecordPatch {
    pub idempotency_key: String,
    pub metadata: BTreeMap<String, String>,
    pub effective_key: Option<i64>,
}
#[derive(Debug, Clone, Copy)]
pub enum ExperimentFault {
    FailAt(usize),
    InterruptAt(usize),
}
#[derive(Debug)]
struct Interrupted;
impl std::fmt::Display for Interrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Simulated interrupted fixture publication")
    }
}
impl std::error::Error for Interrupted {}

struct Inner {
    path: PathBuf,
    nonce: String,
    identity: u64,
    control_identity: u64,
    marker_hash: String,
    current: Mutex<Inventory>,
    plans: Mutex<BTreeMap<String, String>>,
    fault: Mutex<Option<ExperimentFault>>,
    running: AtomicBool,
}
/// Minted only by `generate_disposable_fixture`; no constructor accepts a path.
#[derive(Clone)]
pub struct DisposableExperimentRoot(Arc<Inner>);
impl DisposableExperimentRoot {
    pub fn path(&self) -> &Path {
        &self.0.path
    }
    pub fn simulate_process_running(&self, running: bool) {
        self.0.running.store(running, Ordering::SeqCst);
    }
    pub fn inject_fault(&self, fault: Option<ExperimentFault>) {
        *self.0.fault.lock().expect("fixture fault mutex") = fault;
    }
    fn control(&self) -> PathBuf {
        self.path().join(".cuerithm-experiment")
    }
    fn check(&self, cancel: &CancellationToken) -> Result<()> {
        cancel.check()?;
        ensure!(
            !self.0.running.load(Ordering::SeqCst),
            "DJ.Studio must be closed"
        );
        ensure!(
            !fs::symlink_metadata(self.path())?.file_type().is_symlink()
                && identity(self.path())? == self.0.identity,
            "Generated root identity changed"
        );
        ensure!(
            !fs::symlink_metadata(self.control())?
                .file_type()
                .is_symlink()
                && identity(&self.control())? == self.0.control_identity,
            "Control directory identity changed"
        );
        ensure!(
            crate::transaction::file_hash(&self.path().join(".fixture.json"))?.as_deref()
                == Some(&self.0.marker_hash),
            "Unknown native/renderer fixture profile"
        );
        for suffix in ["-wal", "-shm", "-journal"] {
            ensure!(
                !self.path().join(format!("{DB}{suffix}")).exists(),
                "Fixture SQLite is not closed"
            );
        }
        ensure_closed_processes()?;
        Ok(())
    }
    fn lease(&self, cancel: &CancellationToken) -> Result<File> {
        self.check(cancel)?;
        let path = self.control().join("lease");
        if path.exists() {
            ensure!(
                !fs::symlink_metadata(&path)?.file_type().is_symlink(),
                "Unsafe fixture lease"
            );
        }
        let lease = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        lease
            .try_lock_exclusive()
            .context("Another fixture experiment owns this root")?;
        Ok(lease)
    }
    fn hit(&self, index: usize) -> Result<()> {
        let mut fault = self
            .0
            .fault
            .lock()
            .map_err(|_| anyhow::anyhow!("Fixture fault mutex poisoned"))?;
        match *fault {
            Some(ExperimentFault::FailAt(at)) if at == index => {
                *fault = None;
                bail!("Injected fixture transition failure {index}")
            }
            Some(ExperimentFault::InterruptAt(at)) if at == index => {
                *fault = None;
                return Err(Interrupted.into());
            }
            _ => {}
        }
        Ok(())
    }
}
pub struct DjStudioExperimentPreview {
    root: DisposableExperimentRoot,
    id: String,
    transitions: usize,
}
impl DjStudioExperimentPreview {
    pub fn transition_count(&self) -> usize {
        self.transitions
    }
}
#[derive(Debug, Clone)]
pub struct ExperimentReceipt {
    pub operation_id: String,
    pub production_write_enabled: bool,
}
#[derive(Debug, Default)]
pub struct ExperimentRecovery {
    pub restored: usize,
}

fn ensure_closed_processes() -> Result<()> {
    fn is_app(name: &str) -> bool {
        let name = name.to_lowercase();
        ["dj.studio", "djstudio", "dj-studio"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
    }
    #[cfg(target_os = "linux")]
    {
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            if !entry
                .file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
            {
                continue;
            }
            match fs::read(entry.path().join("comm")) {
                Ok(bytes) => ensure!(
                    !is_app(String::from_utf8_lossy(&bytes).trim()),
                    "DJ.Studio/helper process is running"
                ),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e).context("Cannot establish closed fixture process state"),
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        #[cfg(windows)]
        let output = std::process::Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output()?;
        #[cfg(not(windows))]
        let output = std::process::Command::new("ps")
            .args(["-A", "-o", "comm="])
            .output()?;
        ensure!(
            output.status.success(),
            "Cannot inspect running applications"
        );
        for line in String::from_utf8(output.stdout)?.lines() {
            let name = line
                .trim_matches('"')
                .split(['"', ','])
                .next()
                .unwrap_or("");
            ensure!(
                !is_app(
                    Path::new(name)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(name)
                ),
                "DJ.Studio/helper process is running"
            );
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    fs::create_dir_all(path.parent().context("Missing fixture parent")?)?;
    fs::write(path, serde_json::to_vec(value)?)?;
    Ok(())
}
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn generate_database(path: &Path, fixture: &Value) -> Result<()> {
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../../docs/research/dj-library-import-handover/fixtures/djstudio-sqlite-schema.json"
    ))?;
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA foreign_keys=ON;")?;
    for (name, table) in manifest["tables"].as_object().context("Fixture tables")? {
        let mut columns = Vec::new();
        for col in table["columns"].as_array().context("Fixture columns")? {
            let mut ddl = format!(
                "{} {}",
                quoted(col["name"].as_str().context("Column name")?),
                col["declaredType"].as_str().context("Column type")?
            );
            if col["primaryKey"] == 1 {
                ddl.push_str(" PRIMARY KEY AUTOINCREMENT");
            }
            if col["notNull"] == 1 {
                ddl.push_str(" NOT NULL");
            }
            if let Some(default) = col["default"].as_str() {
                ddl.push_str(" DEFAULT ");
                ddl.push_str(default);
            }
            columns.push(ddl);
        }
        for index in table["indexes"].as_array().context("Fixture indexes")? {
            if index["origin"] == "u" {
                columns.push(format!(
                    "UNIQUE({})",
                    index["columns"]
                        .as_array()
                        .context("Index columns")?
                        .iter()
                        .map(|v| quoted(v.as_str().unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
        }
        for fk in table["foreignKeys"]
            .as_array()
            .context("Foreign keys")?
            .iter()
            .rev()
        {
            columns.push(format!(
                "FOREIGN KEY({}) REFERENCES {}({})",
                quoted(fk["from"].as_str().context("FK from")?),
                quoted(fk["table"].as_str().context("FK table")?),
                quoted(fk["to"].as_str().context("FK to")?)
            ));
        }
        db.execute_batch(&format!(
            "CREATE TABLE {}({});",
            quoted(name),
            columns.join(",")
        ))?;
        for index in table["indexes"].as_array().context("Fixture indexes")? {
            if index["origin"] == "c" {
                db.execute_batch(&format!(
                    "CREATE INDEX {} ON {}({});",
                    quoted(index["name"].as_str().context("Index name")?),
                    quoted(name),
                    index["columns"]
                        .as_array()
                        .context("Index columns")?
                        .iter()
                        .map(|v| quoted(v.as_str().unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join(",")
                ))?;
            }
        }
    }
    for name in manifest["migrationNames"]
        .as_array()
        .context("Migrations")?
    {
        db.execute(
            "INSERT INTO migrations(name) VALUES(?1)",
            [name.as_str().context("Migration name")?],
        )?;
    }
    for table in ["Tracks", "Playlists", "PlaylistTracks", "Folders"] {
        for row in fixture["sqlite"][table]
            .as_array()
            .context("Fixture rows")?
        {
            let values = row.as_object().context("Fixture row")?;
            let data = values
                .values()
                .map(|v| match v {
                    Value::Null => rusqlite::types::Value::Null,
                    Value::String(s) => rusqlite::types::Value::Text(s.clone()),
                    Value::Number(n) if n.is_i64() => {
                        rusqlite::types::Value::Integer(n.as_i64().unwrap_or_default())
                    }
                    Value::Number(n) => {
                        rusqlite::types::Value::Real(n.as_f64().unwrap_or_default())
                    }
                    _ => unreachable!("Pinned scalar fixture"),
                })
                .collect::<Vec<_>>();
            db.execute(
                &format!(
                    "INSERT INTO {}({}) VALUES({})",
                    quoted(table),
                    values
                        .keys()
                        .map(|k| quoted(k))
                        .collect::<Vec<_>>()
                        .join(","),
                    vec!["?"; data.len()].join(",")
                ),
                rusqlite::params_from_iter(data),
            )?;
        }
    }
    db.execute("INSERT INTO Libraries(id,type) VALUES(1,'synthetic')", [])?;
    db.execute("INSERT INTO TrackLibraryMap(trackId,libraryId,identifier) VALUES(101,1,'external-opaque-id')",[])?;
    integrity(&db)?;
    db.close().map_err(|(_, e)| e)?;
    Ok(())
}

/// Creates invented data in a newly allocated empty temporary directory only.
/// Evidence is retained for inspection; this capability never adopts an existing path.
pub fn generate_disposable_fixture() -> Result<DisposableExperimentRoot> {
    let path = tempfile::tempdir()?.keep();
    let nonce = uuid::Uuid::new_v4().to_string();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/research/dj-library-import-handover/fixtures/djstudio-synthetic-records.json"
    ))?;
    fs::create_dir_all(path.join("Database"))?;
    generate_database(&path.join(DB), &fixture)?;
    let mut audio = fixture["audio"].clone();
    audio["unowned"] = json!({"typed":[true,null,42,"keep"]});
    write_json(&path.join(AUDIO), &audio)?;
    write_json(
        &path.join(
            fixture["paths"]["structure"]
                .as_str()
                .context("Structure path")?,
        ),
        &fixture["structure"],
    )?;
    for relative in [
        CACHE,
        "Cache/Database/track-structures-table.json",
        "Database/projects-table/0/synthetic-project",
        "Database/projects-meta-table/0/synthetic-project",
        "Images/synthetic-image",
        "Stems/synthetic-stem",
    ] {
        write_json(
            &path.join(relative),
            &json!({"generated":relative,"untouched":[1,null,"keep"]}),
        )?;
    }
    write_json(
        &path.join(".fixture.json"),
        &json!({"kind":"generated-disposable-only","nonce":nonce,"native":"4.2.9","renderer":RENDERER}),
    )?;
    fs::create_dir(path.join(".cuerithm-experiment"))?;
    let inner = Inner {
        identity: identity(&path)?,
        control_identity: identity(&path.join(".cuerithm-experiment"))?,
        marker_hash: crate::transaction::file_hash(&path.join(".fixture.json"))?
            .context("Marker missing")?,
        current: Mutex::new(inventory(&path)?),
        plans: Mutex::new(BTreeMap::new()),
        fault: Mutex::new(None),
        running: AtomicBool::new(false),
        path,
        nonce,
    };
    Ok(DisposableExperimentRoot(Arc::new(inner)))
}

fn integrity(db: &Connection) -> Result<()> {
    ensure!(
        db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))? == "ok",
        "Experiment database integrity failed"
    );
    ensure!(
        !db.prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some(),
        "Experiment foreign keys failed"
    );
    Ok(())
}
fn validate_patch(patch: &ExistingRecordPatch) -> Result<()> {
    ensure!(
        !patch.idempotency_key.is_empty() && patch.idempotency_key.len() <= 128,
        "Invalid experiment idempotency key"
    );
    ensure!(
        patch
            .metadata
            .iter()
            .all(|(k, v)| matches!(k.as_str(), "title" | "artist" | "comment") && v.len() <= 4096),
        "Patch exceeds existing metadata scope"
    );
    ensure!(
        patch.effective_key.is_none_or(|v| (0..24).contains(&v)),
        "Unknown effective key"
    );
    Ok(())
}
fn stage_path(dir: &Path, index: usize, kind: &str) -> PathBuf {
    dir.join(format!("{kind}-{index}"))
}
fn journal_path(dir: &Path) -> PathBuf {
    dir.join("journal.json")
}
fn trusted(root: &DisposableExperimentRoot, dir: &Path) -> Result<Journal> {
    let j = load(&journal_path(dir))?;
    ensure!(j.plan.root_nonce == root.0.nonce, "Foreign fixture plan");
    let plans = root
        .0
        .plans
        .lock()
        .map_err(|_| anyhow::anyhow!("Plan mutex poisoned"))?;
    ensure!(
        plans.get(&j.plan.id) == Some(&digest(&j.plan)?),
        "Untrusted fixture journal"
    );
    Ok(j)
}

pub fn preview_djstudio_experiment(
    root: &DisposableExperimentRoot,
    patch: ExistingRecordPatch,
    cancel: &CancellationToken,
) -> Result<DjStudioExperimentPreview> {
    validate_patch(&patch)?;
    let _lease = root.lease(cancel)?;
    let id = digest(&patch.idempotency_key)?;
    let dir = root.control().join(&id);
    if dir.exists() {
        let j = trusted(root, &dir)?;
        ensure!(
            j.plan.patch == patch,
            "Idempotency key already has another patch"
        );
        return Ok(DjStudioExperimentPreview {
            root: root.clone(),
            id,
            transitions: 5 + 2 * j.plan.changes.len(),
        });
    }
    let before = inventory(root.path())?;
    ensure!(
        before
            == *root
                .0
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("Inventory mutex poisoned"))?,
        "Fixture changed before preview"
    );
    fs::create_dir(&dir)?;
    let result = (|| -> Result<Journal> {
        fs::create_dir(dir.join("complete-backup"))?;
        for (relative, entry) in &before {
            cancel.check()?;
            if entry.directory {
                continue;
            }
            let target = dir.join("complete-backup").join(relative);
            fs::create_dir_all(target.parent().context("Backup parent")?)?;
            fs::copy(safe(root.path(), relative)?, &target)?;
            File::open(&target)?.sync_all()?;
            ensure!(
                crate::transaction::file_hash(&target)? == entry.hash,
                "Backup verification failed"
            );
        }
        let sql = dir.join("sql-stage");
        fs::copy(root.path().join(DB), &sql)?;
        let mut db = Connection::open(&sql)?;
        db.execute_batch("PRAGMA foreign_keys=ON;")?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for field in ["title", "artist"] {
            if let Some(value) = patch.metadata.get(field) {
                ensure!(
                    tx.execute(
                        &format!("UPDATE Tracks SET {field}=?1 WHERE id=101"),
                        [value]
                    )? == 1,
                    "Existing record missing"
                );
            }
        }
        if let Some(key) = patch.effective_key {
            ensure!(
                tx.execute("UPDATE Tracks SET key=?1 WHERE id=101", [key])? == 1,
                "Existing record missing"
            );
        }
        integrity(&tx)?;
        tx.commit()?;
        db.close().map_err(|(_, e)| e)?;
        File::open(&sql)?.sync_all()?;
        let mut audio: Value = serde_json::from_slice(&fs::read(root.path().join(AUDIO))?)?;
        ensure!(audio["tag"].is_object(), "Existing metadata missing");
        for (field, value) in &patch.metadata {
            audio["tag"][field] = Value::String(value.clone());
        }
        if let Some(key) = patch.effective_key {
            audio["camelotKey"] = json!(key);
        }
        let json_stage = dir.join("json-stage");
        fs::write(&json_stage, serde_json::to_vec(&audio)?)?;
        File::open(&json_stage)?.sync_all()?;
        let mut changes = Vec::new();
        for (relative, stage) in [(AUDIO, Some(json_stage)), (DB, Some(sql)), (CACHE, None)] {
            let entry = before
                .get(relative)
                .context("Participating generated file missing")?;
            let hash = entry.hash.clone().context("Fixture target is not a file")?;
            let index = changes.len();
            fs::copy(
                root.path().join(relative),
                stage_path(&dir, index, "before"),
            )?;
            File::open(stage_path(&dir, index, "before"))?.sync_all()?;
            let after = if let Some(stage) = stage {
                fs::rename(stage, stage_path(&dir, index, "after"))?;
                crate::transaction::file_hash(&stage_path(&dir, index, "after"))?
            } else {
                None
            };
            changes.push(Change {
                relative: relative.into(),
                before: hash,
                after,
            });
        }
        root.check(cancel)?;
        ensure!(
            inventory(root.path())? == before,
            "Fixture changed during preparation"
        );
        let j = Journal {
            plan: Plan {
                id: id.clone(),
                root_nonce: root.0.nonce.clone(),
                patch,
                before,
                changes,
            },
            state: State::Prepared,
            replaced: 0,
        };
        save(&journal_path(&dir), &j)?;
        sync(&dir)?;
        Ok(j)
    })();
    let j = match result {
        Ok(j) => j,
        Err(e) => {
            fs::remove_dir_all(&dir)?;
            return Err(e);
        }
    };
    root.0
        .plans
        .lock()
        .map_err(|_| anyhow::anyhow!("Plan mutex poisoned"))?
        .insert(id.clone(), digest(&j.plan)?);
    Ok(DjStudioExperimentPreview {
        root: root.clone(),
        id,
        transitions: 5 + 2 * j.plan.changes.len(),
    })
}

fn verify_final(root: &DisposableExperimentRoot, j: &Journal, dir: &Path) -> Result<()> {
    validate_view(root.path(), &j.plan)?;
    for (index, c) in j.plan.changes.iter().enumerate() {
        ensure!(
            crate::transaction::file_hash(&safe(root.path(), &c.relative)?)? == c.after,
            "Published fixture hash mismatch"
        );
        if c.after.is_some() {
            ensure!(
                crate::transaction::file_hash(&stage_path(dir, index, "after"))? == c.after,
                "Staged fixture evidence changed"
            );
        }
    }
    let db = Connection::open_with_flags(root.path().join(DB), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    integrity(&db)?;
    let expected = Connection::open_with_flags(
        stage_path(dir, 1, "after"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let values = |db: &Connection| -> Result<(String, String, i64, i64)> {
        Ok(db.query_row(
            "SELECT title,artist,key,analyzed FROM Tracks WHERE id=101",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?)
    };
    ensure!(
        values(&db)? == values(&expected)?,
        "Canonical SQL readback mismatch"
    );
    let actual: Value = serde_json::from_slice(&fs::read(root.path().join(AUDIO))?)?;
    let expected: Value = serde_json::from_slice(&fs::read(stage_path(dir, 0, "after"))?)?;
    ensure!(actual == expected, "Canonical JSON readback mismatch");
    Ok(())
}
fn restore(
    root: &DisposableExperimentRoot,
    j: &mut Journal,
    dir: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    root.check(cancel)?;
    validate_view(root.path(), &j.plan)?;
    j.state = State::Restoring;
    save(&journal_path(dir), j)?;
    for (index, c) in j.plan.changes.iter().enumerate().rev() {
        root.check(cancel)?;
        validate_view(root.path(), &j.plan)?;
        ensure!(
            crate::transaction::file_hash(&stage_path(dir, index, "before"))?.as_deref()
                == Some(&c.before),
            "Backup evidence changed"
        );
        publish(
            &stage_path(dir, index, "before"),
            &safe(root.path(), &c.relative)?,
            &root.control(),
        )?;
        root.hit(100 + index)?;
    }
    let current = inventory(root.path())?;
    for (path, entry) in &j.plan.before {
        ensure!(
            current.get(path).map(|e| (&e.hash, e.directory))
                == Some((&entry.hash, entry.directory)),
            "Restoration hash mismatch"
        );
    }
    j.state = State::Restored;
    j.replaced = 0;
    save(&journal_path(dir), j)?;
    *root
        .0
        .current
        .lock()
        .map_err(|_| anyhow::anyhow!("Inventory mutex poisoned"))? = current;
    Ok(())
}

pub fn apply_djstudio_experiment(
    preview: &DjStudioExperimentPreview,
    cancel: &CancellationToken,
) -> Result<ExperimentReceipt> {
    let root = &preview.root;
    let _lease = root.lease(cancel)?;
    let dir = root.control().join(&preview.id);
    let mut j = trusted(root, &dir)?;
    let receipt = ExperimentReceipt {
        operation_id: j.plan.id.clone(),
        production_write_enabled: false,
    };
    if j.state == State::Committed {
        verify_final(root, &j, &dir)?;
        return Ok(receipt);
    }
    ensure!(
        matches!(j.state, State::Prepared | State::Restored),
        "Recover interrupted fixture operation first"
    );
    ensure!(
        inventory(root.path())?
            == *root
                .0
                .current
                .lock()
                .map_err(|_| anyhow::anyhow!("Inventory mutex poisoned"))?,
        "Fixture changed after preview"
    );
    validate_view(root.path(), &j.plan)?;
    for c in &j.plan.changes {
        ensure!(
            crate::transaction::file_hash(&safe(root.path(), &c.relative)?)?.as_deref()
                == Some(&c.before),
            "Stale fixture before-hash"
        );
    }
    let result = (|| -> Result<()> {
        j.state = State::Applying;
        save(&journal_path(&dir), &j)?;
        root.hit(0)?;
        for (index, c) in j.plan.changes.iter().enumerate() {
            root.check(cancel)?;
            validate_view(root.path(), &j.plan)?;
            root.hit(1 + index * 2)?;
            let target = safe(root.path(), &c.relative)?;
            if let Some(after) = &c.after {
                ensure!(
                    crate::transaction::file_hash(&stage_path(&dir, index, "after"))?.as_ref()
                        == Some(after),
                    "Stage changed"
                );
                publish(&stage_path(&dir, index, "after"), &target, &root.control())?;
            } else {
                fs::remove_file(&target)?;
                sync(target.parent().context("Cache parent")?)?;
            }
            j.replaced = index + 1;
            save(&journal_path(&dir), &j)?;
            root.hit(2 + index * 2)?;
        }
        let last = 1 + j.plan.changes.len() * 2;
        root.hit(last)?;
        root.check(cancel)?;
        verify_final(root, &j, &dir)?;
        root.hit(last + 1)?;
        root.hit(last + 2)?;
        j.state = State::Committed;
        save(&journal_path(&dir), &j)?;
        root.hit(last + 3)?;
        *root
            .0
            .current
            .lock()
            .map_err(|_| anyhow::anyhow!("Inventory mutex poisoned"))? = inventory(root.path())?;
        Ok(())
    })();
    if let Err(error) = result {
        if error.downcast_ref::<Interrupted>().is_none() {
            restore(root, &mut j, &dir, &CancellationToken::default())
                .context(error.to_string())?;
        }
        return Err(error);
    }
    Ok(receipt)
}
pub fn recover_djstudio_experiment(
    root: &DisposableExperimentRoot,
    cancel: &CancellationToken,
) -> Result<ExperimentRecovery> {
    let _lease = root.lease(cancel)?;
    let ids = root
        .0
        .plans
        .lock()
        .map_err(|_| anyhow::anyhow!("Plan mutex poisoned"))?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut recovery = ExperimentRecovery::default();
    for id in ids {
        cancel.check()?;
        let dir = root.control().join(id);
        let mut j = trusted(root, &dir)?;
        match j.state {
            State::Applying | State::Restoring => {
                restore(root, &mut j, &dir, cancel)?;
                recovery.restored += 1;
            }
            State::Committed => verify_final(root, &j, &dir)?,
            _ => {}
        }
    }
    Ok(recovery)
}

//! Reviewed file removal with owned backups. All input paths originate in a scan.
use super::*;
use crate::{
    Library,
    model::{bytes_path, path_bytes},
};
use anyhow::{Context, Result, ensure};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Identity {
    device: u64,
    file: u64,
}
fn identity(path: &Path) -> Result<Identity> {
    ensure!(
        std::fs::symlink_metadata(path)?.is_file(),
        "Removal source is not a regular file"
    );
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Removal source is not a regular file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = file.metadata()?;
        Ok(Identity {
            device: meta.dev(),
            file: meta.ino(),
        })
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } != 0,
            "Cannot identify removal source: {}",
            std::io::Error::last_os_error()
        );
        Ok(Identity {
            device: u64::from(info.dwVolumeSerialNumber),
            file: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        })
    }
}
fn guard(path: &Path, root: &Path) -> Result<()> {
    ensure!(
        root.canonicalize()? == root && std::fs::symlink_metadata(root)?.is_dir(),
        "Removal root changed"
    );
    ensure!(
        path.is_absolute() && path.starts_with(root) && path != root,
        "Removal path leaves reviewed root"
    );
    let parent = path.parent().context("Missing removal parent")?;
    ensure!(
        parent.canonicalize()? == parent,
        "Removal parent changed or became a symlink"
    );
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    path: Vec<u8>,
    identity: Identity,
    public: UnusedRemovalFile,
}
#[derive(Serialize, Deserialize)]
struct Preview {
    public: UnusedRemovalPreview,
    root: Vec<u8>,
    entries: Vec<Entry>,
    logic: String,
    consumed_by: Option<String>,
}
fn load<T: serde::de::DeserializeOwned>(conn: &Connection, id: &str, kind: &str) -> Result<T> {
    let text: String = conn.query_row(
        "SELECT payload_json FROM maintenance_scans WHERE id=?1 AND kind=?2",
        params![id, kind],
        |r| r.get(0),
    )?;
    Ok(serde_json::from_str(&text)?)
}
fn entry(
    path: &Path,
    root: &Path,
    role: UnusedRemovalRole,
    cancel: &CancellationToken,
) -> Result<Entry> {
    guard(path, root)?;
    let before = identity(path)?;
    let hash = file_sha256_cancellable(path, cancel)?;
    let size = std::fs::symlink_metadata(path)?.len();
    ensure!(
        identity(path)? == before,
        "Removal source changed while reading"
    );
    use sha2::{Digest, Sha256};
    let id = format!(
        "{:x}",
        Sha256::digest([path_bytes(path), hash.as_bytes().to_vec()].concat())
    );
    Ok(Entry {
        path: path_bytes(path),
        identity: before,
        public: UnusedRemovalFile {
            id,
            display_path: path.to_string_lossy().into(),
            size_bytes: size,
            source_sha256: hash,
            role,
        },
    })
}
// Source helper prefers Serato, then VirtualDJ, and removes only the first present sibling.
fn sibling(path: &Path) -> Result<Option<PathBuf>> {
    let serato = path.with_file_name({
        let mut stem = path.file_stem().context("Missing stem")?.to_os_string();
        stem.push(".1.2.serato-stems");
        stem
    });
    let mut vdj = path.as_os_str().to_os_string();
    vdj.push(".vdjstems");
    for path in [serato, PathBuf::from(vdj)] {
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(Some(path)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(None)
}
#[derive(Clone, Serialize, Deserialize)]
struct MovedFile {
    entry: Entry,
    backup: Vec<u8>,
    displaced: Vec<u8>,
    restored_identity: Option<Identity>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Operation {
    schema: String,
    kind: String,
    state: String,
    root: Vec<u8>,
    workspace: Vec<u8>,
    files: Vec<MovedFile>,
    result: UnusedRemovalResult,
    error: Option<String>,
    undone_by: Option<String>,
    cleanup_complete: bool,
}
pub(crate) fn ensure_unused_unreserved(conn: &Connection, paths: &[Vec<u8>]) -> Result<()> {
    let paths = paths.iter().collect::<BTreeSet<_>>();
    let mut q=conn.prepare("SELECT payload_json FROM maintenance_scans WHERE kind='unused_removal_operation' AND json_extract(payload_json,'$.state') IN ('prepared','conflict')")?;
    for row in q.query_map([], |r| r.get::<_, String>(0))? {
        let op: Operation = serde_json::from_str(&row?)?;
        ensure!(
            !op.files.iter().any(|f| paths.contains(&f.entry.path)),
            "Destination is reserved by pending unused-file recovery"
        );
    }
    Ok(())
}
fn ensure_metadata_unreserved(conn: &Connection, files: &[MovedFile]) -> Result<()> {
    for f in files {
        let busy:i64=conn.query_row("SELECT (SELECT COUNT(*) FROM tag_write_files f JOIN tag_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state='prepared')+(SELECT COUNT(*) FROM cleanup_write_files f JOIN cleanup_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state='prepared')+(SELECT COUNT(*) FROM maintenance_write_files f JOIN maintenance_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state IN ('prepared','conflict'))",[&f.entry.path],|r|r.get(0))?;
        ensure!(
            busy == 0,
            "Destination is reserved by pending metadata publication"
        );
    }
    Ok(())
}
fn entry_paths(entries: &[Entry]) -> Result<Vec<PathBuf>> {
    entries.iter().map(|e| bytes_path(&e.path)).collect()
}
fn validate_entries(entries: &[Entry], root: &Path, cancel: &CancellationToken) -> Result<()> {
    for e in entries {
        cancel.check()?;
        let path = bytes_path(&e.path)?;
        guard(&path, root)?;
        ensure!(
            identity(&path)? == e.identity
                && file_sha256_cancellable(&path, cancel)? == e.public.source_sha256,
            "Removal input changed; scan again"
        );
    }
    Ok(())
}
fn missing(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.into()),
    }
}
fn copy_backup(from: &Path, to: &Path, hash: &str, cancel: &CancellationToken) -> Result<()> {
    use std::io::{Read, Write};
    let _ = identity(from)?;
    let mut input = std::fs::File::open(from)?;
    let mut out = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(to)?;
    let mut chunk = [0u8; 65536];
    loop {
        cancel.check()?;
        let n = input.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        out.write_all(&chunk[..n])?;
    }
    out.sync_all()?;
    std::fs::set_permissions(to, input.metadata()?.permissions())?;
    ensure!(
        file_sha256_cancellable(to, cancel)? == hash,
        "Recovery copy changed"
    );
    Ok(())
}
fn save(conn: &Connection, op: &Operation) -> Result<()> {
    ensure!(conn.execute("UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2 AND kind='unused_removal_operation'",params![serde_json::to_string(op)?,op.result.id])?==1,"Recovery operation disappeared");
    Ok(())
}
fn load_operation(lib: &Library, id: &str) -> Result<Operation> {
    let op: Operation = load(&*lib.conn()?, id, "unused_removal_operation")?;
    ensure!(op.result.id == id, "Recovery operation identity mismatch");
    ensure!(
        op.schema == "unused-removal-v1" && matches!(op.kind.as_str(), "remove" | "restore"),
        "Unsupported unused-file journal"
    );
    let root = lib.data_dir().canonicalize()?.join("maintenance/removal");
    let workspace = bytes_path(&op.workspace)?;
    ensure!(
        workspace.canonicalize()?.starts_with(&root),
        "Recovery workspace leaves application data"
    );
    let targets = op
        .result
        .files
        .iter()
        .map(|f| (&f.id, &f.source_sha256))
        .collect::<BTreeSet<_>>();
    for f in &op.files {
        let path = bytes_path(&f.entry.path)?;
        let backup = bytes_path(&f.backup)?;
        let displaced = bytes_path(&f.displaced)?;
        ensure!(
            targets.contains(&(&f.entry.public.id, &f.entry.public.source_sha256))
                && f.entry.public.display_path == path.to_string_lossy(),
            "Recovery target differs from reviewed file"
        );
        ensure!(
            super::files::resolved_prefix(&backup)?.starts_with(&workspace),
            "Recovery backup leaves its workspace"
        );
        ensure!(
            path.starts_with(bytes_path(&op.root)?)
                && path.parent() == displaced.parent()
                && path != displaced,
            "Recovery path leaves reviewed source folder"
        );
        let name = displaced
            .file_name()
            .context("Missing recovery filename")?
            .to_string_lossy();
        let prefix = if op.kind == "remove" {
            ".cuerithm-unused-"
        } else {
            ".cuerithm-restore-"
        };
        ensure!(
            name.starts_with(&format!("{prefix}{}-", op.result.id)) && name.ends_with(".tmp"),
            "Recovery temporary is not owned by this operation"
        );
    }
    Ok(op)
}
fn rollback_remove(f: &MovedFile, root: &Path) -> Result<()> {
    let path = bytes_path(&f.entry.path)?;
    let displaced = bytes_path(&f.displaced)?;
    guard(&path, root)?;
    if !missing(&displaced)? {
        // Preserve the actual captured source, including a user edit that raced the move.
        let _ = identity(&displaced)?;
        ensure!(
            missing(&path)?,
            "New user file preserved; recovery backup and captured source retained"
        );
        crate::publish_files::create(&displaced, &path)?;
    } else {
        ensure!(
            !missing(&path)?
                && identity(&path)? == f.entry.identity
                && cuerithm_media::file_sha256(&path)? == f.entry.public.source_sha256,
            "Later user edit or missing source preserved; recovery needs review"
        );
    }
    Ok(())
}
fn rollback_restore(f: &MovedFile, root: &Path) -> Result<()> {
    let path = bytes_path(&f.entry.path)?;
    guard(&path, root)?;
    if !missing(&path)? {
        ensure!(
            f.restored_identity.as_ref() == Some(&identity(&path)?)
                && cuerithm_media::file_sha256(&path)? == f.entry.public.source_sha256,
            "New user file preserved; interrupted restore needs review"
        );
        #[cfg(test)]
        tests::after_restore_check(&path);
        let claimed = bytes_path(&f.displaced)?;
        crate::publish_files::create(&path, &claimed)?;
        if f.restored_identity.as_ref() != Some(&identity(&claimed)?)
            || cuerithm_media::file_sha256(&claimed)? != f.entry.public.source_sha256
        {
            // A replacement arrived after the proof. Return the captured newer file
            // without replacing anything else that appeared at the original path.
            crate::publish_files::create(&claimed, &path)?;
            anyhow::bail!("Racing user replacement preserved during restore rollback");
        }
    }
    Ok(())
}
fn cleanup(op: &Operation) -> Result<()> {
    let root = bytes_path(&op.root)?;
    for f in &op.files {
        let temp = bytes_path(&f.displaced)?;
        guard(&temp, &root)?;
        if missing(&temp)? {
            continue;
        }
        let expected = if op.kind == "remove" {
            Some(&f.entry.identity)
        } else {
            f.restored_identity.as_ref()
        };
        ensure!(
            expected == Some(&identity(&temp)?)
                && cuerithm_media::file_sha256(&temp)? == f.entry.public.source_sha256,
            "Unknown recovery temporary preserved"
        );
        std::fs::remove_file(&temp)?;
        crate::publish_files::sync_parent(&temp)?;
    }
    Ok(())
}
fn finish(lib: &Library, op: &mut Operation, error: Option<&anyhow::Error>) -> Result<()> {
    if let Some(error) = error {
        let root = bytes_path(&op.root)?;
        let mut errors = vec![];
        for f in op.files.iter().rev() {
            let restored = if op.kind == "remove" {
                rollback_remove(f, &root)
            } else {
                rollback_restore(f, &root)
            };
            if let Err(e) = restored {
                errors.push(format!("{e:#}"));
            }
        }
        op.state = if errors.is_empty() {
            "rolled_back"
        } else {
            "conflict"
        }
        .into();
        op.error = Some(if errors.is_empty() {
            format!("{error:#}")
        } else {
            format!("{error:#}; recovery needs attention: {}", errors.join("; "))
        });
        save(&*lib.conn()?, op)?;
        if op.state == "conflict" {
            return Ok(());
        }
    }
    match cleanup(op) {
        Ok(()) => op.cleanup_complete = true,
        Err(e) => {
            op.cleanup_complete = false;
            op.error = Some(format!("Recovery cleanup pending: {e:#}"));
        }
    }
    save(&*lib.conn()?, op)
}

impl Library {
    pub fn preview_unused_removal(
        &self,
        scan_id: &str,
        selected: &[String],
        cancel: &CancellationToken,
    ) -> Result<UnusedRemovalPreview> {
        cancel.check()?;
        ensure!(
            !selected.is_empty() && selected.len() <= 10000,
            "Select between 1 and 10000 reviewed files"
        );
        ensure!(
            selected.iter().collect::<BTreeSet<_>>().len() == selected.len(),
            "Repeated selected file"
        );
        let saved = self.removal_scan(scan_id, cancel)?;
        let root = bytes_path(&saved.root)?;
        let mut entries = vec![];
        let mut seen = BTreeSet::new();
        for id in selected {
            let index = saved
                .report
                .files
                .iter()
                .position(|f| &f.id == id)
                .context("Selected file is not in the scan")?;
            let path = bytes_path(&saved.files[index].path)?;
            if seen.insert(path_bytes(&path)) {
                let e = entry(&path, &root, UnusedRemovalRole::Selected, cancel)?;
                ensure!(
                    e.public.source_sha256 == saved.report.files[index].source_sha256,
                    "Selected file changed; scan again"
                );
                entries.push(e);
            }
            if let Some(sibling) = sibling(&path)?
                && seen.insert(path_bytes(&sibling))
            {
                entries.push(entry(&sibling, &root, UnusedRemovalRole::Sibling, cancel)?);
            }
        }
        let paths = entries
            .iter()
            .map(|e| bytes_path(&e.path))
            .collect::<Result<Vec<_>>>()?;
        let conn = self.conn()?;
        ensure!(
            super::scan::stamp(&conn)? == saved.report.stamp,
            "Library changed; scan again"
        );
        super::unused::ensure_unreferenced(&conn, &paths)?;
        let public = UnusedRemovalPreview {
            id: uuid::Uuid::new_v4().to_string(),
            scan_id: scan_id.into(),
            stamp: saved.report.stamp,
            size_bytes: entries.iter().try_fold(0u64, |sum, e| {
                sum.checked_add(e.public.size_bytes)
                    .context("Removal size overflow")
            })?,
            files: entries.iter().map(|e| e.public.clone()).collect(),
        };
        let preview = Preview {
            public: public.clone(),
            root: saved.root,
            entries,
            logic: super::scan::logic_identity(),
            consumed_by: None,
        };
        cancel.check()?;
        conn.execute(
            "INSERT INTO maintenance_scans VALUES(?1,'unused_removal_plan',?2,?3)",
            params![public.id, serde_json::to_string(&preview)?, crate::now()],
        )?;
        Ok(public)
    }
    pub fn apply_unused_removal(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<UnusedRemovalResult> {
        cancel.check()?;
        let mut preview: Preview = load(&*self.conn()?, id, "unused_removal_plan")?;
        ensure!(
            preview.consumed_by.is_none() && preview.logic == super::scan::logic_identity(),
            "Removal preview was consumed or its implementation changed"
        );
        self.removal_scan(&preview.public.scan_id, cancel)?;
        let root = bytes_path(&preview.root)?;
        validate_entries(&preview.entries, &root, cancel)?;
        let cache = self.data_dir().join("maintenance/removal");
        std::fs::create_dir_all(&cache)?;
        let workspace = tempfile::Builder::new()
            .prefix("remove-")
            .tempdir_in(&cache)?;
        let op_id = uuid::Uuid::new_v4().to_string();
        let mut files = vec![];
        for (i, e) in preview.entries.iter().enumerate() {
            let path = bytes_path(&e.path)?;
            let backup = workspace.path().join(format!("original-{i}"));
            copy_backup(&path, &backup, &e.public.source_sha256, cancel)?;
            files.push(MovedFile {
                entry: e.clone(),
                backup: path_bytes(&backup.canonicalize()?),
                displaced: path_bytes(
                    &path
                        .parent()
                        .unwrap()
                        .join(format!(".cuerithm-unused-{op_id}-{i}.tmp")),
                ),
                restored_identity: None,
            });
        }
        validate_entries(&preview.entries, &root, cancel)?;
        // Full scope proofs are rechecked outside the SQLite publication transaction.
        self.removal_scan(&preview.public.scan_id, cancel)?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut op = Operation {
            schema: "unused-removal-v1".into(),
            kind: "remove".into(),
            state: "prepared".into(),
            root: preview.root.clone(),
            workspace: path_bytes(&workspace.path().canonicalize()?),
            files,
            result: UnusedRemovalResult {
                id: op_id.clone(),
                preview_id: id.into(),
                stamp: preview.public.stamp.clone(),
                files: preview.public.files.clone(),
                size_bytes: preview.public.size_bytes,
                restored: false,
                undo_of: None,
            },
            error: None,
            undone_by: None,
            cleanup_complete: false,
        };
        let assets = op
            .files
            .iter()
            .map(|f| bytes_path(&f.backup))
            .collect::<Result<Vec<_>>>()?;
        crate::publish_files::durable_assets(
            &assets,
            &bytes_path(&op.workspace)?,
            &self.data_dir().canonicalize()?,
        )?;
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            ensure!(
                super::scan::stamp(&tx)? == preview.public.stamp,
                "Library changed; scan again"
            );
            super::unused::ensure_unreferenced(&tx, &entry_paths(&preview.entries)?)?;
            let current: Preview = load(&tx, id, "unused_removal_plan")?;
            ensure!(
                current.consumed_by.is_none(),
                "Removal preview already applied"
            );
            ensure_unused_unreserved(
                &tx,
                &op.files
                    .iter()
                    .map(|f| f.entry.path.clone())
                    .collect::<Vec<_>>(),
            )?;
            ensure_metadata_unreserved(&tx, &op.files)?;
            cancel.check()?;
            tx.execute(
                "INSERT INTO maintenance_scans VALUES(?1,'unused_removal_operation',?2,?3)",
                params![op_id, serde_json::to_string(&op)?, crate::now()],
            )?;
            tx.commit()?;
        }
        let _kept = workspace.keep();
        #[cfg(test)]
        tests::checkpoint("prepared");
        let outcome = (|| -> Result<()> {
            let mut conn = self.conn()?;
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            ensure!(
                super::scan::stamp(&tx)? == preview.public.stamp,
                "Library changed before removal"
            );
            let current: Preview = load(&tx, id, "unused_removal_plan")?;
            ensure!(
                current.consumed_by.is_none(),
                "Removal preview already applied"
            );
            validate_entries(&preview.entries, &root, cancel)?;
            super::unused::ensure_unreferenced(&tx, &entry_paths(&preview.entries)?)?;
            for file in &op.files {
                cancel.check()?;
                let path = bytes_path(&file.entry.path)?;
                guard(&path, &root)?;
                ensure!(
                    identity(&path)? == file.entry.identity
                        && file_sha256_cancellable(&path, cancel)?
                            == file.entry.public.source_sha256,
                    "Removal source changed before exchange"
                );
                let displaced = bytes_path(&file.displaced)?;
                crate::publish_files::create(&path, &displaced)?;
                ensure!(
                    identity(&displaced)? == file.entry.identity
                        && file_sha256_cancellable(&displaced, cancel)?
                            == file.entry.public.source_sha256,
                    "Removal source changed during exchange"
                );
            }
            #[cfg(test)]
            tests::checkpoint("files");
            preview.consumed_by = Some(op_id.clone());
            op.state = "committed".into();
            op.result.stamp = super::merge::advance(&tx)?;
            save(&tx, &op)?;
            tx.execute("UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2 AND kind='unused_removal_plan'",params![serde_json::to_string(&preview)?,id])?;
            cancel.check()?;
            tx.commit()?;
            Ok(())
        })();
        finish(self, &mut op, outcome.as_ref().err())?;
        outcome?;
        Ok(op.result)
    }
    pub fn restore_unused_removal(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<UnusedRemovalResult> {
        cancel.check()?;
        let mut original = load_operation(self, id)?;
        ensure!(
            original.kind == "remove"
                && original.state == "committed"
                && original.undone_by.is_none(),
            "Removal is not available for restore"
        );
        let root = bytes_path(&original.root)?;
        let op_id = uuid::Uuid::new_v4().to_string();
        let mut op = original.clone();
        op.kind = "restore".into();
        op.state = "prepared".into();
        op.error = None;
        op.cleanup_complete = false;
        op.result.id = op_id.clone();
        op.result.restored = true;
        op.result.undo_of = Some(id.into());
        for (i, f) in op.files.iter_mut().enumerate() {
            let path = bytes_path(&f.entry.path)?;
            guard(&path, &root)?;
            ensure!(missing(&path)?, "Restore refuses an occupied original path");
            ensure!(
                file_sha256_cancellable(&bytes_path(&f.backup)?, cancel)?
                    == f.entry.public.source_sha256,
                "Recovery backup changed"
            );
            f.displaced = path_bytes(
                &path
                    .parent()
                    .unwrap()
                    .join(format!(".cuerithm-restore-{op_id}-{i}.tmp")),
            );
            f.restored_identity = None;
        }
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let current: Operation = load(&tx, id, "unused_removal_operation")?;
            ensure!(
                current.state == "committed" && current.undone_by.is_none(),
                "Removal already restored"
            );
            ensure_unused_unreserved(
                &tx,
                &op.files
                    .iter()
                    .map(|f| f.entry.path.clone())
                    .collect::<Vec<_>>(),
            )?;
            ensure_metadata_unreserved(&tx, &op.files)?;
            cancel.check()?;
            tx.execute(
                "INSERT INTO maintenance_scans VALUES(?1,'unused_removal_operation',?2,?3)",
                params![op_id, serde_json::to_string(&op)?, crate::now()],
            )?;
            tx.commit()?;
        }
        #[cfg(test)]
        tests::checkpoint("prepared");
        let outcome = (|| -> Result<()> {
            // Copy outside the SQLite transaction; the original paths remain absent.
            for i in 0..op.files.len() {
                cancel.check()?;
                let temp = bytes_path(&op.files[i].displaced)?;
                guard(&temp, &root)?;
                copy_backup(
                    &bytes_path(&op.files[i].backup)?,
                    &temp,
                    &op.files[i].entry.public.source_sha256,
                    cancel,
                )?;
                op.files[i].restored_identity = Some(identity(&temp)?);
                crate::publish_files::sync_parent(&temp)?;
                save(&*self.conn()?, &op)?;
            }
            let mut conn = self.conn()?;
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let current: Operation = load(&tx, id, "unused_removal_operation")?;
            ensure!(
                current.state == "committed" && current.undone_by.is_none(),
                "Removal already restored"
            );
            for f in &op.files {
                cancel.check()?;
                let path = bytes_path(&f.entry.path)?;
                let temp = bytes_path(&f.displaced)?;
                guard(&path, &root)?;
                ensure!(missing(&path)?, "Restore refuses an occupied original path");
                ensure!(
                    f.restored_identity.as_ref() == Some(&identity(&temp)?)
                        && file_sha256_cancellable(&temp, cancel)? == f.entry.public.source_sha256,
                    "Staged restoration changed"
                );
                crate::publish_files::create(&temp, &path)?;
                ensure!(
                    f.restored_identity.as_ref() == Some(&identity(&path)?)
                        && file_sha256_cancellable(&path, cancel)? == f.entry.public.source_sha256,
                    "Restored source changed before commit"
                );
            }
            #[cfg(test)]
            tests::checkpoint("files");
            original.undone_by = Some(op_id.clone());
            original.state = "undone".into();
            op.state = "committed".into();
            op.result.stamp = super::merge::advance(&tx)?;
            save(&tx, &original)?;
            save(&tx, &op)?;
            cancel.check()?;
            tx.commit()?;
            Ok(())
        })();
        finish(self, &mut op, outcome.as_ref().err())?;
        outcome?;
        Ok(op.result)
    }
    pub fn unused_removal_history(&self) -> Result<Vec<UnusedRemovalHistory>> {
        let conn = self.conn()?;
        let mut q = conn.prepare("SELECT payload_json FROM maintenance_scans WHERE kind='unused_removal_operation' ORDER BY created DESC,rowid DESC")?;
        q.query_map([], |r| r.get::<_, String>(0))?
            .map(|row| {
                let op: Operation = serde_json::from_str(&row?)?;
                Ok(UnusedRemovalHistory {
                    result: op.result,
                    state: op.state,
                    error: op.error,
                    undone_by: op.undone_by,
                    cleanup_pending: !op.cleanup_complete,
                })
            })
            .collect()
    }
    pub fn retry_unused_removal_recovery(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<UnusedRemovalHistory>> {
        cancel.check()?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut op = load_operation(self, id)?;
        let error = anyhow::anyhow!("Explicit unused-file recovery retry");
        let pending = matches!(op.state.as_str(), "prepared" | "conflict");
        finish(self, &mut op, pending.then_some(&error))?;
        self.unused_removal_history()
    }
    pub(crate) fn recover_unused_removals(&self) -> Result<()> {
        let ids: Vec<String> = {
            let conn = self.conn()?;
            let mut q = conn.prepare("SELECT id FROM maintenance_scans WHERE kind='unused_removal_operation' AND (json_extract(payload_json,'$.state')='prepared' OR (json_extract(payload_json,'$.cleanup_complete')=0 AND json_extract(payload_json,'$.state') IN ('committed','undone','rolled_back'))) ORDER BY created,rowid")?;
            q.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in ids {
            let mut op = match load_operation(self, &id) {
                Ok(op) => op,
                Err(error) => {
                    let mut raw: Operation = load(&*self.conn()?, &id, "unused_removal_operation")?;
                    if raw.state == "prepared" {
                        raw.state = "conflict".into();
                    }
                    raw.cleanup_complete = false;
                    raw.error = Some(format!("Recovery assets need attention: {error:#}"));
                    save(&*self.conn()?, &raw)?;
                    continue;
                }
            };
            let error = anyhow::anyhow!("Interrupted unused-file operation rolled back");
            let pending = op.state == "prepared";
            finish(self, &mut op, pending.then_some(&error))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    thread_local! { static CRASH: std::cell::RefCell<Option<&'static str>> = const { std::cell::RefCell::new(None) }; }
    pub(super) fn checkpoint(phase: &str) {
        CRASH.with(|slot| {
            if *slot.borrow() == Some(phase) {
                slot.take();
                panic!("simulated unused interruption: {phase}");
            }
        });
    }
    thread_local! { static REPLACE_AFTER_CHECK: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) }; }
    pub(super) fn after_restore_check(path: &Path) {
        REPLACE_AFTER_CHECK.with(|slot| {
            if let Some(witness) = slot.take() {
                std::fs::rename(path, witness).unwrap();
                std::fs::write(path, b"new user file during rollback").unwrap();
            }
        });
    }
    #[test]
    fn rollback_claims_its_restored_file_before_deleting_and_preserves_a_racing_user_replacement() {
        let (dir, lib, path, plan) = setup();
        let token = CancellationToken::default();
        let applied = lib.apply_unused_removal(&plan.id, &token).unwrap();
        let op = load_operation(&lib, &applied.id).unwrap();
        let mut f = op.files[0].clone();
        std::fs::copy(bytes_path(&f.backup).unwrap(), &path).unwrap();
        f.restored_identity = Some(identity(&path).unwrap());
        let witness = dir.path().join("restored-version");
        REPLACE_AFTER_CHECK.with(|slot| *slot.borrow_mut() = Some(witness.clone()));
        assert!(rollback_restore(&f, &bytes_path(&op.root).unwrap()).is_err());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"new user file during rollback"
        );
        assert_eq!(std::fs::read(&witness).unwrap(), b"authored source");
    }
    #[test]
    fn unresolved_recovery_reserves_its_source_against_a_second_reviewed_removal() {
        let (_dir, lib, file, plan) = setup();
        CRASH.with(|slot| *slot.borrow_mut() = Some("files"));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || lib.apply_unused_removal(&plan.id, &CancellationToken::default())
            ))
            .is_err()
        );
        let db = lib.data_dir().join("db");
        drop(lib);
        std::fs::write(&file, b"new user source").unwrap();
        let lib = Library::open(&db).unwrap();
        let token = CancellationToken::default();
        let scan = lib
            .scan_unused_files(
                UnusedScanRequest {
                    root: file.parent().unwrap().to_owned(),
                    extension_mode: ExtensionMode::Include,
                    extensions: vec!["flac".into()],
                },
                &token,
            )
            .unwrap();
        let next = lib
            .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
            .unwrap();
        assert!(lib.apply_unused_removal(&next.id, &token).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"new user source");
    }
    #[test]
    fn missing_recovery_workspace_becomes_visible_conflict_without_preventing_library_open() {
        let (_dir, lib, file, plan) = setup();
        CRASH.with(|slot| *slot.borrow_mut() = Some("prepared"));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || lib.apply_unused_removal(&plan.id, &CancellationToken::default())
            ))
            .is_err()
        );
        let db = lib.data_dir().join("db");
        let id = lib.unused_removal_history().unwrap()[0].result.id.clone();
        let op = load_operation(&lib, &id).unwrap();
        drop(lib);
        std::fs::remove_dir_all(bytes_path(&op.workspace).unwrap()).unwrap();
        let lib =
            Library::open(&db).expect("Recovery conflict must not prevent browsing the library");
        assert_eq!(std::fs::read(&file).unwrap(), b"authored source");
        assert!(
            lib.unused_removal_history()
                .unwrap()
                .iter()
                .any(|h| h.state == "conflict" && h.error.is_some())
        );
    }
    #[test]
    fn changed_recovery_backup_prevents_restore_without_recreating_any_source_file() {
        let (_dir, lib, path, plan) = setup();
        let token = CancellationToken::default();
        let applied = lib.apply_unused_removal(&plan.id, &token).unwrap();
        let op = load_operation(&lib, &applied.id).unwrap();
        std::fs::write(bytes_path(&op.files[0].backup).unwrap(), b"damaged backup").unwrap();
        assert!(lib.restore_unused_removal(&applied.id, &token).is_err());
        assert!(!path.exists());
        assert_eq!(
            load_operation(&lib, &applied.id).unwrap().state,
            "committed"
        );
    }
    fn setup() -> (tempfile::TempDir, Library, PathBuf, UnusedRemovalPreview) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        std::fs::create_dir(&root).unwrap();
        let file = root.join("authored.flac");
        std::fs::write(&file, b"authored source").unwrap();
        let lib = Library::open(&dir.path().join("app/db")).unwrap();
        let token = CancellationToken::default();
        let scan = lib
            .scan_unused_files(
                UnusedScanRequest {
                    root,
                    extension_mode: ExtensionMode::All,
                    extensions: vec![],
                },
                &token,
            )
            .unwrap();
        let plan = lib
            .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
            .unwrap();
        (dir, lib, file, plan)
    }
    #[test]
    fn interrupted_unused_removal_restores_prepared_and_moved_files_on_restart() {
        for phase in ["prepared", "files"] {
            let (dir, lib, file, plan) = setup();
            CRASH.with(|slot| *slot.borrow_mut() = Some(phase));
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || lib.apply_unused_removal(&plan.id, &CancellationToken::default())
                ))
                .is_err()
            );
            drop(lib);
            let lib = Library::open(&dir.path().join("app/db")).unwrap();
            assert_eq!(std::fs::read(&file).unwrap(), b"authored source");
            assert!(
                lib.unused_removal_history()
                    .unwrap()
                    .iter()
                    .any(|h| h.state == "rolled_back")
            );
        }
    }
    #[test]
    fn interrupted_unused_restore_returns_to_removed_state_and_remains_retryable() {
        let (dir, lib, file, plan) = setup();
        let token = CancellationToken::default();
        let applied = lib.apply_unused_removal(&plan.id, &token).unwrap();
        CRASH.with(|slot| *slot.borrow_mut() = Some("files"));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || lib.restore_unused_removal(&applied.id, &token)
            ))
            .is_err()
        );
        drop(lib);
        let lib = Library::open(&dir.path().join("app/db")).unwrap();
        assert!(!file.exists());
        lib.restore_unused_removal(&applied.id, &token).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"authored source");
    }
    #[test]
    fn interrupted_unused_removal_preserves_new_user_files_as_visible_conflicts() {
        let (dir, lib, file, plan) = setup();
        CRASH.with(|slot| *slot.borrow_mut() = Some("files"));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || lib.apply_unused_removal(&plan.id, &CancellationToken::default())
            ))
            .is_err()
        );
        drop(lib);
        std::fs::write(&file, b"new user version").unwrap();
        let lib = Library::open(&dir.path().join("app/db")).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"new user version");
        assert!(
            lib.unused_removal_history()
                .unwrap()
                .iter()
                .any(|h| h.state == "conflict" && h.error.is_some())
        );
        let conflict = lib
            .unused_removal_history()
            .unwrap()
            .into_iter()
            .find(|h| h.state == "conflict")
            .unwrap();
        std::fs::remove_file(&file).unwrap();
        let history = lib
            .retry_unused_removal_recovery(&conflict.result.id, &CancellationToken::default())
            .unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"authored source");
        assert!(history.iter().any(|h| h.result.id == conflict.result.id
            && h.state == "rolled_back"
            && !h.cleanup_pending));
    }
}

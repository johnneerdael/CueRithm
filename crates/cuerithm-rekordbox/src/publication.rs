//! Low-level, exclusive SQLite publication for an owned/authorized collection.
//!
//! This primitive does not establish rekordbox application compatibility. The
//! desktop service must authorize its accepted schema/application profile and
//! continuously check that the native application is closed. Native files are
//! never exchanged or manually checkpointed/deleted here: SQLite publishes and
//! restores through the retained connection and original database inode.
use crate::*;
use rusqlite::{TransactionBehavior, types::ValueRef};
use std::fs::{File, OpenOptions};

const MAX_DATABASE_ROWS: usize = 2_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeDatabaseState {
    pub schema_fingerprint: String,
    /// Canonical whole logical database, including unknown tables and WAL rows.
    pub logical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeApplyResult {
    pub changed_rows: usize,
    pub before_state: NativeDatabaseState,
    pub after_state: NativeDatabaseState,
}

/// Exclusive native ownership survives transaction commits until this lease is
/// dropped. Do not close another raw file descriptor for this inode on Unix:
/// POSIX process locks can be released by closing any descriptor for that file.
pub struct NativeDatabaseLease {
    // Drop SQLite first, then the retained OS identity/delete-denial handle.
    conn: Connection,
    _file: File,
    identity: FileIdentity,
    path: PathBuf,
    requested_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    volume: u64,
    file: u64,
}

#[cfg(unix)]
fn identity_on(file: &File) -> Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok(FileIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(windows)]
fn identity_on(file: &File) -> Result<FileIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut information) } != 0,
        "Read native database file identity"
    );
    Ok(FileIdentity {
        volume: information.dwVolumeSerialNumber as u64,
        file: ((information.nFileIndexHigh as u64) << 32) | information.nFileIndexLow as u64,
    })
}

fn open_identity_handle(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
        // Permit SQLite's reads/writes, but deny deleting/renaming the inode.
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    }
    options
        .open(path)
        .context("Retain native database file identity")
}

fn verify_identity(path: &Path, requested_path: &Path, expected: FileIdentity) -> Result<()> {
    for candidate in [path, requested_path] {
        let metadata =
            std::fs::symlink_metadata(candidate).context("Native database path disappeared")?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Native database path changed"
        );
        #[cfg(unix)]
        let observed = {
            use std::os::unix::fs::MetadataExt;
            FileIdentity {
                volume: metadata.dev(),
                file: metadata.ino(),
            }
        };
        #[cfg(windows)]
        let observed = identity_on(&open_identity_handle(candidate)?)?;
        ensure!(
            observed == expected,
            "Native database file identity changed"
        );
    }
    Ok(())
}

impl NativeDatabaseLease {
    pub fn acquire(path: &Path, key: &DatabaseKey, cancel: &CancellationToken) -> Result<Self> {
        cancel.check()?;
        let metadata = std::fs::symlink_metadata(path).context("Native database unavailable")?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Choose a regular native database file"
        );
        let requested_path = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()?.join(path)
        };
        let path = path
            .canonicalize()
            .context("Resolve native database path")?;
        let file = open_identity_handle(&path)?;
        let identity = identity_on(&file)?;
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .context("Open native database for exclusive ownership")?;
        // MUST precede the first database/WAL access. In WAL this also avoids
        // entering the shared-memory locking protocol. See sqlite.org/wal.html
        // sections 8/9 and sqlite.org/pragma.html#pragma_locking_mode.
        let mode: String =
            conn.pragma_update_and_check(None, "locking_mode", "EXCLUSIVE", |row| row.get(0))?;
        ensure!(
            mode.eq_ignore_ascii_case("exclusive"),
            "Exclusive native ownership unavailable"
        );
        set_key(&conn, key)?;
        conn.busy_timeout(Duration::ZERO)?;
        conn.execute_batch("BEGIN EXCLUSIVE; COMMIT;")
            .map_err(|_| anyhow::anyhow!("Native database is busy or encrypted access failed"))?;
        verify_identity(&path, &requested_path, identity)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        cancel.check()?;
        Ok(Self {
            conn,
            _file: file,
            identity,
            path,
            requested_path,
        })
    }

    pub fn state(&self, cancel: &CancellationToken) -> Result<NativeDatabaseState> {
        verify_identity(&self.path, &self.requested_path, self.identity)?;
        state_on(&self.conn, cancel)
    }

    pub fn read_collection(
        &self,
        options: CollectionReadOptions,
        cancel: &CancellationToken,
    ) -> Result<RekordboxCollection> {
        verify_identity(&self.path, &self.requested_path, self.identity)?;
        collection::read_collection_on(&self.conn, options, cancel)
    }

    /// Capture an encrypted, self-contained before-image while retaining native
    /// ownership. The caller must retain/copy this owned artifact for recovery.
    pub fn snapshot(
        &self,
        workspace: &Path,
        key: &DatabaseKey,
        cancel: &CancellationToken,
    ) -> Result<DatabaseSnapshot> {
        cancel.check()?;
        verify_identity(&self.path, &self.requested_path, self.identity)?;
        std::fs::create_dir_all(workspace)?;
        let owned = tempfile::Builder::new()
            .prefix("cuerithm-rb-before-")
            .tempdir_in(workspace)?;
        let path = owned.path().join("master.db");
        let mut destination = Connection::open(&path)?;
        set_key(&destination, key)?;
        copy_database(&self.conn, &mut destination, cancel)?;
        destination.pragma_update(None, "journal_mode", "DELETE")?;
        let checks = checks_on(&destination, cancel)?;
        let schema = schema_on(&destination)?;
        ensure!(
            state_on(&destination, cancel)? == self.state(cancel)?,
            "Native before-image proof mismatch"
        );
        drop(destination);
        let sha256 = file_sha256_cancellable(&path, cancel)?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)?
            .sync_all()?;
        Ok(DatabaseSnapshot {
            path,
            schema,
            sha256,
            checks,
            _owned: owned,
        })
    }

    pub fn apply_rows(
        &mut self,
        plan: &NativeWritePlan,
        expected: &NativeDatabaseState,
        cancel: &CancellationToken,
    ) -> Result<NativeApplyResult> {
        self.apply_rows_checked(plan, expected, cancel, || Ok(()))
    }

    /// Run the final application-closed guard inside the transaction. Any guard,
    /// cancellation, identity, integrity, schema or row-proof error rolls back.
    pub fn apply_rows_checked(
        &mut self,
        plan: &NativeWritePlan,
        expected: &NativeDatabaseState,
        cancel: &CancellationToken,
        before_commit: impl FnMut() -> Result<()>,
    ) -> Result<NativeApplyResult> {
        self.apply_rows_guarded(plan, expected, None, cancel, before_commit)
    }

    /// Publish only the exact frozen post-image verified during staging. Check
    /// inside the transaction so trigger/default output cannot escape the journal.
    pub fn apply_rows_frozen_checked(
        &mut self,
        plan: &NativeWritePlan,
        expected: &NativeDatabaseState,
        expected_after: &NativeDatabaseState,
        cancel: &CancellationToken,
        before_commit: impl FnMut() -> Result<()>,
    ) -> Result<NativeApplyResult> {
        self.apply_rows_guarded(plan, expected, Some(expected_after), cancel, before_commit)
    }
    fn apply_rows_guarded(
        &mut self,
        plan: &NativeWritePlan,
        expected: &NativeDatabaseState,
        expected_after: Option<&NativeDatabaseState>,
        cancel: &CancellationToken,
        mut before_commit: impl FnMut() -> Result<()>,
    ) -> Result<NativeApplyResult> {
        cancel.check()?;
        verify_identity(&self.path, &self.requested_path, self.identity)?;
        ensure!(
            plan.source.canonicalize()? == self.path,
            "Native write plan targets another database"
        );
        ensure!(
            plan.schema_fingerprint == expected.schema_fingerprint,
            "Native write plan schema proof differs"
        );
        let transaction = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Exclusive)?;
        let before_state = state_on(&transaction, cancel)?;
        ensure!(
            &before_state == expected,
            "Native whole-database state changed"
        );
        checks_on(&transaction, cancel)?;
        let changed_rows = writer::apply_native_rows(&transaction, plan, cancel)?;
        checks_on(&transaction, cancel)?;
        let after_state = state_on(&transaction, cancel)?;
        ensure!(
            after_state.schema_fingerprint == expected.schema_fingerprint,
            "Native publication changed schema"
        );
        ensure!(
            state_on(&transaction, cancel)? == after_state,
            "Native publication proof changed before commit"
        );
        if let Some(expected_after) = expected_after {
            ensure!(
                &after_state == expected_after,
                "Native publication differs from the frozen staged post-image"
            );
        }
        // Keep the application monitor after the potentially large full-state
        // scan, immediately beside the final identity check and commit.
        before_commit()?;
        cancel.check()?;
        verify_identity(&self.path, &self.requested_path, self.identity)?;
        transaction.commit()?;
        Ok(NativeApplyResult {
            changed_rows,
            before_state,
            after_state,
        })
    }

    /// Restore through SQLite's atomic backup transaction on the retained native
    /// inode. Refuse to overwrite any later external edits. The encrypted image
    /// is fully validated before the backup API can write the destination.
    pub fn restore_snapshot(
        &mut self,
        snapshot: &Path,
        key: &DatabaseKey,
        expected_after_state: &NativeDatabaseState,
        cancel: &CancellationToken,
    ) -> Result<NativeDatabaseState> {
        self.restore_snapshot_inner(snapshot, key, None, expected_after_state, cancel)
    }

    /// Restore only the journal's frozen logical before-image. Validate it in
    /// the same retained source transaction used by SQLite's backup API, before
    /// any destination page can change. This also binds WAL-visible source rows.
    pub fn restore_snapshot_checked(
        &mut self,
        snapshot: &Path,
        key: &DatabaseKey,
        expected_before_state: &NativeDatabaseState,
        expected_after_state: &NativeDatabaseState,
        cancel: &CancellationToken,
    ) -> Result<NativeDatabaseState> {
        self.restore_snapshot_inner(
            snapshot,
            key,
            Some(expected_before_state),
            expected_after_state,
            cancel,
        )
    }

    fn restore_snapshot_inner(
        &mut self,
        snapshot: &Path,
        key: &DatabaseKey,
        frozen_before_state: Option<&NativeDatabaseState>,
        expected_after_state: &NativeDatabaseState,
        cancel: &CancellationToken,
    ) -> Result<NativeDatabaseState> {
        cancel.check()?;
        ensure!(
            self.state(cancel)? == *expected_after_state,
            "Native database changed after publication; restore refused"
        );
        ensure!(
            snapshot.canonicalize()? != self.path,
            "Native restore image must be separate"
        );
        let source = ReadOnlyDatabase::open(snapshot, key)?;
        // Keep one coherent source view through validation and copying, even if
        // a recovery image has acquired a WAL or another connection exists.
        let source_transaction = source.conn.unchecked_transaction()?;
        checks_on(&source_transaction, cancel)?;
        let expected_before_state = state_on(&source_transaction, cancel)?;
        if let Some(frozen_before_state) = frozen_before_state {
            ensure!(
                &expected_before_state == frozen_before_state,
                "Native restore before-image differs from the frozen logical proof"
            );
        }
        verify_identity(&self.path, &self.requested_path, self.identity)?;
        copy_database(&source_transaction, &mut self.conn, cancel)?;
        verify_identity(&self.path, &self.requested_path, self.identity)?;
        checks_on(&self.conn, cancel)?;
        let restored = self.state(cancel)?;
        ensure!(
            restored == expected_before_state,
            "Restored native database proof mismatch"
        );
        Ok(restored)
    }
}

fn copy_database(
    source: &Connection,
    destination: &mut Connection,
    cancel: &CancellationToken,
) -> Result<()> {
    let backup = Backup::new(source, destination).context("Initialize encrypted native backup")?;
    let started = Instant::now();
    loop {
        cancel.check()?;
        ensure!(
            started.elapsed() < Duration::from_secs(120),
            "Encrypted native backup timed out"
        );
        match backup
            .step(64)
            .context("Copy encrypted native backup pages")?
        {
            StepResult::Done => return Ok(()),
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => {
                anyhow::bail!("Encrypted native backup became busy")
            }
            _ => anyhow::bail!("Unsupported encrypted native backup result"),
        }
    }
}

fn checks_on(connection: &Connection, cancel: &CancellationToken) -> Result<DatabaseChecks> {
    cancel.check()?;
    let mut integrity = connection.prepare("PRAGMA integrity_check")?;
    let mut rows = integrity.query([])?;
    ensure!(
        rows.next()?
            .is_some_and(|row| row.get::<_, String>(0).is_ok_and(|value| value == "ok"))
            && rows.next()?.is_none(),
        "Native database integrity check failed"
    );
    cancel.check()?;
    let mut foreign_keys = connection.prepare("PRAGMA foreign_key_check")?;
    ensure!(
        foreign_keys.query([])?.next()?.is_none(),
        "Native database has foreign-key issues"
    );
    cancel.check()?;
    Ok(DatabaseChecks {
        integrity_ok: true,
        foreign_key_issues: 0,
    })
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn hash_bytes(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn hash_value(hash: &mut Sha256, value: ValueRef<'_>) {
    match value {
        ValueRef::Null => hash.update([0]),
        ValueRef::Integer(value) => {
            hash.update([1]);
            hash.update(value.to_be_bytes());
        }
        ValueRef::Real(value) => {
            hash.update([2]);
            hash.update(value.to_bits().to_be_bytes());
        }
        ValueRef::Text(value) => {
            hash.update([3]);
            hash_bytes(hash, value);
        }
        ValueRef::Blob(value) => {
            hash.update([4]);
            hash_bytes(hash, value);
        }
    }
}

fn state_on(connection: &Connection, cancel: &CancellationToken) -> Result<NativeDatabaseState> {
    cancel.check()?;
    let schema = schema_on(connection)?;
    let mut hash = Sha256::new();
    hash_bytes(&mut hash, b"CueRithm native logical database v1");
    hash_bytes(&mut hash, schema.fingerprint.as_bytes());
    hash.update(schema.user_version.to_be_bytes());
    hash.update(schema.application_id.to_be_bytes());
    // Include every persisted table, not merely recognized rekordbox entities.
    let tables = {
        let mut query = connection
            .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")?;
        query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut row_count = 0usize;
    for table in tables {
        cancel.check()?;
        hash_bytes(&mut hash, table.as_bytes());
        let quoted = quote_identifier(&table);
        let columns = {
            let mut query = connection.prepare(&format!("PRAGMA table_xinfo({quoted})"))?;
            query
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        ensure!(
            !columns.is_empty(),
            "Native table has no inspectable columns"
        );
        let mut fields = columns
            .iter()
            .map(|name| quote_identifier(name))
            .collect::<Vec<_>>();
        // Preserve implicit row identities as well. WITHOUT ROWID tables have
        // their explicit primary key included in the ordinary columns instead.
        let without_rowid: bool = connection.query_row(
            "SELECT wr FROM pragma_table_list WHERE schema='main' AND name=?1",
            [&table],
            |row| row.get(0),
        )?;
        if !without_rowid {
            if let Some(alias) = ["_rowid_", "rowid", "oid"].into_iter().find(|alias| {
                !columns
                    .iter()
                    .any(|column| column.eq_ignore_ascii_case(alias))
            }) {
                fields.push(quote_identifier(alias));
            } else {
                // A shadowed rowid cannot be inspected safely; refuse to give a
                // whole-database proof that silently omits that identity.
                anyhow::bail!("Native table hides every implicit row identity");
            }
        }
        hash.update((fields.len() as u64).to_be_bytes());
        for field in &fields {
            hash_bytes(&mut hash, field.as_bytes());
        }
        let mut query =
            connection.prepare(&format!("SELECT {} FROM {quoted}", fields.join(",")))?;
        let mut rows = query.query([])?;
        let mut row_hashes = Vec::<[u8; 32]>::new();
        while let Some(row) = rows.next()? {
            cancel.check()?;
            row_count += 1;
            ensure!(
                row_count <= MAX_DATABASE_ROWS,
                "Native whole-database row budget exceeded"
            );
            let mut row_hash = Sha256::new();
            for index in 0..fields.len() {
                hash_value(&mut row_hash, row.get_ref(index)?);
            }
            row_hashes.push(row_hash.finalize().into());
        }
        row_hashes.sort_unstable();
        hash.update((row_hashes.len() as u64).to_be_bytes());
        for row_hash in row_hashes {
            hash.update(row_hash);
        }
    }
    cancel.check()?;
    Ok(NativeDatabaseState {
        schema_fingerprint: schema.fingerprint,
        logical_sha256: format!("{:x}", hash.finalize()),
    })
}

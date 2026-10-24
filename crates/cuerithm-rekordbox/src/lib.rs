//! Native direct-library access. No collection XML transport is provided.
pub mod anlz;
pub mod path_serde;
mod target_context;
pub use target_context::*;
mod asset_staging;
pub use asset_staging::*;
mod collection;
mod discovery;
mod key_provider;
pub use discovery::*;
pub use key_provider::*;
mod import_stream;
mod records;
pub use import_stream::*;
pub use records::NativeColumn;
mod write_profile;
use anyhow::{Context, Result, ensure};
pub use collection::*;
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub use write_profile::*;
mod writer;
pub use writer::*;
mod native_plan;
pub use native_plan::*;
mod native_color;
pub use native_color::*;
mod publication;
pub use publication::*;
use zeroize::Zeroizing;

/// Runtime-only material: never serializable and never included in diagnostics.
pub struct DatabaseKey(Zeroizing<Vec<u8>>);
impl std::fmt::Debug for DatabaseKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DatabaseKey(<redacted>)")
    }
}
impl DatabaseKey {
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self> {
        ensure!(
            !bytes.as_ref().is_empty() && bytes.as_ref().len() <= 16384,
            "Invalid database key length"
        );
        Ok(Self(Zeroizing::new(bytes.as_ref().to_vec())))
    }
}
fn set_key(conn: &Connection, key: &DatabaseKey) -> Result<()> {
    // The native length-bearing API supports quotes/NULs and keeps key material
    // out of SQL strings, error messages and traced statement text.
    let rc = unsafe {
        rusqlite::ffi::sqlite3_key(conn.handle(), key.0.as_ptr().cast(), key.0.len() as i32)
    };
    ensure!(
        rc == rusqlite::ffi::SQLITE_OK,
        "Could not initialize encrypted database access"
    );
    conn.pragma_update(None, "cipher_compatibility", 4)
        .context("Unsupported cipher compatibility mode")?;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaObject {
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(rename = "tbl_name")]
    pub table: String,
    pub sql: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub user_version: i64,
    pub application_id: i64,
    pub fingerprint: String,
    pub objects: Vec<SchemaObject>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseChecks {
    pub integrity_ok: bool,
    pub foreign_key_issues: usize,
}
/// Only read operations are exposed. Writer/profile acceptance is a separate boundary.
pub struct ReadOnlyDatabase {
    conn: Connection,
    cipher_version: String,
}
/// The temporary directory owns the database and any sidecars until this value drops.
pub struct DatabaseSnapshot {
    pub path: PathBuf,
    pub schema: DatabaseSchema,
    pub sha256: String,
    pub checks: DatabaseChecks,
    _owned: tempfile::TempDir,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CaptureProgress {
    pub remaining_pages: i32,
    pub total_pages: i32,
}
impl ReadOnlyDatabase {
    pub fn open(path: &Path, key: &DatabaseKey) -> Result<Self> {
        let meta = std::fs::symlink_metadata(path).context("Database is unavailable")?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "Choose a regular database file"
        );
        let conn = Connection::open_with_flags(
            path.canonicalize().context("Resolve database path")?,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .context("Open read-only database")?;
        let cipher_version: String = conn
            .pragma_query_value(None, "cipher_version", |r| r.get(0))
            .context("Native encrypted database support is unavailable")?;
        ensure!(
            !cipher_version.is_empty(),
            "Native encrypted database support is unavailable"
        );
        set_key(&conn, key)?;
        // Do not propagate a traced key-setting statement or an untrusted SQLite
        // authentication diagnostic to callers. The key never enters SQL text.
        conn.query_row("SELECT COUNT(*) FROM sqlite_schema", [], |r| {
            r.get::<_, i64>(0)
        })
        .map_err(|_| anyhow::anyhow!("Encrypted database authentication or format check failed"))?;
        conn.pragma_update(None, "query_only", true)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        Ok(Self {
            conn,
            cipher_version,
        })
    }
    pub fn capture_with_progress(
        &self,
        cache: &Path,
        key: &DatabaseKey,
        cancel: &CancellationToken,
        mut progress: impl FnMut(CaptureProgress),
    ) -> Result<DatabaseSnapshot> {
        cancel.check()?;
        std::fs::create_dir_all(cache)?;
        let owned = tempfile::Builder::new()
            .prefix("cuerithm-rb-")
            .tempdir_in(cache)?;
        let path = owned.path().join("master.db");
        {
            let mut dest = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            set_key(&dest, key)?;
            {
                let backup = Backup::new(&self.conn, &mut dest)
                    .context("Initialize coherent encrypted snapshot")?;
                let start = Instant::now();
                loop {
                    cancel.check()?;
                    ensure!(
                        start.elapsed() < Duration::from_secs(120),
                        "Encrypted snapshot timed out"
                    );
                    let step = backup.step(64).context("Copy encrypted snapshot pages")?;
                    let state = backup.progress();
                    progress(CaptureProgress {
                        remaining_pages: state.remaining,
                        total_pages: state.pagecount,
                    });
                    cancel.check()?;
                    match step {
                        StepResult::Done => break,
                        StepResult::More => {}
                        StepResult::Busy | StepResult::Locked => {
                            std::thread::sleep(Duration::from_millis(10))
                        }
                        _ => anyhow::bail!("Unsupported native snapshot result"),
                    }
                }
            }
            // Only the owned destination is changed. This makes the result a
            // standalone database, rather than a main file requiring an omitted WAL.
            dest.pragma_update(None, "journal_mode", "DELETE")?;
        }
        cancel.check()?;
        let snapshot = Self::open(&path, key)?;
        let checks = snapshot.checks()?;
        ensure!(
            checks.integrity_ok,
            "Encrypted snapshot integrity check failed"
        );
        let schema = snapshot.schema()?;
        drop(snapshot);
        let sha256 = file_sha256_cancellable(&path, cancel)?;
        std::fs::File::options()
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
    pub fn cipher_version(&self) -> &str {
        &self.cipher_version
    }
    /// Read support alone never authorizes writing this native collection.
    pub fn write_capability(&self) -> Result<WriteCapability> {
        validate_write_profile(&self.schema()?)
    }
    pub fn schema(&self) -> Result<DatabaseSchema> {
        let tx = self.conn.unchecked_transaction()?;
        let schema = schema_on(&tx)?;
        tx.commit()?;
        Ok(schema)
    }
    pub fn checks(&self) -> Result<DatabaseChecks> {
        let tx = self.conn.unchecked_transaction()?;
        let integrity_ok = {
            let mut q = tx.prepare("PRAGMA quick_check")?;
            let rows = q
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows == ["ok"]
        };
        let foreign_key_issues = {
            let mut q = tx.prepare("PRAGMA foreign_key_check")?;
            let mut rows = q.query([])?;
            let mut count = 0;
            while rows.next()?.is_some() {
                count += 1;
            }
            count
        };
        tx.commit()?;
        Ok(DatabaseChecks {
            integrity_ok,
            foreign_key_issues,
        })
    }
    pub fn capture(
        &self,
        cache: &Path,
        key: &DatabaseKey,
        cancel: &CancellationToken,
    ) -> Result<DatabaseSnapshot> {
        self.capture_with_progress(cache, key, cancel, |_| {})
    }
}

fn schema_on(conn: &Connection) -> Result<DatabaseSchema> {
    let objects = {
        let mut q=conn.prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type,name")?;
        q.query_map([], |r| {
            Ok(SchemaObject {
                kind: r.get(0)?,
                name: r.get(1)?,
                table: r.get(2)?,
                sql: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    let user_version = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let application_id = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&objects)?));
    Ok(DatabaseSchema {
        user_version,
        application_id,
        fingerprint,
        objects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_only_access_cannot_mutate_a_writable_encrypted_file() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("master.db");
        let key = DatabaseKey::new(b"generated native read boundary").unwrap();
        let conn = Connection::open(&path).unwrap();
        set_key(&conn, &key).unwrap();
        conn.execute_batch("CREATE TABLE authored(value TEXT);")
            .unwrap();
        drop(conn);
        let before = cuerithm_media::file_sha256(&path).unwrap();
        let reader = ReadOnlyDatabase::open(&path, &key).unwrap();
        assert!(
            reader
                .conn
                .execute_batch("INSERT INTO authored VALUES('must fail');")
                .is_err()
        );
        assert!(
            reader
                .conn
                .execute_batch("CREATE TABLE forbidden(value);")
                .is_err()
        );
        drop(reader);
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), before);
    }
}

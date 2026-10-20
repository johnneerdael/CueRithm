//! Publishing a verified write copy over a live `master.db`, with a verified backup first, a journal for
//! interrupted publications, and automatic restoration if the result does not verify.
//!
//! The copy is applied with SQLite's online backup API into a connection that holds the live database
//! exclusively, so the live file keeps its identity and journal mode and is never replaced by rename.
//! Nothing here deletes a WAL or SHM file.
use crate::{
    AssetWrite, DatabaseKey, ReadOnlyDatabase, WRITE_PROFILES, WriteDatabase, WriteProfile,
    schema_on, set_key,
};
use anyhow::{Context, Result, bail, ensure};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublishJournal {
    pub id: String,
    /// `prepared` until the live database verifies, then `committed`; `rolled_back` when the original
    /// was kept or restored.
    pub state: String,
    pub live: PathBuf,
    pub backup: PathBuf,
    pub backup_sha256: String,
    pub usn_before: i64,
    pub usn_after: i64,
    pub schema_fingerprint: String,
    /// Files this publication created beside the library, removed again if it is rolled back.
    #[serde(default)]
    pub assets: Vec<JournalAsset>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JournalAsset {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublishReport {
    pub journal: PathBuf,
    pub backup: PathBuf,
    pub usn_before: i64,
    pub usn_after: i64,
}
pub struct PublishRequest<'a> {
    pub live: &'a Path,
    /// A copy that a [`WriteDatabase`] wrote and that was based on a snapshot of `live`.
    pub work: &'a Path,
    pub key: &'a DatabaseKey,
    /// Owned folder for the backup and journal; one subfolder per publication.
    pub backup_dir: &'a Path,
    /// Files that belong with the change, such as analysis files, and the library's `share` folder they go under.
    /// An asset is only ever created: one that already exists stops the publication before anything changes.
    pub assets: &'a [AssetWrite],
    pub share_dir: Option<&'a Path>,
    /// `localUpdateCount` of the snapshot the work copy was made from. The live database must still be there.
    pub base_usn: i64,
}

fn usn_of(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT int_1 FROM agentRegistry WHERE registry_id='localUpdateCount'",
        [],
        |r| r.get::<_, Option<i64>>(0),
    )?
    .context("The database has no local update counter")
}
fn integrity_ok(conn: &Connection) -> Result<bool> {
    let mut q = conn.prepare("PRAGMA quick_check")?;
    let rows = q
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows == ["ok"])
}
/// Copy every page of `from` into `to` in one destination transaction.
fn copy_pages(
    from: &Connection,
    to: &mut Connection,
    cancel: Option<&CancellationToken>,
) -> Result<()> {
    let backup = Backup::new(from, to).context("Initialize encrypted page copy")?;
    let start = std::time::Instant::now();
    loop {
        if let Some(cancel) = cancel {
            cancel.check()?;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(300),
            "Encrypted page copy timed out"
        );
        match backup.step(64).context("Copy encrypted pages")? {
            StepResult::Done => return Ok(()),
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => std::thread::sleep(Duration::from_millis(10)),
            _ => bail!("Unsupported native copy result"),
        }
    }
}
fn open_plain(path: &Path, key: &DatabaseKey, create: bool) -> Result<Connection> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if create {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let conn = Connection::open_with_flags(path, flags)?;
    set_key(&conn, key)?;
    Ok(conn)
}
fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
/// Create every asset, each only if absent, and report what was created. Anything created before a failure
/// is removed again before the error is returned.
fn stage_assets(assets: &[AssetWrite], share: Option<&Path>) -> Result<Vec<JournalAsset>> {
    if assets.is_empty() {
        return Ok(vec![]);
    }
    let share = share.context("Assets need the library's share folder")?;
    let mut targets = vec![];
    for asset in assets {
        let relative = Path::new(&asset.relative_path);
        ensure!(
            !asset.relative_path.is_empty()
                && relative.is_relative()
                && relative
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "An asset path must stay inside the share folder"
        );
        let target = share.join(relative);
        ensure!(
            std::fs::symlink_metadata(&target).is_err(),
            "A file already exists where an analysis file would be written"
        );
        targets.push((target, asset));
    }
    let mut created: Vec<JournalAsset> = vec![];
    let result = (|| -> Result<()> {
        for (target, asset) in &targets {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)?;
            std::io::Write::write_all(&mut file, &asset.bytes)?;
            file.sync_all()?;
            created.push(JournalAsset {
                path: target.clone(),
                sha256: sha256_bytes(&asset.bytes),
            });
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(created),
        Err(error) => {
            remove_assets(&created);
            Err(error)
        }
    }
}
/// Remove files this publication created. A file whose bytes no longer match is left alone.
fn remove_assets(assets: &[JournalAsset]) {
    for asset in assets {
        if std::fs::read(&asset.path).is_ok_and(|bytes| sha256_bytes(&bytes) == asset.sha256) {
            let _ = std::fs::remove_file(&asset.path);
            // Only emptied folders go; remove_dir refuses one that still holds anything.
            let mut folder = asset.path.parent();
            for _ in 0..2 {
                if let Some(dir) = folder {
                    let _ = std::fs::remove_dir(dir);
                    folder = dir.parent();
                }
            }
        }
    }
}
fn save(journal: &PublishJournal, path: &Path) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, serde_json::to_vec_pretty(journal)?)?;
    std::fs::File::open(&temporary)?.sync_all()?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

/// Publish the work copy. On any failure after the live database was touched, the backup is restored.
pub fn publish_verified_copy(
    request: &PublishRequest<'_>,
    cancel: &CancellationToken,
) -> Result<PublishReport> {
    publish_with(request, cancel, &|_| Ok(()), WRITE_PROFILES)
}

/// As [`publish_verified_copy`], with an extra check run on the live database before it is accepted.
pub(crate) fn publish_with(
    request: &PublishRequest<'_>,
    cancel: &CancellationToken,
    extra_check: &dyn Fn(&Connection) -> Result<()>,
    profiles: &'static [WriteProfile],
) -> Result<PublishReport> {
    cancel.check()?;
    // 1. The work copy must itself verify, be a verified profile, and contain a write.
    let work = ReadOnlyDatabase::open(request.work, request.key)?;
    ensure!(
        work.checks()?.integrity_ok,
        "The write copy failed its integrity check"
    );
    let schema = work.schema()?;
    ensure!(
        profiles
            .iter()
            .any(|p| p.schema_fingerprint == schema.fingerprint),
        "The write copy is not a verified write profile"
    );
    let work_usn = usn_of(&work.conn)?;
    ensure!(
        work_usn > request.base_usn,
        "The write copy contains no changes to publish"
    );
    // 2. Hold the live database exclusively. A running rekordbox or agent makes this fail, not corrupt.
    let mut live = WriteDatabase::open_with_profiles(request.live, request.key, profiles)?;
    let live_usn = usn_of(live.connection())?;
    ensure!(
        live_usn == request.base_usn,
        "rekordbox changed the library after the copy was made; nothing was published"
    );
    ensure!(
        schema_on(live.connection())?.fingerprint == schema.fingerprint,
        "The live library's schema differs from the write copy"
    );
    cancel.check()?;
    // 3. A verified backup of exactly what is live now, and a journal that says what is about to happen.
    let id = uuid::Uuid::new_v4().to_string();
    let folder = request.backup_dir.join(&id);
    std::fs::create_dir_all(&folder)?;
    let backup = folder.join("master.backup.db");
    {
        let mut dest = open_plain(&backup, request.key, true)?;
        copy_pages(live.connection(), &mut dest, Some(cancel))?;
        dest.pragma_update(None, "journal_mode", "DELETE")?;
        ensure!(
            integrity_ok(&dest)?,
            "The backup failed its integrity check"
        );
        ensure!(
            usn_of(&dest)? == live_usn,
            "The backup does not match the live library"
        );
    }
    std::fs::File::options()
        .read(true)
        .write(true)
        .open(&backup)?
        .sync_all()?;
    let journal_path = folder.join("journal.json");
    let mut journal = PublishJournal {
        id,
        state: "prepared".into(),
        live: request.live.to_path_buf(),
        backup: backup.clone(),
        backup_sha256: file_sha256_cancellable(&backup, cancel)?,
        usn_before: live_usn,
        usn_after: work_usn,
        schema_fingerprint: schema.fingerprint.clone(),
        assets: vec![],
        error: None,
    };
    save(&journal, &journal_path)?;
    // Files first: if the database step then fails, restoring removes them, and a crash is settled the same way.
    match stage_assets(request.assets, request.share_dir) {
        Ok(created) => journal.assets = created,
        Err(error) => {
            journal.state = "rolled_back".into();
            journal.error = Some(format!("{error:#}"));
            save(&journal, &journal_path)?;
            return Err(error);
        }
    }
    save(&journal, &journal_path)?;
    // 4. Apply, verify, and restore on any failure. From here the cancellation token is not consulted.
    let outcome = (|| -> Result<()> {
        copy_pages(&work.conn, live.connection_mut(), None)?;
        let conn = live.connection();
        ensure!(
            integrity_ok(conn)?,
            "The published library failed its integrity check"
        );
        ensure!(
            usn_of(conn)? == work_usn,
            "The published library has the wrong change number"
        );
        ensure!(
            schema_on(conn)?.fingerprint == schema.fingerprint,
            "The published library's schema changed"
        );
        extra_check(conn)
    })();
    match outcome {
        Ok(()) => {
            journal.state = "committed".into();
            save(&journal, &journal_path)?;
            Ok(PublishReport {
                journal: journal_path,
                backup,
                usn_before: live_usn,
                usn_after: work_usn,
            })
        }
        Err(error) => {
            let restored = restore_from_backup(live.connection_mut(), &journal, request.key);
            remove_assets(&journal.assets);
            journal.state = "rolled_back".into();
            journal.error = Some(match &restored {
                Ok(()) => format!("{error:#}"),
                Err(restore) => format!("{error:#}; restoring the backup failed: {restore:#}"),
            });
            save(&journal, &journal_path)?;
            match restored {
                Ok(()) => Err(error.context("Publication failed and the original library was restored")),
                Err(restore) => Err(error.context(format!(
                    "Publication failed and restoring the backup also failed ({restore:#}); the backup is at {}",
                    journal.backup.display()
                ))),
            }
        }
    }
}

/// Put the journal's verified backup back over the live connection.
fn restore_from_backup(
    live: &mut Connection,
    journal: &PublishJournal,
    key: &DatabaseKey,
) -> Result<()> {
    ensure!(
        file_sha256_cancellable(&journal.backup, &CancellationToken::default())?
            == journal.backup_sha256,
        "The backup changed since it was made"
    );
    let backup = open_plain(&journal.backup, key, false)?;
    ensure!(
        integrity_ok(&backup)?,
        "The backup failed its integrity check"
    );
    copy_pages(&backup, live, None)?;
    ensure!(
        integrity_ok(live)?,
        "The restored library failed its integrity check"
    );
    ensure!(
        usn_of(live)? == journal.usn_before,
        "The restored library has the wrong change number"
    );
    Ok(())
}

/// Settle a publication that was interrupted. Returns the final state, `committed` or `rolled_back`.
pub fn recover_publication(journal_path: &Path, key: &DatabaseKey) -> Result<PublishJournal> {
    recover_with(journal_path, key, WRITE_PROFILES)
}
pub(crate) fn recover_with(
    journal_path: &Path,
    key: &DatabaseKey,
    profiles: &'static [WriteProfile],
) -> Result<PublishJournal> {
    let mut journal: PublishJournal = serde_json::from_slice(&std::fs::read(journal_path)?)?;
    if journal.state != "prepared" {
        return Ok(journal);
    }
    let mut live = WriteDatabase::open_with_profiles(&journal.live, key, profiles)?;
    let usn = usn_of(live.connection());
    let intact = integrity_ok(live.connection()).unwrap_or(false);
    match usn {
        // Never changed: the publication did not start.
        Ok(usn) if intact && usn == journal.usn_before => {
            remove_assets(&journal.assets);
            journal.state = "rolled_back".into();
            journal.error =
                Some("The publication was interrupted before it changed the library".into());
        }
        // Fully copied and intact: keep it. The schema must still be the one that was verified.
        Ok(usn)
            if intact
                && usn == journal.usn_after
                && schema_on(live.connection())?.fingerprint == journal.schema_fingerprint =>
        {
            journal.state = "committed".into();
        }
        // Anything else is a partial or foreign state: put the verified backup back.
        _ => {
            restore_from_backup(live.connection_mut(), &journal, key)?;
            remove_assets(&journal.assets);
            journal.state = "rolled_back".into();
            journal.error = Some(
                "The library was restored from its backup after an interrupted publication".into(),
            );
        }
    }
    save(&journal, journal_path)?;
    Ok(journal)
}

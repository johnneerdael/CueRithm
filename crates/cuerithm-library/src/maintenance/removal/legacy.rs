//! The copies of removed files that earlier versions kept in the data folder.
//!
//! Earlier versions deleted an unused file and kept a copy of it in the data folder, for a
//! Restore button that never expired. The library no longer keeps audio. For a removal that
//! can still be restored this copy is the only one left, so it is not deleted: it goes to the
//! Trash under the file's own name, where the user can still put it back. The copies of a
//! removal that was restored or rolled back duplicate a file that is in its place, so they are
//! simply deleted.
//!
//! Only files the application created are touched: a copy must be inside the removal folder of
//! the data folder, a regular file and not a link. A bounded number of removals is handled per
//! start; the rest follows at the next one.
use super::*;
use crate::legacy_originals::owned_regular_file;
use std::collections::BTreeMap;

/// Removals handled per start.
const OPERATIONS_PER_START: usize = 50;
const AREA: &str = "maintenance/removal";

/// The copies of one removal found on disk, by the hash of their content. A copy is told apart
/// by its bytes, not its name: an interrupted disposal from an earlier version renamed some.
fn copies(data: &Path, workspace: &Path) -> BTreeMap<String, Vec<PathBuf>> {
    let mut found: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let Ok(rows) = std::fs::read_dir(workspace) else {
        return found;
    };
    for row in rows.flatten().take(10_000) {
        let path = row.path();
        if owned_regular_file(data, AREA, &path).is_some()
            && let Ok(hash) = cuerithm_media::file_sha256(&path)
        {
            found.entry(hash).or_default().push(path);
        }
    }
    found
}

/// Moves one copy to the Trash under the file's own name. On a refusal the copy is back where
/// it was.
fn to_trash(
    copy: &Path,
    name: &std::ffi::OsStr,
    hash: &str,
    trash: &dyn FileTrash,
) -> Result<(), String> {
    let holder = copy
        .parent()
        .ok_or("Missing folder")?
        .join(format!("trash-{}", &hash[..16.min(hash.len())]));
    std::fs::create_dir_all(&holder).map_err(|e| e.to_string())?;
    let named = holder.join(name);
    crate::publish_files::create(copy, &named).map_err(|e| format!("{e:#}"))?;
    let moved = trash.move_to_trash(&named);
    if moved.is_err() {
        let _ = crate::publish_files::create(&named, copy);
    }
    let _ = std::fs::remove_dir(&holder);
    moved
}

/// Moves what earlier versions kept out of the data folder. Runs after the interrupted removals
/// have been finished.
pub(super) fn retire(lib: &Library, trash: &dyn FileTrash) -> Result<()> {
    retire_with(lib, trash, OPERATIONS_PER_START)
}
pub(super) fn retire_with(lib: &Library, trash: &dyn FileTrash, limit: usize) -> Result<()> {
    // Nothing creates or reads these any more; one left half-done would only block a library
    // backup for ever.
    lib.conn()?.execute(
        "DELETE FROM maintenance_scans WHERE kind IN ('unused_backup_disposal','unused_backup_disposal_plan')",
        [],
    )?;
    let ids: Vec<String> = {
        let conn = lib.conn()?;
        let mut q = conn.prepare(
            "SELECT id FROM maintenance_scans WHERE kind='unused_removal_operation'
               AND json_extract(payload_json,'$.schema') IN ('unused-removal-v1','unused-removal-v2')
               AND COALESCE(json_extract(payload_json,'$.backups_disposed'),0)=0
               AND json_extract(payload_json,'$.cleanup_complete')=1
               AND json_extract(payload_json,'$.state') IN ('committed','undone','rolled_back')
             ORDER BY created,rowid LIMIT ?1",
        )?;
        q.query_map([limit as i64], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    let data = lib.data_dir().canonicalize()?;
    let (mut files, mut bytes) = (0u64, 0u64);
    for id in ids {
        let Ok(mut op) = load_operation(lib, &id) else {
            continue;
        };
        let Some(workspace) = bytes_path(&op.workspace)
            .ok()
            .filter(|w| !w.as_os_str().is_empty())
        else {
            // Nothing was kept, or the folder was removed: there is nothing left to move.
            op.backups_disposed = true;
            save(&*lib.conn()?, &op)?;
            continue;
        };
        // The copy is the only one left when the removal still stands.
        let stands = op.state == "committed" && op.undone_by.is_none();
        let mut found = copies(&data, &workspace);
        let mut complete = true;
        for f in op.files.iter_mut() {
            if stands && f.disposal.is_some() {
                // Handled by an earlier start that could not finish the whole removal.
                continue;
            }
            let hash = f.entry.public.source_sha256.clone();
            let Some(copy) = found.get_mut(&hash).and_then(|paths| paths.pop()) else {
                // Already gone: a disposal from an earlier version, or a damaged folder.
                if stands && f.disposal.is_none() {
                    f.disposal = Some(UnusedRemovalDisposal::Gone);
                }
                continue;
            };
            let size = std::fs::metadata(&copy).map(|m| m.len()).unwrap_or(0);
            if stands && f.disposal.is_none() {
                let name = bytes_path(&f.entry.path)?
                    .file_name()
                    .map(|n| n.to_owned())
                    .unwrap_or_else(|| "removed-file".into());
                match to_trash(&copy, &name, &hash, trash) {
                    Ok(()) => f.disposal = Some(UnusedRemovalDisposal::Trashed),
                    Err(reason) => {
                        f.reason = Some(format!("Kept in the data folder: {reason}"));
                        complete = false;
                        continue;
                    }
                }
            } else if std::fs::remove_file(&copy).is_err() {
                complete = false;
                continue;
            }
            files += 1;
            bytes += size;
            f.backup = vec![];
        }
        if complete {
            // Whatever else is in the folder (a copy that no file of the removal matches) was
            // made by this application and is of no use to anyone.
            for rest in found.into_values().flatten() {
                if let Ok(()) = std::fs::remove_file(&rest) {
                    files += 1;
                }
            }
            let _ = std::fs::remove_dir(&workspace);
            op.backups_disposed = true;
            op.workspace = vec![];
            for f in &mut op.files {
                f.backup = vec![];
            }
        }
        if stands {
            op.result.outcomes = outcomes(&op);
        }
        save(&*lib.conn()?, &op)?;
    }
    lib.record_originals_removed(files, bytes)
}

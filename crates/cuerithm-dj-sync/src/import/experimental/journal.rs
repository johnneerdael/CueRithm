use crate::transaction::{durable_json, file_hash};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Entry {
    pub hash: Option<String>,
    pub identity: u64,
    pub directory: bool,
}
pub(super) type Inventory = BTreeMap<String, Entry>;
pub(super) fn identity(path: &Path) -> Result<u64> {
    let handle = same_file::Handle::from_path(path)?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    handle.hash(&mut hash);
    Ok(hash.finish())
}
pub(super) fn inventory(root: &Path) -> Result<Inventory> {
    fn walk(root: &Path, path: &Path, output: &mut Inventory) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.parent() == Some(root)
                && path.file_name() == Some(std::ffi::OsStr::new(".cuerithm-experiment"))
            {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "Fixture contains an unauthorized symbolic link"
            );
            ensure!(
                output.len() < 1000,
                "Fixture inventory exceeded generated scope"
            );
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .context("Non-UTF8 fixture path")?
                .replace('\\', "/");
            let directory = metadata.is_dir();
            ensure!(
                directory || metadata.is_file(),
                "Unsupported fixture object"
            );
            output.insert(
                relative,
                Entry {
                    hash: if directory { None } else { file_hash(&path)? },
                    identity: identity(&path)?,
                    directory,
                },
            );
            if directory {
                walk(root, &path, output)?;
            }
        }
        Ok(())
    }
    let mut output = Inventory::new();
    walk(root, root, &mut output)?;
    Ok(output)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) enum State {
    Prepared,
    Applying,
    Committed,
    Restoring,
    Restored,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Change {
    pub relative: String,
    pub before: String,
    pub after: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Plan {
    pub id: String,
    pub root_nonce: String,
    pub patch: super::dj_studio::ExistingRecordPatch,
    pub before: Inventory,
    pub changes: Vec<Change>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Journal {
    pub plan: Plan,
    pub state: State,
    pub replaced: usize,
}
pub(super) fn digest(value: &impl Serialize) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
pub(super) fn save(path: &Path, journal: &Journal) -> Result<()> {
    durable_json(path, journal)
}
pub(super) fn load(path: &Path) -> Result<Journal> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.len() < 4 * 1024 * 1024,
        "Invalid experiment journal"
    );
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
pub(super) fn sync(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
pub(super) fn safe(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    ensure!(
        !relative.is_absolute()
            && relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "Invalid fixture-relative publication"
    );
    let path = root.join(relative);
    for ancestor in path.ancestors() {
        if ancestor == root {
            break;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(m) => ensure!(
                !m.file_type().is_symlink(),
                "Fixture path gained a symbolic link"
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(path)
}
pub(super) fn publish(source: &Path, target: &Path, control: &Path) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(control)?;
    std::io::copy(&mut File::open(source)?, &mut temp)?;
    if target.exists() {
        temp.as_file()
            .set_permissions(fs::metadata(target)?.permissions())?;
    }
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist(target).map_err(|e| e.error)?;
    sync(target.parent().context("Missing fixture parent")?)
}
pub(super) fn validate_view(root: &Path, plan: &Plan) -> Result<Inventory> {
    let current = inventory(root)?;
    let changes = plan
        .changes
        .iter()
        .map(|c| (c.relative.as_str(), c))
        .collect::<BTreeMap<_, _>>();
    for (path, before) in &plan.before {
        if let Some(change) = changes.get(path.as_str()) {
            let now = current.get(path).and_then(|e| e.hash.as_deref());
            ensure!(
                now == Some(change.before.as_str()) || now == change.after.as_deref(),
                "Concurrent target modification blocks experiment recovery"
            );
        } else {
            ensure!(
                current.get(path) == Some(before),
                "Unowned fixture persistence changed"
            );
        }
    }
    for path in current.keys() {
        ensure!(
            plan.before.contains_key(path),
            "Unexpected fixture file appeared"
        );
    }
    Ok(current)
}

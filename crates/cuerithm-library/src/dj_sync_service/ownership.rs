use super::*;
use cuerithm_dj_sync::{
    PlaylistOwnershipContexts, PlaylistOwnershipReports,
    ownership::{PlaylistOwnership, PlaylistOwnershipRequest},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct OwnershipLedger {
    pub version: u32,
    pub generation: u64,
    pub last_journal: String,
    pub registration: String,
    pub stores: BTreeMap<PathBuf, (Option<PathBuf>, PlaylistOwnership)>,
    pub tombstones: Vec<Tombstone>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Tombstone {
    pub source_id: String,
    pub journal: String,
    pub deleted: bool,
}
fn path(root: &Path, target: &SyncTarget) -> Result<PathBuf> {
    Ok(root
        .join("ownership")
        .join(format!("{}.json", digest(target)?)))
}
pub(super) fn read(root: &Path, target: &SyncTarget) -> Result<OwnershipLedger> {
    match File::open(path(root, target)?) {
        Ok(file) => {
            let ledger: OwnershipLedger = serde_json::from_reader(file)?;
            ensure!(ledger.version == 1, "Unsupported playlist ownership ledger");
            Ok(ledger)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(OwnershipLedger::default()),
        Err(e) => Err(e.into()),
    }
}
pub(super) fn save(root: &Path, target: &SyncTarget, ledger: &OwnershipLedger) -> Result<()> {
    let path = path(root, target)?;
    fs::create_dir_all(path.parent().unwrap())?;
    atomic_json(&path, ledger)
}
/// Bind a registration to this filesystem object, never merely its displayed path.
/// Own atomic replacements receive a new registration after verified publication.
pub(super) fn registration(target: &SyncTarget) -> Result<String> {
    fn token(path: &Path) -> Result<String> {
        let meta = fs::symlink_metadata(path)?;
        ensure!(
            !meta.file_type().is_symlink(),
            "DJ registration cannot be a symbolic link"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            digest(&(path, meta.dev(), meta.ino(), meta.created().ok()))
        }
        #[cfg(not(unix))]
        {
            digest(&(
                path,
                meta.created()
                    .context("Filesystem cannot identify the selected DJ target")?,
            ))
        }
    }
    digest(&(
        token(&target.path)?,
        token(target.path.parent().context("DJ target parent missing")?)?,
        target.source_root.as_ref().map(|p| token(p)).transpose()?,
    ))
}
pub(super) fn check_registration(ledger: &OwnershipLedger, target: &SyncTarget) -> Result<()> {
    ensure!(
        ledger.registration.is_empty() || ledger.registration == registration(target)?,
        "The registered DJ database or device was replaced; reconcile playlist ownership before syncing"
    );
    Ok(())
}
pub(super) fn contexts(
    connection: &Connection,
    ledger: &OwnershipLedger,
    scope: &DjSyncScope,
) -> Result<PlaylistOwnershipContexts> {
    let mut removed = BTreeSet::new();
    if matches!(scope, DjSyncScope::Full | DjSyncScope::Changed) {
        for (id, node) in crate::playlists::load(connection)? {
            if node.node.deleted && node.node.deleted_by.is_some() {
                removed.insert(id);
            }
        }
    }
    Ok(ledger
        .stores
        .iter()
        .map(|(path, (root, state))| {
            let bound_removed = state
                .bindings
                .iter()
                .map(|b| &b.source_id)
                .filter(|id| removed.contains(*id))
                .cloned()
                .collect();
            (
                path.clone(),
                (
                    root.clone(),
                    PlaylistOwnershipRequest {
                        previous: state.clone(),
                        removed: bound_removed,
                    },
                ),
            )
        })
        .collect())
}
impl OwnershipLedger {
    pub fn contains(&self, id: &str) -> bool {
        self.stores
            .values()
            .any(|(_, s)| s.bindings.iter().any(|b| b.source_id == id))
    }
    pub fn next(&self, reports: PlaylistOwnershipReports, journal: &str) -> Self {
        let mut next = self.clone();
        next.version = 1;
        next.generation += 1;
        next.last_journal = journal.into();
        for (path, (root, report)) in reports {
            for id in &report.deleted {
                let event = Tombstone {
                    source_id: id.clone(),
                    journal: journal.into(),
                    deleted: true,
                };
                if !next.tombstones.contains(&event) {
                    next.tombstones.push(event);
                }
            }
            for id in &report.retired {
                let event = Tombstone {
                    source_id: id.clone(),
                    journal: journal.into(),
                    deleted: false,
                };
                if !next.tombstones.contains(&event) {
                    next.tombstones.push(event);
                }
            }
            next.stores.insert(path, (root, report.current));
        }
        next
    }
}
pub(super) fn verify_generation(
    root: &Path,
    target: &SyncTarget,
    before: &OwnershipLedger,
    after: Option<&OwnershipLedger>,
) -> Result<bool> {
    let current = read(root, target)?;
    let mut logical_current = current.clone();
    logical_current.registration.clear();
    let installed = if let Some(after) = after {
        let mut logical = after.clone();
        logical.registration.clear();
        digest(&logical_current)? == digest(&logical)?
    } else {
        false
    };
    ensure!(
        digest(&current)? == digest(before)? || installed,
        "Playlist ownership changed after preview; preview again"
    );
    Ok(installed)
}
pub(super) fn ack_key(id: &str) -> String {
    format!("dj_sync_ack:{id}")
}
pub(super) fn recovered_revision(connection: &Connection, id: &str) -> Result<Option<u64>> {
    Ok(connection
        .query_row("SELECT value FROM meta WHERE key=?1", [ack_key(id)], |r| {
            r.get(0)
        })
        .optional()?)
}
pub(super) fn ensure_settled(root: &Path, target: &SyncTarget) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type()?.is_dir() || uuid::Uuid::parse_str(&id).is_err() {
            continue;
        }
        let transaction = FileTransaction::open(root, &id)?;
        let state = transaction.inspect()?.state;
        if !matches!(
            state,
            JournalState::Applied
                | JournalState::Applying
                | JournalState::Restoring
                | JournalState::Restored
        ) {
            continue;
        }
        if let Ok(preview) = cuerithm_dj_sync::sync_preview(root, &id)
            && digest(&preview.target)? == digest(target)?
        {
            let settled = read_library_journal(&transaction).is_ok_and(|saved| match state {
                JournalState::Applied => saved.acknowledged,
                JournalState::Restored => saved.restore_acknowledged,
                _ => false,
            });
            ensure!(
                settled,
                "An interrupted sync for this target needs recovery from Sync history before a new preview"
            );
        }
    }
    Ok(())
}

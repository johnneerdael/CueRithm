use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RenameBefore {
    File { sha256: String },
    Directory { manifest: Vec<DirectoryEntry> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseRenameChange {
    #[serde(with = "crate::path_serde")]
    pub from: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub to: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub temporary: PathBuf,
    pub before: RenameBefore,
}
/// True only for one native directory entry addressed with a different leaf
/// spelling. Distinct hard links and ancestor-only spelling changes are excluded.
pub fn case_alias(from: &Path, to: &Path) -> Result<bool> {
    safe_ancestors(from)?;
    safe_ancestors(to)?;
    if from == to || from.file_name() == to.file_name() || !from.exists() || !to.exists() {
        return Ok(false);
    }
    // Inode identity alone cannot distinguish another hard link to this file.
    if from.file_name().unwrap().to_string_lossy().to_lowercase()
        != to.file_name().unwrap().to_string_lossy().to_lowercase()
    {
        return Ok(false);
    }
    if !same_file::is_same_file(from, to)?
        || !same_file::is_same_file(
            from.parent().context("Rename parent missing")?,
            to.parent().context("Rename parent missing")?,
        )?
    {
        return Ok(false);
    }
    let names = names(from.parent().unwrap())?;
    Ok(names.contains(from.file_name().unwrap()) && !names.contains(to.file_name().unwrap()))
}
/// Plan case-only aliases between original and final native namespace paths.
/// Targets are native publication paths, not paths to private staged copies.
/// All guards are captured and the complete rename plan is saved once.
pub fn plan_case_renames(
    transaction: &FileTransaction,
    sources: &[PathBuf],
    targets: &[PathBuf],
) -> Result<()> {
    use std::hash::{Hash, Hasher};
    fn identity(path: &Path) -> Result<Option<u64>> {
        safe_ancestors(path)?;
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() || meta.is_dir() => {
                let mut hash = std::collections::hash_map::DefaultHasher::new();
                same_file::Handle::from_path(path)?.hash(&mut hash);
                Ok(Some(hash.finish()))
            }
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    let _lease = transaction.lock()?;
    let mut journal = transaction.inspect()?;
    ensure!(
        journal.state == JournalState::Prepared,
        "Sync preview is no longer editable"
    );
    let mut groups: BTreeMap<u64, Vec<&Path>> = BTreeMap::new();
    for source in sources.iter().collect::<BTreeSet<_>>() {
        if let Some(id) = identity(source)? {
            groups.entry(id).or_default().push(source);
        }
    }
    let mut pairs = Vec::new();
    for target in targets.iter().collect::<BTreeSet<_>>() {
        let Some(candidates) = identity(target)?.and_then(|id| groups.get(&id)) else {
            continue;
        };
        let mut matched = None;
        for source in candidates {
            if case_alias(source, target)? {
                ensure!(matched.is_none(), "Ambiguous native case rename source");
                matched = Some(*source);
            }
        }
        if let Some(source) = matched {
            pairs.push((source, target.as_path()));
        }
    }
    pairs.sort_by_key(|(from, to)| (from.components().count(), *from, *to));
    let previous_len = journal.renames.len();
    for (from, to) in pairs {
        if journal
            .renames
            .iter()
            .any(|prior| prior.from == from && prior.to == to)
        {
            continue;
        }
        let before = if fs::symlink_metadata(from)?.is_dir() {
            RenameBefore::Directory {
                manifest: directory_manifest(from)?.context("Case rename directory disappeared")?,
            }
        } else {
            RenameBefore::File {
                sha256: file_hash(from)?.context("Case rename file disappeared")?,
            }
        };
        transaction.prepare_case_rename(&mut journal, from, to, before)?;
    }
    if journal.renames.len() != previous_len {
        transaction.save(&journal)?;
    }
    Ok(())
}
fn names(parent: &Path) -> Result<BTreeSet<std::ffi::OsString>> {
    Ok(fs::read_dir(parent)?
        .map(|e| e.map(|e| e.file_name()))
        .collect::<std::io::Result<_>>()?)
}
fn order(renames: &[CaseRenameChange]) -> Vec<usize> {
    let mut ids: Vec<_> = (0..renames.len()).collect();
    ids.sort_by_key(|id| (renames[*id].from.components().count(), *id));
    ids
}
fn prefix(path: &Path, pairs: &[(PathBuf, PathBuf)]) -> PathBuf {
    pairs
        .iter()
        .filter_map(|(from, to)| {
            path.strip_prefix(from).ok().map(|suffix| {
                (
                    from.components().count(),
                    if suffix.as_os_str().is_empty() {
                        to.clone()
                    } else {
                        to.join(suffix)
                    },
                )
            })
        })
        .max_by_key(|(depth, _)| *depth)
        .map_or_else(|| path.into(), |(_, path)| path)
}
/// Normalize both original and intermediate parent spellings into final paths.
/// This is lexical planning only; it does not traverse or change the filesystem.
pub fn translate_case_renames(path: &Path, renames: &[CaseRenameChange]) -> PathBuf {
    let mut pairs = Vec::new();
    for id in order(renames) {
        let rename = &renames[id];
        let from = prefix(&rename.from, &pairs);
        let to = prefix(&rename.to, &pairs);
        pairs.push((rename.from.clone(), to.clone()));
        pairs.push((from, to));
    }
    prefix(path, &pairs)
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    From,
    Temporary,
    To,
}
struct Location {
    index: usize,
    phase: Phase,
    path: PathBuf,
    final_path: PathBuf,
    directory: bool,
}
fn resolve(path: &Path, locations: &[Location]) -> PathBuf {
    locations
        .iter()
        .filter_map(|location| {
            if location.directory {
                path.strip_prefix(&location.final_path).ok().map(|suffix| {
                    (
                        location.final_path.components().count(),
                        location.path.join(suffix),
                    )
                })
            } else {
                (path == location.final_path).then(|| {
                    (
                        location.final_path.components().count(),
                        location.path.clone(),
                    )
                })
            }
        })
        .max_by_key(|(depth, _)| *depth)
        .map_or_else(|| path.into(), |(_, path)| path)
}
fn locations(journal: &FileJournal) -> Result<Vec<Location>> {
    let mut result = Vec::new();
    let mut cache = BTreeMap::new();
    for index in order(&journal.renames) {
        let rename = &journal.renames[index];
        let final_path = translate_case_renames(&rename.to, &journal.renames);
        let parent = resolve(
            final_path.parent().context("Rename parent missing")?,
            &result,
        );
        safe_ancestors(&parent)?;
        let entries = match cache.entry(parent.clone()) {
            std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::btree_map::Entry::Vacant(e) => e.insert(names(&parent)?),
        };
        let mut found = Vec::new();
        for (phase, path) in [
            (Phase::From, &rename.from),
            (Phase::Temporary, &rename.temporary),
            (Phase::To, &rename.to),
        ] {
            let name = path.file_name().context("Rename leaf name missing")?;
            if entries.contains(name) {
                found.push((phase, parent.join(name)));
            }
        }
        ensure!(
            found.len() == 1,
            "Case rename state is missing or ambiguous"
        );
        let (phase, path) = found.remove(0);
        safe_ancestors(&path)?;
        let metadata = fs::symlink_metadata(&path)?;
        let directory = matches!(rename.before, RenameBefore::Directory { .. });
        ensure!(
            if directory {
                metadata.is_dir()
            } else {
                metadata.is_file()
            },
            "Case rename object type changed"
        );
        result.push(Location {
            index,
            phase,
            path,
            final_path,
            directory,
        });
    }
    Ok(result)
}
pub(super) fn physical(path: &Path, journal: &FileJournal) -> Result<PathBuf> {
    if journal.renames.is_empty() {
        return Ok(path.into());
    }
    Ok(resolve(
        &translate_case_renames(path, &journal.renames),
        &locations(journal)?,
    ))
}
pub(super) fn file(journal: &FileJournal, path: &Path) -> Result<Option<String>> {
    {
        let path = physical(path, journal)?;
        safe_ancestors(&path)?;
        file_hash(&path)
    }
}
pub(super) fn directory(journal: &FileJournal, path: &Path) -> Result<Option<Vec<DirectoryEntry>>> {
    directory_manifest(&physical(path, journal)?)
}
pub(super) fn overlay_children(
    journal: &FileJournal,
    directory: &DirectoryChange,
    current: bool,
    entries: &mut BTreeMap<String, DirectoryEntryKind>,
) -> Result<()> {
    let parent = translate_case_renames(&directory.target, &journal.renames);
    let observed = if current {
        locations(journal)?
    } else {
        Vec::new()
    };
    for (index, rename) in journal.renames.iter().enumerate() {
        let from_parent = translate_case_renames(
            rename.from.parent().context("Rename parent missing")?,
            &journal.renames,
        );
        if from_parent != parent {
            continue;
        }
        for path in [&rename.from, &rename.to, &rename.temporary] {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .context("Non-UTF-8 case rename name")?;
            entries.remove(name);
        }
        let name = if current {
            observed
                .iter()
                .find(|r| r.index == index)
                .context("Missing case rename observation")?
                .path
                .file_name()
                .context("Rename leaf missing")?
        } else {
            rename.to.file_name().context("Rename leaf missing")?
        };
        entries.insert(
            name.to_str().context("Non-UTF-8 case rename name")?.into(),
            if matches!(rename.before, RenameBefore::Directory { .. }) {
                DirectoryEntryKind::Directory
            } else {
                DirectoryEntryKind::File
            },
        );
    }
    Ok(())
}
pub(super) fn validate_plan(journal: &FileJournal) -> Result<()> {
    let mut destinations = BTreeSet::new();
    for rename in &journal.renames {
        safe_ancestors(&rename.from)?;
        safe_ancestors(&rename.to)?;
        safe_ancestors(&rename.temporary)?;
        ensure!(
            rename.from.file_name() != rename.to.file_name(),
            "Case rename must change the leaf spelling"
        );
        let destination = translate_case_renames(&rename.to, &journal.renames);
        ensure!(
            destinations.insert(destination.clone()),
            "Duplicate case rename destination"
        );
        match &rename.before {
            RenameBefore::File { .. } => ensure!(
                !journal.changes.iter().any(|c| translate_case_renames(
                    &c.target,
                    &journal.renames
                ) == destination
                    && c.after_sha256.is_none()),
                "A renamed file cannot also be deleted"
            ),
            RenameBefore::Directory { manifest } => {
                valid_manifest(manifest)?;
                ensure!(
                    !journal.directories.iter().any(|c| translate_case_renames(
                        &c.target,
                        &journal.renames
                    ) == destination
                        && !c.after_exists),
                    "A renamed directory cannot also be removed"
                );
            }
        }
    }
    Ok(())
}
pub(super) fn preflight(journal: &FileJournal, restoring: bool) -> Result<()> {
    validate_plan(journal)?;
    for location in locations(journal)? {
        let rename = &journal.renames[location.index];
        if !restoring {
            ensure!(
                location.phase == Phase::From && case_alias(&rename.from, &rename.to)?,
                "Case rename preview is stale"
            );
        }
        match &rename.before {
            RenameBefore::File { sha256 } => {
                let current = file_hash(&location.path)?;
                let mut allowed = current.as_ref() == Some(sha256);
                if restoring {
                    allowed |= journal.changes.iter().any(|c| {
                        translate_case_renames(&c.target, &journal.renames) == location.final_path
                            && (current == c.before_sha256 || current == c.after_sha256)
                    });
                }
                ensure!(allowed, "Case rename file changed after preview");
            }
            RenameBefore::Directory { manifest } => {
                let expected = if restoring {
                    child_manifest(
                        journal,
                        &DirectoryChange {
                            target: rename.to.clone(),
                            before_manifest: Some(manifest.clone()),
                            after_exists: true,
                        },
                        true,
                    )?
                } else {
                    manifest.clone()
                };
                ensure!(
                    directory_manifest(&location.path)? == Some(expected),
                    "Case rename directory changed after preview"
                );
            }
        }
    }
    Ok(())
}
fn move_to(journal: &FileJournal, index: usize, target: Phase) -> Result<()> {
    let locations = locations(journal)?;
    let location = locations
        .iter()
        .find(|r| r.index == index)
        .context("Rename state missing")?;
    let rename = &journal.renames[index];
    let parent = location.path.parent().context("Rename parent missing")?;
    let logical = match target {
        Phase::From => &rename.from,
        Phase::Temporary => &rename.temporary,
        Phase::To => &rename.to,
    };
    let destination = parent.join(logical.file_name().context("Rename name missing")?);
    safe_ancestors(&location.path)?;
    safe_ancestors(&destination)?;
    ensure!(
        matches!(fs::symlink_metadata(&destination),Err(ref e) if e.kind()==std::io::ErrorKind::NotFound),
        "Case rename destination appeared externally"
    );
    fs::rename(&location.path, &destination)?;
    sync_directory(parent)?;
    let observed = locations_for(journal, index)?;
    ensure!(
        observed == target,
        "Case rename spelling verification failed"
    );
    Ok(())
}
fn locations_for(journal: &FileJournal, index: usize) -> Result<Phase> {
    Ok(locations(journal)?
        .into_iter()
        .find(|r| r.index == index)
        .context("Rename observation missing")?
        .phase)
}
pub(super) fn apply(journal: &FileJournal, check: &impl Fn() -> Result<()>) -> Result<()> {
    for index in order(&journal.renames) {
        check()?;
        ensure!(
            locations_for(journal, index)? == Phase::From,
            "Case rename changed during apply"
        );
        move_to(journal, index, Phase::Temporary)?;
        check()?;
        move_to(journal, index, Phase::To)?;
    }
    Ok(())
}
pub(super) fn restore(journal: &FileJournal, check: &impl Fn() -> Result<()>) -> Result<()> {
    for index in order(&journal.renames).into_iter().rev() {
        check()?;
        match locations_for(journal, index)? {
            Phase::From => continue,
            Phase::Temporary => {}
            Phase::To => move_to(journal, index, Phase::Temporary)?,
        }
        check()?;
        move_to(journal, index, Phase::From)?;
    }
    Ok(())
}
pub(super) fn verify(journal: &FileJournal, restored: bool) -> Result<()> {
    let expected = if restored { Phase::From } else { Phase::To };
    for location in locations(journal)? {
        ensure!(
            location.phase == expected,
            "Case rename spelling changed after publication"
        );
        let rename = &journal.renames[location.index];
        if let RenameBefore::Directory { manifest } = &rename.before {
            let expected = if restored {
                manifest.clone()
            } else {
                child_manifest(
                    journal,
                    &DirectoryChange {
                        target: rename.to.clone(),
                        before_manifest: Some(manifest.clone()),
                        after_exists: true,
                    },
                    false,
                )?
            };
            ensure!(
                directory_manifest(&location.path)? == Some(expected),
                "Case-renamed directory changed after publication"
            );
        }
    }
    preflight(journal, true)
}

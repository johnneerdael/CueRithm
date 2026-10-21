use super::*;
use crate::ownership::{
    PlaylistBinding, PlaylistOwnership, PlaylistOwnershipReport, PlaylistOwnershipRequest,
    fingerprint,
};

const IDENTITY: &str = "virtualdj-8.5-mylists";
const NAMESPACE: &str = "MyLists/CueRithm.subfolders";
fn relative(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
            && (path.starts_with(NAMESPACE)
                || path.starts_with("Playlists/CueRithm")
                || (path.parent() == Some(Path::new("Folders/Filters"))
                    && path
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("CueRithm-")))),
        "Invalid VirtualDJ ownership path"
    );
    Ok(path.into())
}
fn node_fingerprint(home: &Path, path: &Path) -> Result<Option<String>> {
    let path = home.join(relative(path)?);
    let meta = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(
        !meta.file_type().is_symlink(),
        "VirtualDJ playlist is a symbolic link"
    );
    if meta.is_file() {
        return crate::transaction::file_hash(&path);
    }
    ensure!(meta.is_dir(), "Invalid VirtualDJ playlist object");
    fingerprint(&(
        crate::transaction::directory_manifest(&path)?,
        crate::transaction::file_hash(&path.join("order"))?,
    ))
    .map(Some)
}
fn order_files(
    home: &Path,
    directory: &Path,
    result: &mut BTreeMap<PathBuf, Vec<String>>,
) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        ensure!(
            !entry.file_type()?.is_symlink(),
            "VirtualDJ list tree contains a symbolic link"
        );
        if entry.file_type()?.is_dir() {
            order_files(home, &entry.path(), result)?;
        } else if entry.file_name() == "order" {
            result.insert(
                directory.strip_prefix(home)?.into(),
                fs::read_to_string(entry.path())?
                    .lines()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            );
        }
    }
    Ok(())
}
fn order_name(home: &Path, path: &Path) -> String {
    if home.join("MyLists").is_dir() {
        path.file_stem()
    } else {
        path.file_name()
    }
    .unwrap()
    .to_string_lossy()
    .into_owned()
}
fn merge_order(previous: Vec<String>, selected: Vec<String>) -> Vec<String> {
    let selected_set: BTreeSet<_> = selected.iter().cloned().collect();
    let mut pending = selected.into_iter();
    let mut result = Vec::new();
    for old in previous {
        if selected_set.contains(&old) {
            if let Some(next) = pending.next() {
                result.push(next);
            }
        } else if !result.contains(&old) {
            result.push(old);
        }
    }
    result.extend(pending);
    result
}
/// Reconcile on a staged home only. The publication journal owns all real paths.
pub fn write_owned(
    path: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
    request: &PlaylistOwnershipRequest,
) -> Result<PlaylistOwnershipReport> {
    write_owned_partitioned(path, library, &library.tracks, options, request)
}
pub fn write_owned_partitioned(
    path: &Path,
    library: &SyncLibrary,
    database_tracks: &[crate::SyncTrack],
    options: &SyncOptions,
    request: &PlaylistOwnershipRequest,
) -> Result<PlaylistOwnershipReport> {
    let home = if path.is_dir() {
        path.to_owned()
    } else {
        path.parent().context("VirtualDJ home missing")?.to_owned()
    };
    let identity = if home.join("MyLists").is_dir() {
        IDENTITY
    } else {
        "virtualdj-8.5-legacy"
    };
    ensure!(
        request.previous.identity.is_empty() || request.previous.identity == identity,
        "VirtualDJ library identity changed"
    );
    let database = if path.is_dir() {
        path.join("database.xml")
    } else {
        path.into()
    };
    let home = database.parent().context("VirtualDJ home missing")?;
    let previous: BTreeMap<_, _> = request
        .previous
        .bindings
        .iter()
        .map(|b| (b.source_id.clone(), b.clone()))
        .collect();
    ensure!(
        previous.len() == request.previous.bindings.len(),
        "Duplicate VirtualDJ source bindings"
    );
    let mut old_paths = BTreeMap::new();
    for binding in previous.values() {
        let path = relative(Path::new(&binding.locator))?;
        ensure!(
            old_paths
                .insert(path.clone(), binding.source_id.clone())
                .is_none(),
            "VirtualDJ playlist has multiple source bindings"
        );
        if (!request.removed.contains(&binding.source_id) || binding.created)
            && let Some(current) = node_fingerprint(home, &path)?
        {
            ensure!(
                current == binding.fingerprint,
                "VirtualDJ playlist was edited outside CueRithm; reconcile before syncing"
            );
        }
    }
    let files = playlist_files(home, library, options)?;
    let mut destinations: BTreeMap<String, PathBuf> = playlist_paths(home, library, options)?
        .into_iter()
        .map(|(id, path)| Ok((id, path.strip_prefix(home)?.to_owned())))
        .collect::<Result<_>>()?;
    let folder_ids: BTreeSet<_> = library
        .playlists
        .iter()
        .filter(|p| p.folder)
        .map(|p| p.id.clone())
        .chain(
            previous
                .values()
                .filter(|b| home.join(&b.locator).is_dir())
                .map(|b| b.source_id.clone()),
        )
        .collect();
    let folder_moves: Vec<_> = library
        .playlists
        .iter()
        .filter(|p| p.folder)
        .filter_map(|p| {
            previous
                .get(&p.id)
                .map(|b| (PathBuf::from(&b.locator), destinations[&p.id].clone()))
        })
        .filter(|(a, b)| a != b)
        .collect();
    let translated = |path: &Path| -> PathBuf {
        folder_moves
            .iter()
            .filter(|(old, _)| path.starts_with(old))
            .max_by_key(|(old, _)| old.components().count())
            .map_or_else(
                || path.into(),
                |(old, new)| new.join(path.strip_prefix(old).unwrap()),
            )
    };
    for binding in previous.values() {
        if !request.removed.contains(&binding.source_id) {
            destinations
                .entry(binding.source_id.clone())
                .or_insert_with(|| translated(Path::new(&binding.locator)));
        }
    }
    let mut unique = BTreeSet::new();
    for destination in destinations.values() {
        ensure!(
            unique.insert(destination.to_string_lossy().to_lowercase()),
            "VirtualDJ playlist destination collision"
        );
    }
    let mut deleted = BTreeSet::new();
    let mut retired = BTreeSet::new();
    for source in &request.removed {
        if let Some(binding) = previous.get(source) {
            if binding.created {
                deleted.insert(source.clone());
            } else {
                retired.insert(source.clone());
            }
        }
    }
    let affected: BTreeSet<_> = previous
        .values()
        .filter(|b| {
            deleted.contains(&b.source_id)
                || destinations
                    .get(&b.source_id)
                    .is_some_and(|d| d != Path::new(&b.locator))
        })
        .map(|b| PathBuf::from(&b.locator))
        .collect();
    // No recursive move/deletion: each child must have its own binding and plan.
    for old in &affected {
        let directory = home.join(old);
        if directory.is_dir() {
            for entry in fs::read_dir(&directory)? {
                let entry = entry?;
                let rel = entry.path().strip_prefix(home)?.to_path_buf();
                ensure!(
                    entry.file_name() == "order"
                        || (old_paths.contains_key(&rel) && affected.contains(&rel)),
                    "VirtualDJ folder contains an unrelated or surviving child"
                );
            }
        }
    }
    let mut moved_bytes = BTreeMap::new();
    for old in &affected {
        let source = home.join(old);
        if source.is_file() {
            moved_bytes.insert(old.clone(), fs::read(source)?);
        }
    }
    let mut originals = BTreeMap::new();
    let namespace = if home.join("MyLists").is_dir() {
        NAMESPACE
    } else {
        "Playlists/CueRithm"
    };
    order_files(home, &home.join(namespace), &mut originals)?;
    let mut transformed_orders = BTreeMap::<PathBuf, Vec<String>>::new();
    for (dir, entries) in originals {
        if deleted.iter().any(|id| {
            previous
                .get(id)
                .is_some_and(|b| dir.starts_with(&b.locator))
        }) {
            continue;
        }
        let new_dir = translated(&dir);
        let target = transformed_orders.entry(new_dir.clone()).or_default();
        for entry in entries {
            let owned = old_paths
                .iter()
                .find(|(p, _)| p.parent() == Some(dir.as_path()) && order_name(home, p) == entry);
            if let Some((_, id)) = owned {
                if retired.contains(id) {
                    target.push(entry);
                    continue;
                }
                if let Some(new) = destinations.get(id)
                    && new.parent() == Some(new_dir.as_path())
                {
                    target.push(order_name(home, new));
                }
            } else {
                target.push(entry);
            }
        }
    }
    // Vacate every affected old path first so case-only renames and swaps work.
    let mut removal: Vec<_> = affected.iter().cloned().collect();
    removal.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for old in &removal {
        let path = home.join(old);
        if path.is_dir() {
            let order = path.join("order");
            if order.is_file() {
                fs::remove_file(order)?;
            }
            fs::remove_dir(path)?;
        } else if path.exists() {
            fs::remove_file(path)?;
        }
    }
    let mut created = BTreeSet::new();
    for node in &library.playlists {
        ensure!(
            !request.removed.contains(&node.id),
            "VirtualDJ playlist cannot be active and removed"
        );
        let destination = &destinations[&node.id];
        let prior = previous.get(&node.id);
        if prior.is_none() && node_fingerprint(home, destination)?.is_none() {
            created.insert(node.id.clone());
        }
        if let Some(prior) = prior {
            if affected.contains(Path::new(&prior.locator)) {
                ensure!(
                    !home.join(destination).exists(),
                    "VirtualDJ move collides with an existing native playlist"
                );
            } else if node_fingerprint(home, Path::new(&prior.locator))?.is_none() {
                ensure!(
                    !home.join(destination).exists(),
                    "Missing VirtualDJ binding collides with an existing native playlist"
                );
                created.insert(node.id.clone());
            }
        }
    }
    for (old, bytes) in moved_bytes {
        let id = &old_paths[&old];
        if let Some(destination) = destinations.get(id) {
            let path = home.join(destination);
            ensure!(
                !path.exists(),
                "VirtualDJ relocation collides with another file"
            );
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, bytes)?;
        }
    }
    for (id, destination) in &destinations {
        if folder_ids.contains(id) && !retired.contains(id) {
            fs::create_dir_all(home.join(destination))?;
        }
    }
    super::write_partitioned(&database, library, database_tracks, options)?;
    for (file, bytes) in files {
        if file.file_name().is_some_and(|s| s == "order") {
            let dir = file.parent().unwrap().strip_prefix(home)?.to_path_buf();
            let selected = String::from_utf8(bytes)?
                .lines()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            let old = transformed_orders.remove(&dir).unwrap_or_default();
            transformed_orders.insert(dir, merge_order(old, selected));
        }
    }
    for (dir, order) in transformed_orders {
        let path = home.join(&dir);
        if path.is_dir() {
            let bytes = if order.is_empty() {
                String::new()
            } else {
                format!("{}\r\n", order.join("\r\n"))
            };
            fs::write(path.join("order"), bytes)?;
        }
    }
    let mut bindings = previous;
    for id in deleted.iter().chain(&retired) {
        bindings.remove(id);
    }
    for node in &library.playlists {
        let old = bindings.get(&node.id);
        bindings.insert(
            node.id.clone(),
            PlaylistBinding {
                source_id: node.id.clone(),
                locator: destinations[&node.id].to_string_lossy().into_owned(),
                parent_source_id: node.parent_id.clone(),
                created: created.contains(&node.id) || old.is_some_and(|b| b.created),
                fingerprint: String::new(),
            },
        );
    }
    for (id, binding) in &mut bindings {
        if let Some(path) = destinations.get(id) {
            binding.locator = path.to_string_lossy().into_owned();
        }
        binding.fingerprint = node_fingerprint(home, Path::new(&binding.locator))?
            .context("VirtualDJ bound playlist disappeared after staging")?;
    }
    Ok(PlaylistOwnershipReport {
        current: PlaylistOwnership {
            identity: identity.into(),
            bindings: bindings.into_values().collect(),
        },
        deleted,
        retired,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("database.xml");
        fs::create_dir(path.parent().unwrap().join("MyLists")).unwrap();
        fs::write(&path, b"<VirtualDJ_Database Version=\"8.5\"/>").unwrap();
        (dir, path)
    }
    fn library(name: &str) -> SyncLibrary {
        SyncLibrary {
            tracks: vec![],
            playlists: vec![
                crate::SyncPlaylist {
                    id: "folder".into(),
                    name: name.into(),
                    folder: true,
                    ..Default::default()
                },
                crate::SyncPlaylist {
                    id: "leaf".into(),
                    parent_id: Some("folder".into()),
                    name: "Leaf".into(),
                    ..Default::default()
                },
                crate::SyncPlaylist {
                    id: "empty".into(),
                    name: "Empty".into(),
                    folder: true,
                    ..Default::default()
                },
            ],
        }
    }
    #[test]
    fn windows_context_virtualdj_legacy_playlists_preserve_capability_and_owned_removal() {
        let (_dir, path) = fixture();
        let home = path.parent().unwrap();
        fs::remove_dir(home.join("MyLists")).unwrap();
        fs::create_dir(home.join("Playlists")).unwrap();
        fs::write(home.join("Playlists/foreign.m3u"), b"foreign").unwrap();
        let mut input = library("Sets");
        input.tracks.push(crate::SyncTrack {
            native_path: None,
            id: "one".into(),
            path: PathBuf::from(r"D:\Music\one.wav"),
            ..Default::default()
        });
        input.playlists[1].track_ids.push("one".into());
        let first = write_owned(&path, &input, &Default::default(), &Default::default()).unwrap();
        assert!(!home.join("MyLists").exists());
        let leaf = first
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == "leaf")
            .unwrap();
        assert!(Path::new(&leaf.locator).starts_with("Playlists/CueRithm"));
        assert!(leaf.locator.ends_with(".m3u"));
        assert_eq!(
            fs::read_to_string(home.join(&leaf.locator)).unwrap(),
            "#EXTVDJ:\nD:\\Music\\one.wav\n"
        );
        input.playlists.retain(|p| p.id != "leaf");
        let removed = write_owned(
            &path,
            &input,
            &Default::default(),
            &PlaylistOwnershipRequest {
                previous: first.current.clone(),
                removed: BTreeSet::from(["leaf".into()]),
            },
        )
        .unwrap();
        assert!(removed.deleted.contains("leaf"));
        assert!(!home.join(&leaf.locator).exists());
        assert_eq!(
            fs::read(home.join("Playlists/foreign.m3u")).unwrap(),
            b"foreign"
        );
    }
    #[test]
    fn legacy_native_filter_materialization_moves_only_its_owned_representation() {
        use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
        let (_dir, path) = fixture();
        let home = path.parent().unwrap();
        fs::remove_dir(home.join("MyLists")).unwrap();
        fs::create_dir_all(home.join("Folders/Filters")).unwrap();
        let foreign = home.join("Folders/Filters/foreign.vdjfolder");
        fs::write(&foreign, b"native foreign filter").unwrap();
        let mut input = library("Sets");
        input.playlists[1].smart = Some(SmartDefinition {
            match_all: true,
            rules: vec![SmartRule {
                id: "rule".into(),
                field: "artist".into(),
                operator: Operator::StringContains,
                value: RuleValue::Text("Authored".into()),
                or: false,
            }],
        });
        let native = write_owned(&path, &input, &Default::default(), &Default::default()).unwrap();
        let filter = native
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == "leaf")
            .unwrap();
        assert_eq!(
            Path::new(&filter.locator).parent(),
            Some(Path::new("Folders/Filters"))
        );
        assert_eq!(
            parse(&fs::read(home.join(&filter.locator)).unwrap())
                .unwrap()
                .name,
            "FilterFolder"
        );
        let materialized = write_owned(
            &path,
            &input,
            &SyncOptions {
                materialize_smartlists: true,
                ..Default::default()
            },
            &PlaylistOwnershipRequest {
                previous: native.current.clone(),
                removed: Default::default(),
            },
        )
        .unwrap();
        assert!(!home.join(&filter.locator).exists());
        let list = materialized
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == "leaf")
            .unwrap();
        assert!(list.locator.ends_with(".m3u"));
        assert_eq!(fs::read(foreign).unwrap(), b"native foreign filter");
        assert!(!home.join("MyLists").exists());
    }
    #[test]
    fn dotted_names_keep_owned_hierarchy_distinct_paths_and_order() {
        let (_dir, path) = fixture();
        let home = path.parent().unwrap();
        let mut data = library("Folder.2026");
        data.playlists[1].name = "Mix.1".into();
        data.playlists.push(crate::SyncPlaylist {
            id: "second".into(),
            name: "Mix.2".into(),
            parent_id: Some("folder".into()),
            ..Default::default()
        });
        let report = write_owned(&path, &data, &Default::default(), &Default::default()).unwrap();
        let locator = |id: &str| {
            PathBuf::from(
                &report
                    .current
                    .bindings
                    .iter()
                    .find(|b| b.source_id == id)
                    .unwrap()
                    .locator,
            )
        };
        let folder = locator("folder");
        let first = locator("leaf");
        let second = locator("second");
        assert_eq!(first.parent(), Some(folder.as_path()));
        assert_eq!(second.parent(), Some(folder.as_path()));
        assert_ne!(first, second);
        assert!(
            folder
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Folder.2026-")
        );
        assert!(
            first
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Mix.1-")
        );
        assert!(
            second
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Mix.2-")
        );
        assert_eq!(
            fs::read_to_string(home.join(&folder).join("order")).unwrap(),
            format!(
                "{}\r\n{}\r\n",
                first.file_stem().unwrap().to_str().unwrap(),
                second.file_stem().unwrap().to_str().unwrap()
            )
        );
        for binding in &report.current.bindings {
            assert!(home.join(&binding.locator).exists());
        }
        let mut renamed = data;
        renamed.playlists[0].name = "Folder.2027".into();
        let moved = write_owned(
            &path,
            &renamed,
            &Default::default(),
            &PlaylistOwnershipRequest {
                previous: report.current,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!home.join(folder).exists());
        for binding in &moved.current.bindings {
            assert!(home.join(&binding.locator).exists());
        }
        let published = home.join("published");
        fs::create_dir(&published).unwrap();
        let original_database = b"<VirtualDJ_Database Version=\"8.5\"/>";
        fs::write(published.join("database.xml"), original_database).unwrap();
        let mut staged: Vec<_> = playlist_files(home, &renamed, &Default::default())
            .unwrap()
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        staged.push(path.clone());
        let mut directories = BTreeSet::new();
        let files: Vec<_> = staged
            .into_iter()
            .map(|staged| {
                let target = published.join(staged.strip_prefix(home).unwrap());
                let mut parent = target.parent().unwrap();
                while parent != published {
                    directories.insert(parent.to_path_buf());
                    parent = parent.parent().unwrap();
                }
                crate::transaction::FilePublication {
                    before_sha256: crate::transaction::file_hash(&target).unwrap(),
                    target,
                    staged: Some(staged),
                }
            })
            .collect();
        let directories: Vec<_> = directories
            .into_iter()
            .map(|target| crate::transaction::DirectoryChange {
                target,
                before_manifest: None,
                after_exists: true,
            })
            .collect();
        let transaction =
            crate::transaction::FileTransaction::create(&home.join("journals")).unwrap();
        transaction.add_batch(&files, &directories).unwrap();
        transaction.apply(|| Ok(())).unwrap();
        for binding in &moved.current.bindings {
            assert!(published.join(&binding.locator).exists());
        }
        transaction.restore(|| Ok(())).unwrap();
        assert_eq!(
            fs::read(published.join("database.xml")).unwrap(),
            original_database
        );
        assert!(!published.join("MyLists").exists());
    }
    #[test]
    fn renamed_subtree_and_empty_folder_keep_proof_then_delete_without_tracks() {
        let (_dir, path) = fixture();
        let home = path.parent().unwrap();
        let first = write_owned(
            &path,
            &library("Folder"),
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        assert_eq!(first.current.bindings.len(), 3);
        assert!(first.current.bindings.iter().all(|b| b.created));
        let old = first
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == "folder")
            .unwrap()
            .locator
            .clone();
        let second = write_owned(
            &path,
            &library("Renamed"),
            &Default::default(),
            &PlaylistOwnershipRequest {
                previous: first.current,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!home.join(old).exists());
        assert!(second.current.bindings.iter().all(|b| b.created));
        for binding in &second.current.bindings {
            assert!(home.join(&binding.locator).exists());
        }
        let final_report = write_owned(
            &path,
            &SyncLibrary::default(),
            &Default::default(),
            &PlaylistOwnershipRequest {
                previous: second.current,
                removed: BTreeSet::from(["folder".into(), "leaf".into(), "empty".into()]),
            },
        )
        .unwrap();
        assert_eq!(final_report.deleted.len(), 3);
        assert!(final_report.current.bindings.is_empty());
        assert_eq!(
            fs::read_to_string(home.join(NAMESPACE).join("order")).unwrap(),
            ""
        );
    }
    #[test]
    fn external_match_is_retained_and_native_edits_refuse_owned_changes() {
        let (_dir, path) = fixture();
        let home = path.parent().unwrap();
        let data = library("Folder");
        super::super::write(&path, &data, &Default::default()).unwrap();
        let report = write_owned(&path, &data, &Default::default(), &Default::default()).unwrap();
        assert!(report.current.bindings.iter().all(|b| !b.created));
        let leaf = report
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == "leaf")
            .unwrap();
        let order = home.join(&leaf.locator).parent().unwrap().join("order");
        let before_order = fs::read(&order).unwrap();
        let retired = write_owned(
            &path,
            &SyncLibrary::default(),
            &Default::default(),
            &PlaylistOwnershipRequest {
                previous: report.current,
                removed: BTreeSet::from(["leaf".into()]),
            },
        )
        .unwrap();
        assert!(retired.retired.contains("leaf"));
        assert_eq!(before_order, fs::read(&order).unwrap());
        let first = write_owned(
            &path,
            &SyncLibrary {
                tracks: vec![],
                playlists: vec![crate::SyncPlaylist {
                    id: "new".into(),
                    name: "Owned".into(),
                    folder: true,
                    ..Default::default()
                }],
            },
            &Default::default(),
            &Default::default(),
        )
        .unwrap();
        let locator = first.current.bindings[0].locator.clone();
        fs::write(home.join(&locator).join("foreign.txt"), b"retain").unwrap();
        let result = write_owned(
            &path,
            &SyncLibrary::default(),
            &Default::default(),
            &PlaylistOwnershipRequest {
                previous: first.current,
                removed: BTreeSet::from(["new".into()]),
            },
        );
        assert!(result.unwrap_err().to_string().contains("edited outside"));
        assert!(home.join(&locator).join("foreign.txt").exists());
    }
}

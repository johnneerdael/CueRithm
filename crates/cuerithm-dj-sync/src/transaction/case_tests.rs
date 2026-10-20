use super::*;
fn exact(path: &Path) -> bool {
    path.parent().is_some_and(|p| {
        fs::read_dir(p).is_ok_and(|mut entries| {
            entries.any(|e| e.is_ok_and(|e| Some(e.file_name().as_os_str()) == path.file_name()))
        })
    })
}
#[test]
fn case_planner_preserves_distinct_hardlinks_and_noop_journal() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let a = root.join("a.vdjfolder");
    let b = root.join("b.vdjfolder");
    fs::write(&a, b"same inode").unwrap();
    fs::hard_link(&a, &b).unwrap();
    let alias = root.join("A.vdjfolder");
    if !alias.exists() {
        return;
    }
    assert!(!case_alias(&b, &alias).unwrap());
    let tx = FileTransaction::create(&root.join("journals")).unwrap();
    let before = fs::read(tx.root.join("journal.json")).unwrap();
    plan_case_renames(&tx, std::slice::from_ref(&b), std::slice::from_ref(&alias)).unwrap();
    assert_eq!(fs::read(tx.root.join("journal.json")).unwrap(), before);
    plan_case_renames(&tx, &[b, a.clone()], std::slice::from_ref(&alias)).unwrap();
    assert_eq!(tx.inspect().unwrap().renames.len(), 1);
    plan_case_renames(&tx, &[a], &[alias]).unwrap();
    assert_eq!(tx.inspect().unwrap().renames.len(), 1);
    tx.apply(|| Ok(())).unwrap();
    assert!(exact(&root.join("b.vdjfolder")));
    tx.restore(|| Ok(())).unwrap();
    assert!(exact(&root.join("a.vdjfolder")));
}
fn nested(root: &Path) -> Option<(FileTransaction, PathBuf, PathBuf)> {
    let old = root.join("folder.subfolders");
    let new = root.join("Folder.subfolders");
    fs::create_dir(&old).unwrap();
    fs::create_dir(old.join("child.subfolders")).unwrap();
    fs::write(old.join("child.subfolders/set.vdjfolder"), b"before").unwrap();
    fs::write(old.join("child.subfolders/unrelated"), b"keep").unwrap();
    if !new.is_dir() {
        eprintln!("Case-sensitive host: native alias test not applicable");
        return None;
    }
    let child = new.join("Child.subfolders");
    let leaf = child.join("Set.vdjfolder");
    let tx = FileTransaction::create(&root.join("journals")).unwrap();
    plan_case_renames(
        &tx,
        &[
            old.join("child.subfolders/set.vdjfolder"),
            old.join("child.subfolders/unrelated"),
            old.join("child.subfolders"),
            old.clone(),
        ],
        &[
            leaf.clone(),
            child.join("unrelated"),
            child.clone(),
            new.clone(),
        ],
    )
    .unwrap();
    assert_eq!(tx.inspect().unwrap().renames.len(), 3);
    let staged = root.join("new-content");
    fs::write(&staged, b"after").unwrap();
    tx.add_batch(
        &[FilePublication {
            target: leaf,
            before_sha256: Some(
                file_hash(&old.join("child.subfolders/set.vdjfolder"))
                    .unwrap()
                    .unwrap(),
            ),
            staged: Some(staged),
        }],
        &[
            DirectoryChange {
                target: root.into(),
                before_manifest: directory_manifest(root).unwrap(),
                after_exists: true,
            },
            DirectoryChange {
                target: new.clone(),
                before_manifest: directory_manifest(&new).unwrap(),
                after_exists: true,
            },
            DirectoryChange {
                target: child.clone(),
                before_manifest: directory_manifest(&child).unwrap(),
                after_exists: true,
            },
        ],
    )
    .unwrap();
    Some((tx, old, new))
}
fn original(old: &Path, new: &Path) {
    assert!(exact(old));
    assert!(!exact(new));
    assert!(exact(&old.join("child.subfolders")));
    assert!(!exact(&old.join("Child.subfolders")));
    assert!(exact(&old.join("child.subfolders/set.vdjfolder")));
    assert_eq!(
        fs::read(old.join("child.subfolders/set.vdjfolder")).unwrap(),
        b"before"
    );
    assert_eq!(
        fs::read(old.join("child.subfolders/unrelated")).unwrap(),
        b"keep"
    );
}
#[test]
fn nested_case_renames_update_parent_manifests_and_preserve_unrelated_children() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let Some((tx, old, new)) = nested(&root) else {
        return;
    };
    tx.apply(|| Ok(())).unwrap();
    assert!(exact(&new));
    assert!(!exact(&old));
    assert!(exact(&new.join("Child.subfolders/Set.vdjfolder")));
    assert_eq!(
        fs::read(new.join("Child.subfolders/Set.vdjfolder")).unwrap(),
        b"after"
    );
    assert_eq!(
        fs::read(new.join("Child.subfolders/unrelated")).unwrap(),
        b"keep"
    );
    tx.verify_applied().unwrap();
    tx.restore(|| Ok(())).unwrap();
    original(&old, &new);
    tx.verify_restored().unwrap();
}
#[test]
fn every_case_apply_phase_is_recoverable_from_persisted_temporary_names() {
    for cut in 0..9 {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let Some((tx, old, new)) = nested(&root) else {
            return;
        };
        let id = tx.inspect().unwrap().id;
        let calls = std::cell::Cell::new(0);
        let result = tx.apply(|| {
            let n = calls.get();
            calls.set(n + 1);
            assert!(
                observed_file_hash(&tx.inspect()?, &new.join("Child.subfolders/unrelated"))?
                    .is_some()
            );
            ensure!(n != cut, "Injected apply phase interruption");
            Ok(())
        });
        if result.is_err() {
            if tx.inspect().unwrap().state != JournalState::Prepared {
                FileTransaction::open(&root.join("journals"), &id)
                    .unwrap()
                    .restore(|| Ok(()))
                    .unwrap();
            }
        } else {
            tx.restore(|| Ok(())).unwrap();
        }
        original(&old, &new);
    }
}
#[test]
fn every_case_restore_phase_is_recoverable_and_verifies_exact_before_spelling() {
    for cut in 0..9 {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let Some((tx, old, new)) = nested(&root) else {
            return;
        };
        let id = tx.inspect().unwrap().id;
        tx.apply(|| Ok(())).unwrap();
        let calls = std::cell::Cell::new(0);
        if tx
            .restore(|| {
                let n = calls.get();
                calls.set(n + 1);
                assert!(
                    observed_file_hash(&tx.inspect()?, &new.join("Child.subfolders/unrelated"))?
                        .is_some()
                );
                ensure!(n != cut, "Injected restore phase interruption");
                Ok(())
            })
            .is_err()
        {
            FileTransaction::open(&root.join("journals"), &id)
                .unwrap()
                .restore(|| Ok(()))
                .unwrap();
        }
        original(&old, &new);
        tx.verify_restored().unwrap();
    }
}
#[test]
fn foreign_destination_between_case_rename_phases_is_never_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let from = root.join("set");
    let to = root.join("Set");
    fs::write(&from, b"owned").unwrap();
    if !to.exists() {
        return;
    }
    let tx = FileTransaction::create(&root.join("journals")).unwrap();
    tx.add_case_rename(
        &from,
        &to,
        RenameBefore::File {
            sha256: file_hash(&from).unwrap().unwrap(),
        },
    )
    .unwrap();
    let temporary = tx.inspect().unwrap().renames[0].temporary.clone();
    assert!(
        tx.apply(|| {
            if temporary.exists() && !to.exists() {
                fs::write(&to, b"foreign")?;
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(fs::read(&to).unwrap(), b"foreign");
    assert_eq!(fs::read(&temporary).unwrap(), b"owned");
    assert!(tx.restore(|| Ok(())).is_err());
    fs::remove_file(&to).unwrap();
    tx.restore(|| Ok(())).unwrap();
    assert_eq!(fs::read(from).unwrap(), b"owned");
}
#[test]
fn restored_verifier_rejects_foreign_file_or_directory_changes_without_mutating_them() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let file = root.join("database");
    let staged = root.join("new");
    let folder = root.join("folder");
    fs::create_dir(&folder).unwrap();
    fs::write(&file, b"before").unwrap();
    fs::write(&staged, b"after").unwrap();
    let tx = FileTransaction::create(&root.join("journals")).unwrap();
    tx.add_batch(
        &[FilePublication {
            target: file.clone(),
            before_sha256: file_hash(&file).unwrap(),
            staged: Some(staged),
        }],
        &[DirectoryChange {
            target: folder.clone(),
            before_manifest: Some(vec![]),
            after_exists: false,
        }],
    )
    .unwrap();
    tx.apply(|| Ok(())).unwrap();
    tx.restore(|| Ok(())).unwrap();
    tx.verify_restored().unwrap();
    fs::write(&file, b"foreign").unwrap();
    assert!(tx.verify_restored().is_err());
    assert_eq!(fs::read(&file).unwrap(), b"foreign");
    fs::write(&file, b"before").unwrap();
    fs::write(folder.join("foreign"), b"keep").unwrap();
    assert!(tx.verify_restored().is_err());
    assert!(folder.join("foreign").is_file());
    assert_eq!(tx.inspect().unwrap().state, JournalState::Restored);
}
#[test]
fn failed_batch_retains_the_old_journal_and_its_staged_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let staged = root.join("new");
    fs::write(&staged, b"bytes").unwrap();
    let tx = FileTransaction::create(&root.join("journals")).unwrap();
    tx.add(&root.join("existing-plan"), None, Some(&staged))
        .unwrap();
    let before = fs::read(tx.path().join("journal.json")).unwrap();
    assert!(
        tx.add_batch(
            &[
                FilePublication {
                    target: root.join("first"),
                    before_sha256: None,
                    staged: Some(staged)
                },
                FilePublication {
                    target: root.join("second"),
                    before_sha256: None,
                    staged: Some(root.join("missing-stage"))
                }
            ],
            &[DirectoryChange {
                target: root.join("new-folder"),
                before_manifest: None,
                after_exists: true
            }]
        )
        .is_err()
    );
    assert_eq!(fs::read(tx.path().join("journal.json")).unwrap(), before);
    assert_eq!(fs::read(tx.path().join("staged/0")).unwrap(), b"bytes");
    assert_eq!(fs::read_dir(tx.path().join("staged")).unwrap().count(), 1);
    assert!(!root.join("first").exists());
    assert!(!root.join("new-folder").exists());
}
#[test]
fn translation_accepts_original_and_intermediate_parent_spellings() {
    let rename = |from: &str, to: &str| CaseRenameChange {
        from: from.into(),
        to: to.into(),
        temporary: "/native/private".into(),
        before: RenameBefore::Directory { manifest: vec![] },
    };
    let renames = vec![
        rename("/native/folder", "/native/Folder"),
        rename("/native/folder/child", "/native/Folder/Child"),
    ];
    assert_eq!(
        translate_case_renames(Path::new("/native/folder/child/one"), &renames),
        PathBuf::from("/native/Folder/Child/one")
    );
    assert_eq!(
        translate_case_renames(Path::new("/native/Folder/child/two"), &renames),
        PathBuf::from("/native/Folder/Child/two")
    );
    assert_eq!(
        translate_case_renames(Path::new("/native/folder/childish"), &renames),
        PathBuf::from("/native/Folder/childish")
    );
    // Path equality ignores a trailing separator, but native file APIs reject it.
    let file = CaseRenameChange {
        from: "/native/intro.vdjfolder".into(),
        to: "/native/Intro.vdjfolder".into(),
        temporary: "/native/private".into(),
        before: RenameBefore::File {
            sha256: "synthetic".into(),
        },
    };
    assert_eq!(
        translate_case_renames(&file.from, std::slice::from_ref(&file)).as_os_str(),
        file.to.as_os_str()
    );
}
#[test]
fn uncollapsed_case_alias_directory_changes_are_rejected_before_removal() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let from = root.join("folder");
    let to = root.join("Folder");
    fs::create_dir(&from).unwrap();
    if !to.exists() {
        return;
    }
    let tx = FileTransaction::create(&root.join("journals")).unwrap();
    tx.add_directory(&from, Some(vec![]), false).unwrap();
    tx.add_directory(&to, Some(vec![]), true).unwrap();
    assert!(tx.apply(|| Ok(())).is_err());
    assert!(exact(&from));
    assert_eq!(tx.inspect().unwrap().state, JournalState::Prepared);
}

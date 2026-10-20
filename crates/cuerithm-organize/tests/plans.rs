use cuerithm_organize::{
    paths::{NativePath, PathRules},
    plan::*,
    template::{MissingPolicy, NamingContext},
};
fn native(path: &str) -> std::path::PathBuf {
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    std::path::PathBuf::from(root).join(path.trim_start_matches('/'))
}
fn file(path: &str, id: &str) -> FileSnapshot {
    FileSnapshot {
        path: NativePath(native(path)),
        identity: id.into(),
        sha256: "a".repeat(64),
        byte_len: 20,
        path_revision: 1,
    }
}
fn track(id: &str, title: &str) -> TrackInput {
    TrackInput {
        id: id.into(),
        source: file(&format!("/incoming/{id}.flac"), id),
        metadata_revision: 2,
        metadata: NamingContext {
            title: Some(title.into()),
            ..Default::default()
        },
        sidecars: vec![],
        references: vec![],
    }
}
fn request(tracks: Vec<TrackInput>) -> PlanInput {
    PlanInput {
        config: PlanConfig {
            mode: OrganizationMode::Move,
            pattern: "{title}".into(),
            missing: MissingPolicy::Block,
            organization_optional: false,
            rules: PathRules::Portable,
            managed_root: Some(RootSnapshot {
                path: NativePath(native("/managed")),
                identity: "root-1".into(),
            }),
        },
        tracks,
        existing: vec![],
        non_files: vec![],
    }
}
#[test]
fn freezes_complete_scope_and_invalidates_changed_inputs() {
    let req = request(vec![track("1", "Title")]);
    let plan = build_plan(req.clone(), || false).unwrap();
    assert!(plan.can_apply());
    assert_eq!(
        plan.rows[0].files[0].destination.0,
        native("/managed/Title.flac")
    );
    assert!(plan.matches_inputs(&req));
    let mut changed = req;
    changed.tracks[0].metadata_revision += 1;
    assert!(!plan.matches_inputs(&changed));
}
#[test]
fn collision_blocks_all_selected_rows_and_unselected_files() {
    let mut req = request(vec![track("1", "Café"), track("2", "CAFE\u{301}")]);
    req.existing.push(file("/managed/Café.flac", "other"));
    let plan = build_plan(req, || false).unwrap();
    assert!(
        plan.rows
            .iter()
            .all(|r| matches!(r.outcome, RowOutcome::Blocked { .. }))
    );
    assert!(!plan.can_apply());
}
#[test]
fn sidecar_collisions_block_both_tracks() {
    let mut a = track("1", "One");
    let mut b = track("2", "Two");
    a.sidecars.push(Sidecar {
        source: file("/incoming/a.jpg", "art1"),
        destination_name: "cover.jpg".into(),
    });
    b.sidecars.push(Sidecar {
        source: file("/incoming/b.jpg", "art2"),
        destination_name: "cover.jpg".into(),
    });
    assert!(
        build_plan(request(vec![a, b]), || false)
            .unwrap()
            .rows
            .iter()
            .all(|r| matches!(r.outcome, RowOutcome::Blocked { .. }))
    );
}
#[test]
fn identical_existing_copy_requires_content_proof() {
    let mut req = request(vec![track("1", "Title")]);
    req.config.mode = OrganizationMode::Copy;
    req.existing.push(file("/managed/Title.flac", "copy"));
    assert_eq!(
        build_plan(req.clone(), || false).unwrap().rows[0].outcome,
        RowOutcome::IdenticalCopy
    );
    req.existing[0].sha256 = "b".repeat(64);
    assert!(matches!(
        build_plan(req, || false).unwrap().rows[0].outcome,
        RowOutcome::Blocked { .. }
    ));
}
#[test]
fn cancellation_never_returns_partial_plan() {
    assert_eq!(
        build_plan(request(vec![track("1", "Title")]), || true),
        Err(PlanError::Cancelled)
    );
}
#[test]
fn hundred_thousand_rows_have_stable_paging() {
    let req = request(
        (0..100_000)
            .map(|i| track(&format!("{i:06}"), &format!("Title {i}")))
            .collect(),
    );
    let plan = build_plan(req, || false).unwrap();
    assert_eq!(plan.rows.len(), 100_000);
    assert_eq!(plan.page(99_990, 20).len(), 10);
    assert_eq!(plan.page(100_001, 10).len(), 0);
    assert_eq!(plan.page(0, 2000).len(), 1000);
    assert_eq!(plan.page(1, 1)[0].track.id, "000001");
}

#[test]
fn selected_source_reserves_its_existing_path_even_if_omitted_from_inventory() {
    let mut req = request(vec![track("1", "2"), track("2", "Elsewhere")]);
    req.config.managed_root.as_mut().unwrap().path = NativePath(native("/incoming"));
    let plan = build_plan(req, || false).unwrap();
    assert!(matches!(plan.rows[0].outcome, RowOutcome::Blocked { .. }));
}
#[test]
fn raw_native_path_survives_freezing_and_json() {
    #[cfg(unix)]
    let raw = {
        use std::os::unix::ffi::OsStringExt;
        std::path::PathBuf::from(std::ffi::OsString::from_vec(b"/raw-\xff/old.flac".to_vec()))
    };
    #[cfg(windows)]
    let raw = {
        use std::os::windows::ffi::OsStringExt;
        std::path::PathBuf::from(std::ffi::OsString::from_wide(&[
            67, 58, 92, 0xd800, 92, 111, 108, 100, 46, 102, 108, 97, 99,
        ]))
    };
    let mut req = request(vec![track("1", "New")]);
    req.tracks[0].source.path = NativePath(raw.clone());
    req.config.mode = OrganizationMode::RenameInPlace;
    let plan = build_plan(req, || false).unwrap();
    assert_eq!(plan.rows[0].outcome, RowOutcome::Ready);
    assert_eq!(plan.rows[0].files[0].destination.0.parent(), raw.parent());
    let restored: CompletedPlan =
        serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
    assert_eq!(plan, restored);
}
#[test]
fn optional_skip_cannot_hide_invalid_final_paths() {
    let mut req = request(vec![track("1", "CON")]);
    req.config.missing = MissingPolicy::SkipOptional;
    req.config.organization_optional = true;
    assert!(matches!(
        build_plan(req, || false).unwrap().rows[0].outcome,
        RowOutcome::Blocked { .. }
    ));
}
#[test]
fn cancellation_during_scope_never_returns_a_token() {
    let mut n = 0;
    let result = build_plan(
        request(
            (0..100)
                .map(|i| track(&i.to_string(), &format!("Title{i}")))
                .collect(),
        ),
        || {
            n += 1;
            n == 40
        },
    );
    assert_eq!(result, Err(PlanError::Cancelled));
}

#[test]
fn symlink_ancestors_and_directory_targets_block_publication() {
    let mut req = request(vec![track("1", "Album/Title")]);
    req.config.pattern = "Album/{title}".into();
    req.non_files.push(NonFileEntry {
        path: NativePath(native("/managed/Album")),
        kind: NonFileKind::Symlink,
    });
    assert!(matches!(
        build_plan(req.clone(), || false).unwrap().rows[0].outcome,
        RowOutcome::Blocked { .. }
    ));
    req.non_files[0].kind = NonFileKind::Directory;
    assert_eq!(
        build_plan(req.clone(), || false).unwrap().rows[0].outcome,
        RowOutcome::Ready
    );
    req.non_files.push(NonFileEntry {
        path: NativePath(native("/managed/Album/Album_Title.flac")),
        kind: NonFileKind::Directory,
    });
    assert!(matches!(
        build_plan(req, || false).unwrap().rows[0].outcome,
        RowOutcome::Blocked { .. }
    ));
}

#[test]
fn nested_patterns_preserve_logical_separators_when_appending_the_extension() {
    let mut req = request(vec![track("1", "Title.v2")]);
    req.config.pattern = "Artist/Album/{title}".into();
    let plan = build_plan(req, || false).unwrap();
    assert_eq!(plan.rows[0].outcome, RowOutcome::Ready);
    assert_eq!(
        plan.rows[0].files[0].destination.0,
        native("/managed/Artist/Album/Title.v2.flac")
    );
}

#[test]
fn totals_distinguish_blocked_skipped_and_ready() {
    let mut req = request(vec![track("1", "CON"), track("2", "Good"), track("3", "")]);
    req.config.missing = MissingPolicy::SkipOptional;
    req.config.organization_optional = true;
    let plan = build_plan(req, || false).unwrap();
    assert_eq!(
        plan.totals(),
        PlanTotals {
            total: 3,
            ready: 1,
            blocked: 1,
            skipped: 1,
            ..Default::default()
        }
    );
}

#[test]
fn selected_source_parent_directories_cannot_become_file_destinations() {
    let mut a = track("1", "Parent");
    let mut b = track("2", "New");
    a.source.path = NativePath(native("/incoming/old.flac"));
    b.source.path = NativePath(native("/incoming/Parent.flac/child.flac"));
    let mut req = request(vec![a, b]);
    req.config.mode = OrganizationMode::RenameInPlace;
    assert!(matches!(
        build_plan(req, || false).unwrap().rows[0].outcome,
        RowOutcome::Blocked { .. }
    ));
}

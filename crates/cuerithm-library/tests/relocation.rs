use cuerithm_library::{
    AcceptedMusicalData, Library, PlaylistEdit, PlaylistKind, RelocationChoice, RelocationPlan,
    RelocationRequest,
};
use cuerithm_media::{CancellationToken, MediaRuntime, file_sha256_cancellable};
use std::path::{Path, PathBuf};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    db: PathBuf,
    library: Library,
    runtime: MediaRuntime,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let db = root.join("library.sqlite");
        let library = Library::open(&db).unwrap();
        let media = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin");
        let runtime = MediaRuntime::from_bundle(&media)
            .expect("Bundled media runtime required for relocation acceptance");
        Self {
            _temp: temp,
            root,
            db,
            library,
            runtime,
        }
    }
    fn audio(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../cuerithm-media/tests/fixtures/covered.flac"),
            &path,
        )
        .unwrap();
        path
    }
    fn add(&self, path: &Path) -> String {
        let jobs = self
            .library
            .enqueue(
                &[path.to_owned()],
                self.library.settings().unwrap().revision,
            )
            .unwrap();
        let job = self
            .library
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == jobs[0])
            .unwrap();
        let run = self
            .library
            .claim_next()
            .unwrap()
            .expect("Fixture job must be claimable");
        assert_eq!(run.id, job.id);
        self.library
            .fail_for_run(
                &run,
                "test",
                "Controlled fixture; no preparation job may run",
            )
            .unwrap();
        job.track_id
    }
    fn search(&self, ids: &[String], root: &Path) -> RelocationPlan {
        self.library
            .plan_relocation(
                RelocationRequest {
                    mode: "selection".into(),
                    track_ids: ids.to_vec(),
                    search_root: Some(root.to_owned()),
                    ..Default::default()
                },
                &self.runtime,
                &CancellationToken::default(),
                |_| {},
            )
            .unwrap()
    }
    fn apply(
        &self,
        plan: &RelocationPlan,
        row: usize,
        candidate: usize,
        survivor: &str,
    ) -> anyhow::Result<cuerithm_library::RelocationResult> {
        let row = &plan.rows[row];
        self.library.apply_relocation(
            &plan.id,
            &[RelocationChoice {
                track_id: row.track_id.clone(),
                candidate_id: row.candidates[candidate].id.clone(),
                survivor: survivor.into(),
                survivor_track_id: None,
            }],
            &self.runtime,
            &CancellationToken::default(),
            None,
        )
    }
}
fn hash(path: &Path) -> String {
    file_sha256_cancellable(path, &CancellationToken::default()).unwrap()
}
fn selected(plan: &RelocationPlan) -> Vec<RelocationChoice> {
    plan.rows
        .iter()
        .filter_map(|r| {
            r.candidates.first().map(|c| RelocationChoice {
                track_id: r.track_id.clone(),
                candidate_id: c.id.clone(),
                survivor: "source".into(),
                survivor_track_id: None,
            })
        })
        .collect()
}

#[test]
fn component_root_mapping_does_not_match_a_sibling_prefix() {
    use cuerithm_library::relocation_root_suffix;
    assert_eq!(
        relocation_root_suffix("D:/Music/House/A.mp3", "D:/Music").unwrap(),
        Some(vec!["House".into(), "A.mp3".into()])
    );
    assert_eq!(
        relocation_root_suffix("D:/Music2/A.mp3", "D:/Music").unwrap(),
        None
    );
    assert_eq!(
        relocation_root_suffix("/home/music/A.mp3", "/music").unwrap(),
        None
    );
}

#[test]
fn moving_audio_preserves_identity_and_undo_after_restart() {
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let id = f.add(&old);
    let new = f.root.join("new/A.flac");
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::rename(&old, &new).unwrap();
    let digest = hash(&new);
    let modified = std::fs::metadata(&new).unwrap().modified().unwrap();
    let plan = f.search(std::slice::from_ref(&id), new.parent().unwrap());
    assert!(plan.rows[0].candidates[0].preselected);
    let result = f.apply(&plan, 0, 0, "source").unwrap();
    assert_eq!(result.applied, 1);
    assert_eq!(f.library.track_path(&id).unwrap(), new);
    assert_eq!(f.library.track(&id).unwrap().id, id);
    assert_eq!(hash(&new), digest);
    assert_eq!(
        std::fs::metadata(&new).unwrap().modified().unwrap(),
        modified
    );
    let db = f.db.clone();
    drop(f.library);
    let reopened = Library::open(&db).unwrap();
    reopened
        .undo_relocation(&result.operation_id, &CancellationToken::default())
        .unwrap();
    assert_eq!(reopened.track_path(&id).unwrap(), old);
    assert_eq!(hash(&new), digest);
    assert!(!old.exists());
}

#[test]
fn pn_suffix_and_parent_tier_win_without_renaming_audio() {
    let f = Fixture::new();
    let old = f.audio("old/House/A_pn.flac");
    let id = f.add(&old);
    std::fs::remove_file(&old).unwrap();
    let best = f.audio("new/House/A.flac");
    f.audio("new/Other/A.flac");
    let plan = f.search(&[id], &f.root.join("new"));
    assert_eq!(plan.rows[0].candidates.len(), 1);
    assert_eq!(plan.rows[0].candidates[0].path, best);
    assert!(plan.rows[0].candidates[0].preselected);
    f.apply(&plan, 0, 0, "source").unwrap();
    assert!(best.exists());
    assert!(!best.with_file_name("A_pn.flac").exists());
}

#[test]
fn duplicate_highest_tier_candidates_stay_ambiguous() {
    let f = Fixture::new();
    let old = f.audio("old/House/A.flac");
    let id = f.add(&old);
    std::fs::remove_file(&old).unwrap();
    f.audio("new/x/House/A.flac");
    f.audio("new/y/House/A.flac");
    let plan = f.search(&[id], &f.root.join("new"));
    assert_eq!(plan.rows[0].candidates.len(), 2);
    assert!(plan.rows[0].candidates.iter().all(|c| !c.preselected));
}

#[test]
fn case_fallback_and_changed_encoded_hash_require_explicit_replacement() {
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let id = f.add(&old);
    std::fs::remove_file(&old).unwrap();
    let different_case = f.audio("new/a.flac");
    let plan = f.search(std::slice::from_ref(&id), &f.root.join("new"));
    assert!(!plan.rows[0].candidates[0].preselected);
    assert!(plan.rows[0].candidates[0].replacement);
    std::fs::rename(&different_case, f.root.join("new/A.flac")).unwrap();
    let target = f.root.join("new/A.flac");
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&target)
        .unwrap()
        .write_all(b"changed encoded bytes")
        .unwrap();
    let plan = f.search(std::slice::from_ref(&id), &f.root.join("new"));
    assert!(plan.rows[0].candidates[0].replacement);
    assert!(!plan.rows[0].candidates[0].preselected);
    f.apply(&plan, 0, 0, "source").unwrap();
    assert_eq!(f.library.track_path(&id).unwrap(), target);
}

#[test]
fn available_selection_is_skipped_but_explicit_all_root_mapping_is_reviewed() {
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let id = f.add(&old);
    let target = f.audio("new/A.flac");
    let selected = f.search(std::slice::from_ref(&id), &f.root.join("new"));
    assert!(selected.rows.is_empty());
    assert_eq!(selected.skipped, 1);
    let plan = f
        .library
        .plan_relocation(
            RelocationRequest {
                mode: "root".into(),
                source_root: Some(f.root.join("old").to_string_lossy().into()),
                destination_root: Some(f.root.join("new")),
                all_tracks: true,
                ..Default::default()
            },
            &f.runtime,
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap();
    assert_eq!(plan.rows.len(), 1);
    f.apply(&plan, 0, 0, "source").unwrap();
    assert_eq!(f.library.track_path(&id).unwrap(), target);
    assert!(old.exists());
}

#[test]
fn stale_file_and_accepted_cue_revision_reject_the_entire_selected_set() {
    let f = Fixture::new();
    let a = f.audio("old/A.flac");
    let b = f.audio("old/B.flac");
    let ids = vec![f.add(&a), f.add(&b)];
    std::fs::remove_file(&a).unwrap();
    std::fs::remove_file(&b).unwrap();
    f.audio("new/A.flac");
    let target = f.audio("new/B.flac");
    let plan = f.search(&ids, &f.root.join("new"));
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(&target)
        .unwrap()
        .write_all(b"mutation")
        .unwrap();
    assert!(
        f.library
            .apply_relocation(
                &plan.id,
                &selected(&plan),
                &f.runtime,
                &CancellationToken::default(),
                None
            )
            .is_err()
    );
    assert_eq!(f.library.track_path(&ids[0]).unwrap(), a);
    assert_eq!(f.library.track_path(&ids[1]).unwrap(), b);
    let plan = f.search(&ids, &f.root.join("new"));
    f.library
        .accept_musical_data(
            &ids[0],
            &AcceptedMusicalData {
                id: "changed-musical".into(),
                source: "fixture".into(),
                cues: vec![],
                tempo_markers: vec![],
            },
        )
        .unwrap();
    assert!(
        f.library
            .apply_relocation(
                &plan.id,
                &selected(&plan),
                &f.runtime,
                &CancellationToken::default(),
                None
            )
            .is_err()
    );
    assert_eq!(f.library.track_path(&ids[0]).unwrap(), a);
    assert_eq!(f.library.track_path(&ids[1]).unwrap(), b);
}

#[test]
fn cancelled_search_and_apply_never_change_bindings() {
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let id = f.add(&old);
    std::fs::remove_file(&old).unwrap();
    f.audio("new/A.flac");
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        f.library
            .plan_relocation(
                RelocationRequest {
                    track_ids: vec![id.clone()],
                    search_root: Some(f.root.join("new")),
                    ..Default::default()
                },
                &f.runtime,
                &cancel,
                |_| {}
            )
            .is_err()
    );
    let plan = f.search(std::slice::from_ref(&id), &f.root.join("new"));
    assert!(
        f.library
            .apply_relocation(&plan.id, &selected(&plan), &f.runtime, &cancel, None)
            .is_err()
    );
    assert_eq!(f.library.track_path(&id).unwrap(), old);
}

fn playlist(lib: &Library, ids: Vec<String>) {
    let mut revision = lib.playlist_state().unwrap().revision;
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some("playlist".into()),
            parent_id: None,
            kind: PlaylistKind::Playlist,
            name: "Review order".into(),
            definition: None,
        },
        revision,
    )
    .unwrap();
    revision = lib.playlist_state().unwrap().revision;
    lib.apply_playlist_edit(
        PlaylistEdit::AddMembers {
            id: "playlist".into(),
            track_ids: ids,
        },
        revision,
    )
    .unwrap();
}
#[test]
fn source_and_existing_survivors_rewire_playlist_and_undo_after_reopen() {
    for survivor in ["source", "existing"] {
        let f = Fixture::new();
        let old = f.audio("old/A.flac");
        let source = f.add(&old);
        let target = f.audio("new/A.flac");
        let existing = f.add(&target);
        playlist(&f.library, vec![source.clone(), existing.clone()]);
        std::fs::remove_file(&old).unwrap();
        let digest = hash(&target);
        let plan = f.search(std::slice::from_ref(&source), &f.root.join("new"));
        assert_eq!(
            plan.rows[0].candidates[0].occupied_by.as_deref(),
            Some(existing.as_str())
        );
        let result = f.apply(&plan, 0, 0, survivor).unwrap();
        let keeper = if survivor == "source" {
            &source
        } else {
            &existing
        };
        assert_eq!(
            f.library.manual_playlist_members("playlist").unwrap(),
            vec![keeper.clone()]
        );
        assert_eq!(hash(&target), digest);
        let db = f.db.clone();
        drop(f.library);
        let lib = Library::open(&db).unwrap();
        lib.undo_relocation(&result.operation_id, &CancellationToken::default())
            .unwrap();
        assert_eq!(
            lib.manual_playlist_members("playlist").unwrap(),
            vec![source.clone(), existing.clone()]
        );
        assert_eq!(lib.track_path(&source).unwrap(), old);
        assert_eq!(lib.track_path(&existing).unwrap(), target);
        assert_eq!(hash(&target), digest);
    }
}

#[cfg(unix)]
#[test]
fn symlink_directories_are_not_followed_and_root_boundaries_are_component_aware() {
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let id = f.add(&old);
    std::fs::remove_file(&old).unwrap();
    let outside = f.audio("outside/A.flac");
    let original_hash = hash(&outside);
    std::fs::create_dir_all(f.root.join("new")).unwrap();
    std::os::unix::fs::symlink(outside.parent().unwrap(), f.root.join("new/alias")).unwrap();
    let plan = f.search(&[id], &f.root.join("new"));
    assert!(plan.rows[0].candidates.is_empty());
    assert_eq!(hash(&outside), original_hash);
}

#[test]
fn relocation_operation_lease_rejects_a_concurrent_musical_writer() {
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let id = f.add(&old);
    let operation = f.library.exclusive_operation().unwrap();
    let error = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                f.library.accept_musical_data(
                    &id,
                    &AcceptedMusicalData {
                        id: "concurrent-cues".into(),
                        source: "fixture".into(),
                        cues: vec![],
                        tempo_markers: vec![],
                    },
                )
            })
            .join()
            .unwrap()
            .unwrap_err()
    });
    assert!(
        error.to_string().to_lowercase().contains("busy"),
        "{error:#}"
    );
    assert!(f.library.accepted_musical_data(&id).unwrap().is_none());
    drop(operation);
    f.library
        .accept_musical_data(
            &id,
            &AcceptedMusicalData {
                id: "after-operation".into(),
                source: "fixture".into(),
                cues: vec![],
                tempo_markers: vec![],
            },
        )
        .unwrap();
}

#[test]
fn undo_refuses_to_overwrite_musical_edits_made_after_relocation() {
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let id = f.add(&old);
    std::fs::remove_file(&old).unwrap();
    let target = f.audio("new/A.flac");
    let plan = f.search(std::slice::from_ref(&id), target.parent().unwrap());
    let operation = f.apply(&plan, 0, 0, "source").unwrap();
    f.library
        .accept_musical_data(
            &id,
            &AcceptedMusicalData {
                id: "after-relocation".into(),
                source: "fixture".into(),
                cues: vec![],
                tempo_markers: vec![],
            },
        )
        .unwrap();
    assert!(
        f.library
            .undo_relocation(&operation.operation_id, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(f.library.track_path(&id).unwrap(), target);
    assert_eq!(
        f.library.accepted_musical_data(&id).unwrap().unwrap().id,
        "after-relocation"
    );
}

#[test]
fn shared_destination_requires_a_complete_survivor_and_undo_restores_both_records() {
    let f = Fixture::new();
    let old_a = f.audio("old/x/A.flac");
    let old_b = f.audio("old/y/A.flac");
    let a = f.add(&old_a);
    let b = f.add(&old_b);
    for (id, revision, start) in [(&a, "source-a", 2.0), (&b, "source-b", 7.0)] {
        f.library
            .accept_musical_data(
                id,
                &AcceptedMusicalData {
                    id: revision.into(),
                    source: "fixture".into(),
                    cues: vec![cuerithm_library::AcceptedCue {
                        id: format!("cue-{revision}"),
                        kind: cuerithm_library::CueKind::Cue,
                        start_seconds: start,
                        end_seconds: None,
                        slot: Some(0),
                        memory: false,
                        name: Some(revision.into()),
                        color: None,
                        active_loop: false,
                        source_data: Default::default(),
                    }],
                    tempo_markers: vec![],
                },
            )
            .unwrap();
    }
    playlist(&f.library, vec![a.clone(), b.clone()]);
    std::fs::remove_file(&old_a).unwrap();
    std::fs::remove_file(&old_b).unwrap();
    let target = f.audio("new/A.flac");
    let digest = hash(&target);
    let plan = f.search(&[a.clone(), b.clone()], target.parent().unwrap());
    assert_eq!(plan.rows.len(), 2);
    assert!(plan.rows.iter().all(|row| !row.candidates[0].preselected));
    let row = plan.rows.iter().find(|row| row.track_id == a).unwrap();
    assert_eq!(row.candidates[0].group_track_ids, vec![b.clone()]);
    let mut choice = RelocationChoice {
        track_id: a.clone(),
        candidate_id: row.candidates[0].id.clone(),
        survivor: "source".into(),
        survivor_track_id: None,
    };
    assert!(
        f.library
            .apply_relocation(
                &plan.id,
                std::slice::from_ref(&choice),
                &f.runtime,
                &CancellationToken::default(),
                None
            )
            .is_err()
    );
    choice.survivor_track_id = Some("unreviewed".into());
    assert!(
        f.library
            .apply_relocation(
                &plan.id,
                std::slice::from_ref(&choice),
                &f.runtime,
                &CancellationToken::default(),
                None
            )
            .is_err()
    );
    choice.survivor_track_id = Some(b.clone());
    let result = f
        .library
        .apply_relocation(
            &plan.id,
            &[choice],
            &f.runtime,
            &CancellationToken::default(),
            None,
        )
        .unwrap();
    assert_eq!(result.applied, 2);
    assert_eq!(f.library.track_path(&b).unwrap(), target);
    assert!(f.library.track(&a).unwrap().archived);
    assert!(f.library.track_path(&a).is_err());
    assert_eq!(
        f.library.accepted_musical_data(&b).unwrap().unwrap().cues[0].start_seconds,
        7.0
    );
    assert_eq!(
        f.library.accepted_musical_data(&a).unwrap().unwrap().cues[0].start_seconds,
        2.0
    );
    assert_eq!(
        f.library.manual_playlist_members("playlist").unwrap(),
        vec![b.clone()]
    );
    assert_eq!(hash(&target), digest);
    let db = f.db.clone();
    drop(f.library);
    let library = Library::open(&db).unwrap();
    library
        .undo_relocation(&result.operation_id, &CancellationToken::default())
        .unwrap();
    assert_eq!(
        library.manual_playlist_members("playlist").unwrap(),
        vec![a.clone(), b.clone()]
    );
    assert_eq!(library.track_path(&a).unwrap(), old_a);
    assert_eq!(library.track_path(&b).unwrap(), old_b);
    assert!(!library.track(&a).unwrap().archived);
    assert_eq!(hash(&target), digest);
}

#[cfg(unix)]
#[test]
fn foreign_provenance_uses_the_bound_native_path_flavor_for_later_root_relocation() {
    use cuerithm_library::{
        AnalysisStatus, EncodedPath, PathFlavor, PortableLocator, PortableTrack,
    };
    let f = Fixture::new();
    let old = f.audio("old/A.flac");
    let next = f.audio("new/A.flac");
    let imported = PortableTrack {
        id: "foreign".into(),
        locator: PortableLocator {
            id: "foreign-locator".into(),
            flavor: PathFlavor::WindowsDrive,
            original: EncodedPath::utf8("D:/Music/A.flac"),
            source_root: Some(EncodedPath::utf8("D:/Music")),
            components: Some(vec![EncodedPath::utf8("A.flac")]),
        },
        source_sha256: None,
        title: "Foreign track".into(),
        artist: "Artist".into(),
        album: String::new(),
        genre: String::new(),
        metadata: Default::default(),
        key: None,
        bpm: None,
        energy: None,
        duration: None,
        archived: false,
        origin: "lexicon".into(),
        analysis_status: AnalysisStatus::Imported,
        musical: None,
    };
    f.library.import_portable_tracks(&[imported]).unwrap();
    f.library
        .bind_portable_track("foreign", &old, Some(&hash(&old)))
        .unwrap();
    assert_eq!(
        f.library.source_locator("foreign").unwrap().unwrap().flavor,
        PathFlavor::WindowsDrive
    );
    assert!(
        f.library
            .relocation_roots()
            .unwrap()
            .iter()
            .any(|root| root.source_root == old.parent().unwrap().to_string_lossy())
    );
    let plan = f
        .library
        .plan_relocation(
            RelocationRequest {
                mode: "root".into(),
                source_root: Some(old.parent().unwrap().to_string_lossy().into()),
                destination_root: Some(next.parent().unwrap().to_owned()),
                all_tracks: true,
                ..Default::default()
            },
            &f.runtime,
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap();
    assert_eq!(plan.rows.len(), 1);
    f.apply(&plan, 0, 0, "source").unwrap();
    assert_eq!(f.library.track_path("foreign").unwrap(), next);
}

#[cfg(unix)]
#[test]
fn hardlinked_candidate_aliases_form_one_explicit_survivor_group() {
    let f = Fixture::new();
    let a_old = f.audio("old/A.flac");
    let b_old = f.audio("old/B.flac");
    let a = f.add(&a_old);
    let b = f.add(&b_old);
    std::fs::remove_file(a_old).unwrap();
    std::fs::remove_file(b_old).unwrap();
    let a_new = f.audio("new/A.flac");
    let b_new = f.root.join("new/B.flac");
    std::fs::hard_link(&a_new, &b_new).unwrap();
    let plan = f.search(&[a.clone(), b.clone()], a_new.parent().unwrap());
    let row = plan.rows.iter().find(|row| row.track_id == a).unwrap();
    assert_eq!(row.candidates[0].group_track_ids, vec![b.clone()]);
    assert!(!row.candidates[0].preselected);
    let result = f
        .library
        .apply_relocation(
            &plan.id,
            &[RelocationChoice {
                track_id: a.clone(),
                candidate_id: row.candidates[0].id.clone(),
                survivor: "source".into(),
                survivor_track_id: Some(b.clone()),
            }],
            &f.runtime,
            &CancellationToken::default(),
            None,
        )
        .unwrap();
    assert_eq!(result.applied, 2);
    assert!(same_file::is_same_file(f.library.track_path(&b).unwrap(), &a_new).unwrap());
    assert!(f.library.track_path(&a).is_err());
}

#[cfg(unix)]
#[test]
fn all_tracks_groups_never_archive_the_reviewed_existing_destination_survivor() {
    for keep_existing_as_group_choice in [false, true] {
        let f = Fixture::new();
        let a_old = f.audio("old/A.flac");
        let b_old = f.audio("other/A.flac");
        let a = f.add(&a_old);
        let b = f.add(&b_old);
        let target = f.audio("new/old/A.flac");
        let existing = f.add(&target);
        playlist(&f.library, vec![a.clone(), b.clone(), existing.clone()]);
        std::fs::remove_file(a_old).unwrap();
        std::fs::remove_file(b_old).unwrap();
        for relative in ["new/other/A.flac", "new/new/old/A.flac"] {
            let alias = f.root.join(relative);
            std::fs::create_dir_all(alias.parent().unwrap()).unwrap();
            std::fs::hard_link(&target, alias).unwrap();
        }
        let plan = f
            .library
            .plan_relocation(
                RelocationRequest {
                    mode: "root".into(),
                    source_root: Some(f.root.to_string_lossy().into()),
                    destination_root: Some(f.root.join("new")),
                    all_tracks: true,
                    ..Default::default()
                },
                &f.runtime,
                &CancellationToken::default(),
                |_| {},
            )
            .unwrap();
        let row = plan.rows.iter().find(|row| row.track_id == a).unwrap();
        assert!(row.candidates[0].group_track_ids.contains(&existing));
        let result = f
            .library
            .apply_relocation(
                &plan.id,
                &[RelocationChoice {
                    track_id: a.clone(),
                    candidate_id: row.candidates[0].id.clone(),
                    survivor: "existing".into(),
                    survivor_track_id: Some(if keep_existing_as_group_choice {
                        existing.clone()
                    } else {
                        b.clone()
                    }),
                }],
                &f.runtime,
                &CancellationToken::default(),
                None,
            )
            .unwrap();
        assert!(!f.library.track(&existing).unwrap().archived);
        assert!(
            same_file::is_same_file(f.library.track_path(&existing).unwrap(), &target).unwrap()
        );
        assert!(f.library.track(&a).unwrap().archived);
        assert!(f.library.track(&b).unwrap().archived);
        assert_eq!(result.applied, 2);
        assert_eq!(result.no_op, 1);
        assert_eq!(result.skipped, 0);
        assert_eq!(
            f.library.manual_playlist_members("playlist").unwrap(),
            vec![existing]
        );
    }
}

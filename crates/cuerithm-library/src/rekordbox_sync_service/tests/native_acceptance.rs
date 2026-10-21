//! Explicitly opted-in, private native application acceptance. Never run in CI.
//!
//! The caller supplies a disposable, application-created native library and
//! authored media. This module never creates or changes its native SQL schema,
//! edits global settings, or grants a production write capability. Native UI
//! reopen, playback and visual inspection remain separate manual evidence.
//! The `verify` phase reads an already-applied operation without recovery or
//! publication. `CUERITHM_NATIVE_ACCEPTANCE_INCLUDE_BPM=1` explicitly selects BPM
//! for newly generated requests; the default request leaves BPM unselected.
use super::super::*;
use cuerithm_rekordbox::anlz::{AnlzFile, Beat, ParseLimits};
use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
use std::{cell::Cell, process::Command};

const PURPOSE: &str = "cuerithm-isolated-native-acceptance";
const SENTINEL: &str = ".cuerithm-native-acceptance.json";
const TRACK: &str = "authored-native-acceptance";
const SECOND_TRACK: &str = "authored-native-acceptance-b";
const FOLDER: &str = "authored-native-folder";
const PLAYLIST: &str = "authored-native-playlist";
const SMART: &str = "authored-native-smart";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sentinel {
    version: u32,
    purpose: String,
    database: PathBuf,
    asset_root: PathBuf,
    source: PathBuf,
    #[serde(default)]
    second_source: Option<PathBuf>,
    agent_options: PathBuf,
    application_version: String,
    application_major: u32,
    schema_fingerprint: String,
    #[serde(default)]
    owner_marker: String,
    /// Exact source proofs for native-owned demo media in this selected signed
    /// application's Resources directory or copied into this private case
    /// outside authored-media. Empty preserves authored-only scope.
    #[serde(default)]
    native_owned_extra_sources: Vec<NativeOwnedExtraSource>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeOwnedExtraSource {
    content_id: String,
    uuid: String,
    source: PathBuf,
    source_sha256: String,
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_extra_sources(
    extras: &[NativeOwnedExtraSource],
    resources: &Path,
    case_root: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    if extras.is_empty() {
        return Ok(());
    }
    let canonical_resources = resources
        .canonicalize()
        .ok()
        .filter(|path| path == resources);
    ensure!(
        case_root.canonicalize()? == case_root,
        "Private case root must be canonical"
    );
    let mut ids = BTreeSet::new();
    let mut uuids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for extra in extras {
        ensure!(
            !extra.content_id.is_empty()
                && !extra.uuid.is_empty()
                && valid_sha256(&extra.source_sha256)
                && extra.source.is_absolute()
                && ids.insert(&extra.content_id)
                && uuids.insert(&extra.uuid)
                && paths.insert(&extra.source),
            "Native-owned extras need unique exact identity, path and SHA256 proofs"
        );
        regular(&extra.source)?;
        let source = extra.source.canonicalize()?;
        ensure!(
            source == extra.source
                && ((source.starts_with(case_root)
                    && !source.starts_with(case_root.join("authored-media")))
                    || canonical_resources
                        .as_ref()
                        .is_some_and(|resources| source.starts_with(resources))),
            "Native-owned extra source must be canonical and inside the private case outside authored-media or the selected signed app Resources"
        );
        ensure!(
            file_sha256_cancellable(&source, cancel)? == extra.source_sha256,
            "Native-owned bundled source no longer matches its explicit proof"
        );
    }
    Ok(())
}

struct Case {
    root: PathBuf,
    work: PathBuf,
    source: PathBuf,
    second_source: Option<PathBuf>,
    schema_fingerprint: String,
    native_owned_extra_sources: Vec<NativeOwnedExtraSource>,
    target: RekordboxSyncTarget,
}

#[derive(Default, Serialize, Deserialize)]
struct CaseState {
    operation_id: Option<String>,
    noop_id: Option<String>,
    #[serde(default)]
    earlier_operation_ids: Vec<String>,
}

impl CaseState {
    fn remember_noop(&mut self, id: String) {
        if let Some(previous) = self.noop_id.replace(id)
            && self.noop_id.as_ref() != Some(&previous)
            && !self.earlier_operation_ids.contains(&previous)
        {
            self.earlier_operation_ids.push(previous);
        }
    }

    fn permits_operation(&self, id: &str, phase: &str) -> bool {
        self.operation_id.as_deref() == Some(id)
            || self.earlier_operation_ids.iter().any(|saved| saved == id)
            || (matches!(phase, "restore" | "refuse-restore" | "verify")
                && self.noop_id.as_deref() == Some(id))
    }
}

#[test]
fn private_history_retains_noop_operations_without_admitting_unrelated_ids() {
    let mut state = CaseState {
        operation_id: Some("primary".into()),
        ..Default::default()
    };
    state.remember_noop("noop-one".into());
    assert!(state.permits_operation("noop-one", "restore"));
    assert!(state.permits_operation("noop-one", "verify"));
    assert!(state.permits_operation("noop-one", "refuse-restore"));
    assert!(!state.permits_operation("noop-one", "apply"));
    state.remember_noop("noop-two".into());
    state.remember_noop("noop-two".into());
    assert_eq!(state.earlier_operation_ids, ["noop-one"]);
    assert!(state.permits_operation("noop-one", "restore"));
    assert!(state.permits_operation("noop-two", "restore"));
    assert!(state.permits_operation("primary", "restore"));
    assert!(!state.permits_operation("unrelated", "restore"));
    assert!(!state.permits_operation("unrelated", "verify"));
}

fn regular(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "A regular private file is required"
    );
    Ok(())
}

fn relative_child(root: &Path, relative: &Path) -> Result<PathBuf> {
    use std::path::Component;
    ensure!(
        !relative.as_os_str().is_empty()
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "Private case paths must be strictly relative"
    );
    let path = root.join(relative).canonicalize()?;
    ensure!(
        path != root && path.starts_with(root),
        "Private case path escapes its root"
    );
    Ok(path)
}

fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Private workspace must be a directory"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_private(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

impl Case {
    fn load() -> Result<Self> {
        ensure!(
            cfg!(target_os = "macos"),
            "This private native harness currently requires macOS"
        );
        let supplied = PathBuf::from(
            std::env::var_os("CUERITHM_NATIVE_ACCEPTANCE_ROOT")
                .context("Explicit private acceptance root is required")?,
        );
        ensure!(
            supplied.is_absolute(),
            "Private acceptance root must be absolute"
        );
        let root = supplied.canonicalize()?;
        regular(&root.join(SENTINEL))?;
        let sentinel: Sentinel = serde_json::from_slice(&fs::read(root.join(SENTINEL))?)?;
        ensure!(
            sentinel.version == 1 && sentinel.purpose == PURPOSE,
            "Private acceptance sentinel is not authorized"
        );
        let original = DiscoveryPaths::current()?;
        let original_library = original.default_database_dir;
        ensure!(
            !root.starts_with(&original_library),
            "Original native library is forbidden"
        );
        let canonical_repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?;
        // An original checkout can be outside this worktree. Its path must be
        // supplied explicitly rather than inferred from a machine-specific path.
        let allowed_repo = std::env::var_os("CUERITHM_NATIVE_ACCEPTANCE_REPO")
            .map(PathBuf::from)
            .unwrap_or(canonical_repo)
            .canonicalize()?;
        let boundary = allowed_repo
            .join(".analysis/rekordbox-acceptance")
            .canonicalize();
        let inside = boundary
            .as_ref()
            .is_ok_and(|boundary| root != *boundary && root.starts_with(boundary));
        let caller_marker = std::env::var("CUERITHM_NATIVE_ACCEPTANCE_OWNER_MARKER").ok();
        ensure!(
            inside
                || (!sentinel.owner_marker.is_empty()
                    && caller_marker.as_deref() == Some(sentinel.owner_marker.as_str())),
            "Private root is outside the acceptance boundary without an explicit owner marker"
        );
        let database = relative_child(&root, &sentinel.database)?;
        let asset_root = relative_child(&root, &sentinel.asset_root)?;
        let source = relative_child(&root, &sentinel.source)?;
        regular(&database)?;
        regular(&source)?;
        ensure!(
            !database.starts_with(&original_library),
            "Original native database is forbidden"
        );
        ensure!(asset_root.is_dir(), "Native asset directory is required");
        ensure!(
            source.file_name().and_then(|n| n.to_str()) == Some("CueRithm authored acceptance.wav")
                && source.starts_with(root.join("authored-media")),
            "Only the case's explicitly authored WAV is allowed"
        );
        let second_source = sentinel
            .second_source
            .as_ref()
            .map(|path| relative_child(&root, path))
            .transpose()?;
        if let Some(path) = &second_source {
            regular(path)?;
            ensure!(
                path.file_name().and_then(|name| name.to_str())
                    == Some("CueRithm authored acceptance B.flac")
                    && path.starts_with(root.join("authored-media")),
                "Only the case's explicitly authored second FLAC is allowed"
            );
        }
        ensure!(
            sentinel.agent_options.is_absolute(),
            "Agent options must be explicitly selected"
        );
        regular(&sentinel.agent_options)?;
        ensure!(
            matches!(sentinel.application_major, 6 | 7),
            "Only native 6 or 7 acceptance is supported"
        );
        ensure!(
            sentinel.schema_fingerprint.len() == 64
                && sentinel
                    .schema_fingerprint
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit()),
            "An exact observed schema fingerprint is required"
        );
        let executable = PathBuf::from(
            std::env::var_os("CUERITHM_NATIVE_ACCEPTANCE_EXECUTABLE")
                .context("Explicit signed native executable is required")?,
        )
        .canonicalize()?;
        regular(&executable)?;
        let app = executable
            .ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .context("Selected executable must belong to a native app bundle")?
            .to_path_buf();
        let signature = Command::new("/usr/bin/codesign")
            .args(["--verify", "--strict"])
            .arg(&app)
            .output()?;
        ensure!(
            signature.status.success(),
            "Selected native application signature did not verify"
        );
        let plist = Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
            .arg(app.join("Contents/Info.plist"))
            .output()?;
        ensure!(
            plist.status.success(),
            "Selected native application version is unavailable"
        );
        let version = String::from_utf8(plist.stdout)?.trim().to_owned();
        ensure!(
            version == sentinel.application_version
                && version
                    .split('.')
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    == Some(sentinel.application_major),
            "Selected native application version differs from the private sentinel"
        );
        validate_extra_sources(
            &sentinel.native_owned_extra_sources,
            &app.join("Contents/Resources"),
            &root,
            &CancellationToken::default(),
        )?;
        let installation = TargetInstallation {
            root: app,
            executable,
            version,
            major: sentinel.application_major,
            version_source: "private-native-acceptance-plist".into(),
        };
        let id = stable_target_id(
            &installation,
            &database,
            &sentinel.agent_options,
            &asset_root,
            &TargetEnvironment::Native,
        )?;
        let target = RekordboxSyncTarget {
            environment: TargetEnvironment::Native,
            id,
            installation,
            database,
            options_file: sentinel.agent_options,
            asset_root,
        };
        let work = root.join("cuerithm-harness");
        private_dir(&work)?;
        ensure!(
            work.canonicalize()?.starts_with(&root),
            "Private workspace escaped the case"
        );
        Ok(Self {
            root,
            work,
            source,
            second_source,
            schema_fingerprint: sentinel.schema_fingerprint,
            native_owned_extra_sources: sentinel.native_owned_extra_sources,
            target,
        })
    }

    fn guard(&self) -> Result<()> {
        // Fixture publication bypasses only discovery/capability. The actual
        // foreground/background process guard runs at every publisher boundary.
        ensure_closed()?;
        ensure!(
            self.target.database.canonicalize()?.starts_with(&self.root),
            "Native target escaped the case"
        );
        regular(&self.target.database)
    }

    fn state_path(&self) -> PathBuf {
        self.work.join("phase-state.json")
    }
    fn state(&self) -> Result<CaseState> {
        if !self.state_path().exists() {
            return Ok(CaseState::default());
        }
        regular(&self.state_path())?;
        Ok(serde_json::from_slice(&fs::read(self.state_path())?)?)
    }

    fn verify_operation(&self, library: &Library, id: &str) -> Result<Operation> {
        let op = load(library, id)?;
        ensure!(
            digest(&op.target)? == digest(&self.target)?,
            "Persisted native target differs from this case"
        );
        ensure!(
            op.sources
                .iter()
                .all(|s| (s.id == TRACK && s.path == self.source)
                    || (s.id == SECOND_TRACK && self.second_source.as_ref() == Some(&s.path))),
            "Persisted operation contains non-authored sources"
        );
        ensure!(
            op.resolved.iter().all(|resolved| self
                .sources()
                .iter()
                .any(|(id, _)| *id == resolved.local_id)
                && !self
                    .native_owned_extra_sources
                    .iter()
                    .any(|extra| extra.content_id == resolved.identity.content_id)),
            "Persisted operation resolves outside the authored sources"
        );
        ensure!(
            op.plan
                .rows
                .iter()
                .all(|row| !self
                    .native_owned_extra_sources
                    .iter()
                    .any(
                        |extra| (row.table == "djmdContent" && row.id == extra.content_id)
                            || (row.table == "contentCue" && row.id == extra.uuid)
                            || row.values.get("ContentID")
                                == Some(&TargetValue::Text(extra.content_id.clone()))
                            || row.values.get("ContentUUID")
                                == Some(&TargetValue::Text(extra.uuid.clone()))
                    )),
            "Persisted operation attempts to mutate a native-owned extra track"
        );
        Ok(op)
    }

    fn sources(&self) -> Vec<(&str, &Path)> {
        let mut sources = vec![(TRACK, self.source.as_path())];
        if let Some(path) = &self.second_source {
            sources.push((SECOND_TRACK, path));
        }
        sources
    }
}

fn validate_native_sources<'a>(
    case: &Case,
    collection: &'a RekordboxCollection,
    cancel: &CancellationToken,
) -> Result<BTreeSet<Option<&'a Path>>> {
    validate_extra_sources(
        &case.native_owned_extra_sources,
        &case.target.installation.root.join("Contents/Resources"),
        &case.root,
        cancel,
    )?;
    let mut ids = BTreeSet::new();
    let mut uuids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut extras_seen = BTreeSet::new();
    for track in &collection.tracks {
        let path = track.source.resolved_path.as_deref().map(Path::new);
        ensure!(
            ids.insert(&track.identity.content_id)
                && track
                    .identity
                    .uuid
                    .as_deref()
                    .is_some_and(|uuid| !uuid.is_empty() && uuids.insert(uuid))
                && path.is_some()
                && paths.insert(path),
            "Native case contains a missing or duplicate source or track identity"
        );
        if case
            .sources()
            .iter()
            .any(|(_, source)| Some(*source) == path)
        {
            ensure!(
                !case
                    .native_owned_extra_sources
                    .iter()
                    .any(|extra| extra.content_id == track.identity.content_id
                        || Some(extra.uuid.as_str()) == track.identity.uuid.as_deref()),
                "Authored source collides with a native-owned extra identity"
            );
            continue;
        }
        let extra = case
            .native_owned_extra_sources
            .iter()
            .find(|extra| {
                extra.content_id == track.identity.content_id
                    && Some(extra.uuid.as_str()) == track.identity.uuid.as_deref()
                    && Some(extra.source.as_path()) == path
            })
            .context(
                "Native case contains an unrelated source without an exact bundled identity proof",
            )?;
        ensure!(
            extras_seen.insert(&extra.content_id),
            "Duplicate native-owned extra source"
        );
    }
    ensure!(
        paths.contains(&Some(case.source.as_path()))
            && extras_seen.len() == case.native_owned_extra_sources.len(),
        "Native case is missing its authored source or an explicitly proved native-owned extra"
    );
    Ok(paths)
}

fn initialize_local(
    case: &Case,
    library: &Library,
    runtime: &cuerithm_media::MediaRuntime,
    cancel: &CancellationToken,
) -> Result<()> {
    let sources = case.sources();
    let existing_count: u64 =
        library
            .conn()?
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
    if existing_count != 0 {
        ensure!(
            existing_count == sources.len() as u64,
            "Private harness library contains an incomplete or unrelated fixture"
        );
        for (id, path) in &sources {
            let (saved_hash, saved_path): (String, Vec<u8>) = library.conn()?.query_row(
                "SELECT source_hash,path FROM tracks WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            ensure!(
                saved_hash == file_sha256_cancellable(path, cancel)?
                    && crate::model::bytes_path(&saved_path)? == *path,
                "Authored source changed after fixture preparation"
            );
        }
        return Ok(());
    }
    let mut proofs = Vec::new();
    for (id, source) in &sources {
        let audio = runtime.decode_cancellable(source, &case.work.join("decode"), cancel)?;
        ensure!(
            audio.clock.duration_seconds() > 8.,
            "Authored source needs more than eight playable seconds"
        );
        let publication = json!({
            "grid":{"bpm":128.0,"startTime":0.0},
            "cues":[
                {"name":"Authored cue","startTime":0.9375,"type":"1","position":0,"color":null},
                {"name":"Authored four-beat loop","startTime":1.875,"endTime":3.75,"activeLoop":true,"type":"5","position":1,"color":null}
            ],
            "implementation_sha256":digest(&"private-authored-manual-publication-v2")?,
            "inputs_sha256":digest(&json!([&audio.source_sha256,128,0,0.9375,1.875,3.75]))?
        });
        let analysis = json!({
            "source_sample_rate":audio.info.sample_rate,
            "media":{
                "decoder":audio.decoder_identity,"codec":audio.info.codec,
                "source_sha256":audio.source_sha256,"pcm_sha256":audio.pcm_sha256_cancellable(cancel)?,
                "clock":audio.clock,"export_timing":audio.export_timing
            },
            "publication":publication
        });
        let title = if *id == TRACK {
            "CueRithm authored synchronized A"
        } else {
            "CueRithm authored synchronized B"
        };
        library.conn()?.execute("INSERT INTO tracks(id,path,title,artist,source_hash,state,analysis_json,metadata_json,musical_key,bpm,duration,created,updated,sort_title,sort_artist) VALUES(?1,?2,?3,'CueRithm acceptance',?4,'ready',?5,?6,'8A',128,?7,1,1,?3,'cuerithm acceptance')", params![id, crate::model::path_bytes(source),title,audio.source_sha256,analysis.to_string(),serde_json::to_string(&audio.info)?,audio.clock.duration_seconds()])?;
        proofs.push(json!({"local_id":id,"source_sha256":audio.source_sha256,"pcm_sha256":audio.pcm_sha256_cancellable(cancel)?,"frames":audio.frames,"clock":audio.clock,"publication":publication}));
    }
    for (id, kind, name, definition) in [
        (
            FOLDER,
            PlaylistKind::Folder,
            "CueRithm authored folder",
            None,
        ),
        (
            PLAYLIST,
            PlaylistKind::Playlist,
            "01 Authored ordered set",
            None,
        ),
        (
            SMART,
            PlaylistKind::Smartlist,
            "02 Authored materialized smart set",
            Some(SmartDefinition {
                match_all: true,
                rules: vec![SmartRule {
                    id: "authored-title-regex".into(),
                    field: "title".into(),
                    operator: Operator::StringRegExpMatch,
                    value: RuleValue::Text("^CueRithm authored".into()),
                    or: false,
                }],
            }),
        ),
    ] {
        library.apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some(id.into()),
                parent_id: (id != FOLDER).then(|| FOLDER.into()),
                kind,
                name: name.into(),
                definition,
            },
            library.playlist_state()?.revision,
        )?;
    }
    library.apply_playlist_edit(
        PlaylistEdit::AddMembers {
            id: PLAYLIST.into(),
            track_ids: sources.iter().map(|(id, _)| (*id).into()).collect(),
        },
        library.playlist_state()?.revision,
    )?;
    library.conn()?.execute_batch("INSERT INTO custom_tag_categories(id,name) VALUES('authored-category','CueRithm authored'); INSERT INTO custom_tags(id,category_id,name) VALUES('authored-tag','authored-category','Acceptance'); INSERT INTO track_custom_tags(track_id,tag_id) VALUES('authored-native-acceptance','authored-tag');")?;
    if case.second_source.is_some() {
        library.conn()?.execute(
            "INSERT INTO track_custom_tags(track_id,tag_id) VALUES(?1,'authored-tag')",
            [SECOND_TRACK],
        )?;
    }
    library.refresh_smart_lists(crate::model::now())?;
    let projected = library.dj_sync_library(DjSyncScope::Full)?;
    ensure!(
        projected
            .playlists
            .iter()
            .find(|p| p.id == SMART)
            .is_some_and(|p| p.track_ids
                == sources
                    .iter()
                    .map(|(id, _)| (*id).to_owned())
                    .collect::<Vec<_>>()),
        "Authored smart fixture did not materialize its whole membership"
    );
    write_private(
        &case.work.join("fixture-proof.json"),
        &json!({"authored_manual_publication":true,"engine_accuracy_claim":false,"tracks":proofs,"limitations":["user-facing SavedGrid projects one constant anchor; variable grid needs separate SDK acceptance","native UI reopen, artwork display, and cue/grid playback are manual evidence"]}),
    )?;
    Ok(())
}

fn request(case: &Case) -> RekordboxSyncRequest {
    let options = RekordboxSyncOptions {
        metadata_fields: ["title", "artist", "source_path"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        cues: true,
        beat_grid: true,
        artwork: true,
        playlists: true,
        my_tags: true,
        materialize_smart_playlists: true,
        convert_incompatible_audio: false,
        nearest_color: false,
    };
    let mut request = RekordboxSyncRequest {
        configuration: None,
        target_id: case.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: case
            .sources()
            .into_iter()
            .map(|(id, _)| {
                (
                    id.into(),
                    [
                        ("title", NativeFieldDecision::UseLocal),
                        ("artist", NativeFieldDecision::UseLocal),
                        (
                            "source_path",
                            if id == TRACK {
                                NativeFieldDecision::AcceptEqual
                            } else {
                                NativeFieldDecision::UseLocal
                            },
                        ),
                        ("cues", NativeFieldDecision::UseLocal),
                        ("beat_grid", NativeFieldDecision::UseLocal),
                        (
                            "artwork",
                            if id == SECOND_TRACK {
                                NativeFieldDecision::UseLocal
                            } else {
                                NativeFieldDecision::UseRemote
                            },
                        ),
                    ]
                    .into_iter()
                    .map(|(field, decision)| (field.into(), decision))
                    .collect(),
                )
            })
            .collect(),
    };
    if std::env::var("CUERITHM_NATIVE_ACCEPTANCE_INCLUDE_BPM").as_deref() == Ok("1") {
        request.options.metadata_fields.insert("bpm".into());
        for decisions in request.decisions.values_mut() {
            decisions.insert("bpm".into(), NativeFieldDecision::UseLocal);
        }
    }
    if std::env::var("CUERITHM_NATIVE_ACCEPTANCE_CLEAR_ARTWORK").as_deref() == Ok("1")
        && let Some(decisions) = request.decisions.get_mut(SECOND_TRACK)
    {
        decisions.insert("artwork".into(), NativeFieldDecision::ClearRemote);
    }
    request
}

fn native_proof(
    case: &Case,
    cancel: &CancellationToken,
) -> Result<BTreeMap<PathBuf, (String, std::time::SystemTime)>> {
    fn append(
        path: &Path,
        cancel: &CancellationToken,
        proofs: &mut BTreeMap<PathBuf, (String, std::time::SystemTime)>,
    ) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "Native no-op proof refuses symlinks"
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                append(&entry?.path(), cancel, proofs)?;
            }
        } else {
            ensure!(
                metadata.is_file(),
                "Native no-op proof requires regular files"
            );
            proofs.insert(
                path.to_path_buf(),
                (file_sha256_cancellable(path, cancel)?, metadata.modified()?),
            );
        }
        Ok(())
    }
    let mut proof = BTreeMap::new();
    append(&case.target.database, cancel, &mut proof)?;
    append(&case.target.asset_root, cancel, &mut proof)?;
    Ok(proof)
}

#[derive(PartialEq, Eq)]
struct UnownedNativeProof {
    records: BTreeMap<(String, String), String>,
    extras: String,
    assets: BTreeMap<PathBuf, (String, std::time::SystemTime)>,
}

fn unowned_native_proof(
    case: &Case,
    library: &Library,
    op: &Operation,
    key: &DatabaseKey,
    cancel: &CancellationToken,
) -> Result<UnownedNativeProof> {
    let database = ReadOnlyDatabase::open(&case.target.database, key)?;
    let collection = database.read_collection(Default::default(), cancel)?;
    let asset_records = database.read_asset_records(2_000_000, cancel)?;
    validate_native_sources(case, &collection, cancel)?;
    let planned: BTreeSet<_> = op
        .plan
        .rows
        .iter()
        .map(|row| (&row.table, &row.id))
        .collect();
    let records = collection
        .records
        .values()
        .flatten()
        .chain(&asset_records)
        .filter(|row| !planned.contains(&(&row.table, &row.id)))
        .map(|row| ((row.table.clone(), row.id.clone()), row.fingerprint.clone()))
        .collect();
    let extras = digest(
        &collection
            .tracks
            .iter()
            .filter(|track| {
                case.native_owned_extra_sources
                    .iter()
                    .any(|extra| extra.content_id == track.identity.content_id)
            })
            .map(|track| (&track.identity.content_id, track))
            .collect::<BTreeMap<_, _>>(),
    )?;
    let journal =
        FileTransaction::open(&root(library).join("assets"), &op.asset_journal_id)?.inspect()?;
    let owned_assets: BTreeSet<_> = journal
        .changes
        .iter()
        .map(|change| change.target.as_path())
        .collect();
    let assets = native_proof(case, cancel)?
        .into_iter()
        .filter(|(path, _)| path != &case.target.database && !owned_assets.contains(path.as_path()))
        .collect();
    Ok(UnownedNativeProof {
        records,
        extras,
        assets,
    })
}

fn require_unowned_native_preserved(
    before: &UnownedNativeProof,
    case: &Case,
    library: &Library,
    op: &Operation,
    key: &DatabaseKey,
    cancel: &CancellationToken,
) -> Result<()> {
    ensure!(
        before == &unowned_native_proof(case, library, op, key, cancel)?,
        "Native operation changed unowned raw records, bundled tracks, or asset bytes/modification times"
    );
    Ok(())
}

fn require_frozen_native_state(
    case: &Case,
    key: &DatabaseKey,
    cancel: &CancellationToken,
    expected: &NativeDatabaseState,
) -> Result<()> {
    ensure!(
        NativeDatabaseLease::acquire(&case.target.database, key, cancel)?.state(cancel)?
            == *expected,
        "Native operation differs from its frozen whole-database proof, including unknown rows"
    );
    Ok(())
}

fn verify_authored_sql_cues(
    native: &ImportedTrack,
    owned: &[String],
    records: &[TargetRecord],
) -> Result<()> {
    let cues: Vec<_> = native
        .cues
        .iter()
        .filter(|cue| owned.contains(&cue.id))
        .collect();
    ensure!(
        cues.len() == 3 && owned.len() == 3 && owned.iter().collect::<BTreeSet<_>>().len() == 3,
        "Authored publication must own exactly its two hot cues and synthesized active memory marker"
    );
    for (kind, slot, memory, start, end, active, sql_active, name, loop_size) in [
        (
            1,
            Some(0),
            false,
            0.937,
            None,
            false,
            0,
            "Authored cue",
            None,
        ),
        (
            2,
            Some(1),
            false,
            1.875,
            Some(3.75),
            true,
            1,
            "Authored four-beat loop",
            Some(262145),
        ),
        (
            4,
            None,
            true,
            1.875,
            Some(3.75),
            true,
            0,
            "Authored four-beat loop",
            Some(262145),
        ),
    ] {
        let matching: Vec<_> = cues.iter().filter(|cue| cue.kind == kind).collect();
        ensure!(
            matching.len() == 1,
            "Authored native cue kind is missing or ambiguous"
        );
        let cue = matching[0];
        ensure!(
            cue.slot == slot
                && cue.memory == memory
                && cue.start_seconds == Some(start)
                && cue.end_seconds == end
                && cue.active_loop == active
                && cue.comment == name
                && cue.content_uuid == native.identity.uuid
                && cue.beat_loop_size == loop_size,
            "Authored hot cue or synthesized memory marker interval, slot, activity, size, or identity differs"
        );
        let raw = records
            .iter()
            .find(|record| record.id == cue.id)
            .context("Authored native cue raw row is missing")?;
        ensure!(
            raw.values.get("ActiveLoop") == Some(&TargetValue::Integer(sql_active))
                && raw.values.get("ContentID")
                    == Some(&TargetValue::Text(native.identity.content_id.clone()))
                && raw.values.get("ContentUUID")
                    == native
                        .identity
                        .uuid
                        .as_ref()
                        .map(|uuid| TargetValue::Text(uuid.clone()))
                        .as_ref(),
            "Authored native raw cue activation or track identity differs"
        );
    }
    Ok(())
}

fn verify_authored_grid(beats: &[Beat], duration: f64) -> Result<()> {
    ensure!(
        duration.is_finite() && duration > 8.,
        "Authored duration is unavailable"
    );
    let count = (duration / (60. / 128.)).ceil() as usize;
    ensure!(
        beats.len() == count,
        "Authored grid does not span the full playable source"
    );
    for (index, beat) in beats.iter().enumerate() {
        ensure!(
            beat.number == (index % 4 + 1) as u16
                && beat.tempo == 12800
                && beat.time_ms == (index as f64 * 468.75).floor() as u32,
            "Authored native grid differs from 128 BPM with origin zero"
        );
    }
    Ok(())
}

fn verify_authored_analysis(dat: &AnlzFile, ext: &AnlzFile, duration: f64) -> Result<()> {
    let grids: Vec<_> = dat
        .tags()
        .iter()
        .filter(|tag| tag.fourcc() == *b"PQTZ")
        .collect();
    ensure!(
        grids.len() == 1,
        "Authored DAT grid is missing or ambiguous"
    );
    let beats = grids[0].beats()?;
    verify_authored_grid(&beats, duration)?;
    for tag in ext.tags() {
        match &tag.fourcc() {
            b"PQTZ" => ensure!(tag.beats()? == beats, "EXT grid differs from companion DAT"),
            b"PQT2" => {
                let grid = tag.native_microsecond_grid(&beats)?;
                ensure!(
                    !grid.variable
                        && grid
                            .fractions
                            .iter()
                            .enumerate()
                            .all(|(index, value)| *value == ((index % 4) * 750 % 1000) as u16),
                    "EXT fractional grid differs from 128 BPM companion DAT"
                );
            }
            _ => {}
        }
    }
    for (file, extended) in [(dat, false), (ext, true)] {
        for fourcc in if extended {
            vec![*b"PCOB", *b"PCO2"]
        } else {
            vec![*b"PCOB"]
        } {
            let tags = file
                .tags()
                .iter()
                .filter(|tag| tag.fourcc() == fourcc)
                .map(|tag| tag.cues())
                .collect::<Result<Vec<_>>>()?;
            let lists: Vec<_> = tags.iter().filter(|list| list.kind == 1).collect();
            ensure!(
                lists.len() == 1,
                "Authored hot cue ANLZ list is missing or ambiguous"
            );
            for (slot, kind, start, end, active, name) in [
                (1, 1, 937, 0, false, "Authored cue"),
                (2, 2, 1875, 3750, true, "Authored four-beat loop"),
            ] {
                let cues: Vec<_> = lists[0]
                    .entries
                    .iter()
                    .filter(|cue| cue.hot_cue == slot)
                    .collect();
                ensure!(
                    cues.len() == 1,
                    "Authored ANLZ hot cue slot is missing or ambiguous"
                );
                let cue = cues[0];
                ensure!(
                    cue.cue_type == kind && cue.time_ms == start && cue.loop_time_ms == end,
                    "Authored ANLZ cue/loop interval differs from native SQL semantics"
                );
                if fourcc == *b"PCOB" {
                    ensure!(
                        cue.legacy
                            .as_ref()
                            .is_some_and(|details| details.status == if active { 4 } else { 0 }),
                        "Authored DAT/EXT loop activity differs from native SQL semantics"
                    );
                } else {
                    ensure!(
                        cue.extended
                            .as_ref()
                            .is_some_and(|details| details.comment.as_deref() == Some(name)),
                        "Authored EXT cue comment differs from native SQL semantics"
                    );
                }
            }
        }
    }
    Ok(())
}

fn is_later_native_edit_refusal(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.to_string().as_str(),
            "Native database changed after publication; restore refused"
                | "Original native assets changed; restoring old references would be unsafe"
        )
    })
}

fn frozen_entity_id<'a>(
    receipts: &'a [NativePlaylistPublicationReceipt],
    bindings: Option<&'a BTreeMap<String, NativePlaylistIdentity>>,
    local: &str,
) -> Result<&'a str> {
    receipts
        .iter()
        .find(|receipt| receipt.local_id == local)
        .map(|receipt| receipt.native_id.as_str())
        .or_else(|| {
            bindings
                .and_then(|bindings| bindings.get(local))
                .map(|binding| binding.native_id.as_str())
        })
        .context("Authored entity has no publication receipt or frozen binding")
}

#[test]
fn frozen_readback_identity_uses_receipt_then_exact_binding() {
    let bindings = BTreeMap::from([(
        "parent".into(),
        NativePlaylistIdentity {
            native_id: "stable-parent".into(),
            uuid: None,
            expected_raw_hash: "authored".into(),
            owned: true,
            owned_link_ids: BTreeSet::new(),
        },
    )]);
    let receipts = vec![NativePlaylistPublicationReceipt {
        kind: NativePlaylistObjectKind::Playlist,
        local_id: "parent".into(),
        native_id: "published-parent".into(),
        created_link_ids: BTreeSet::new(),
    }];
    assert_eq!(
        frozen_entity_id(&receipts, Some(&bindings), "parent").unwrap(),
        "published-parent"
    );
    assert_eq!(
        frozen_entity_id(&[], Some(&bindings), "parent").unwrap(),
        "stable-parent"
    );
    assert!(frozen_entity_id(&[], Some(&bindings), "unknown").is_err());
    assert!(frozen_entity_id(&[], None, "parent").is_err());
}

fn verify_native_publication(
    case: &Case,
    library: &Library,
    id: &str,
    key: &DatabaseKey,
    cancel: &CancellationToken,
) -> Result<Value> {
    case.guard()?;
    let op = case.verify_operation(library, id)?;
    let collection = ReadOnlyDatabase::open(&case.target.database, key)?
        .read_collection(Default::default(), cancel)?;
    let native_id = |local: &str| {
        frozen_entity_id(
            &op.receipts,
            op.playlist_plan.as_ref().map(|plan| &plan.bindings),
            local,
        )
    };
    let mut checked_playlists = 0;
    let mut checked_tags = 0;
    if let Some(plan) = &op.playlist_plan {
        for playlist in &plan.playlists {
            if let NativePlaylistMutation::Upsert {
                local_id,
                parent_local_id,
                representation,
                ..
            } = playlist
            {
                let id = native_id(local_id)?;
                let actual = collection
                    .playlists
                    .iter()
                    .find(|playlist| playlist.id == id && playlist.deleted != Some(true))
                    .context("Authored native playlist is missing")?;
                if let Some(parent) = parent_local_id {
                    let parent_id = native_id(parent)?;
                    ensure!(
                        collection
                            .playlists
                            .iter()
                            .any(|playlist| playlist.id == parent_id
                                && playlist.attribute == 1
                                && playlist.deleted != Some(true)),
                        "Authored native parent folder is missing"
                    );
                    ensure!(
                        actual.parent_id.as_deref() == Some(parent_id),
                        "Authored native folder hierarchy differs from the frozen plan"
                    );
                }
                let expected = match representation {
                    NativePlaylistRepresentation::Folder => {
                        ensure!(
                            actual.attribute == 1,
                            "Authored native folder has the wrong representation"
                        );
                        continue;
                    }
                    NativePlaylistRepresentation::Ordinary { content_ids } => content_ids,
                    NativePlaylistRepresentation::MaterializedSmart { content_ids, .. } => {
                        ensure!(
                            actual.attribute == 0
                                && actual.smart_rule.as_deref().is_none_or(str::is_empty),
                            "Whole-list materialization became a native smart definition"
                        );
                        content_ids
                    }
                    NativePlaylistRepresentation::NativeSmart { .. } => anyhow::bail!(
                        "Authored acceptance never requests a native smart definition"
                    ),
                };
                let mut links: Vec<_> = collection
                    .playlist_links
                    .iter()
                    .filter(|link| link.playlist_id == id && link.deleted != Some(true))
                    .collect();
                links.sort_by_key(|link| link.position);
                ensure!(
                    links
                        .iter()
                        .map(|link| link.content_id.as_str())
                        .collect::<Vec<_>>()
                        == expected.iter().map(String::as_str).collect::<Vec<_>>(),
                    "Authored native membership/order differs from the frozen whole list"
                );
                checked_playlists += 1;
            }
        }
        for tag in &plan.tags {
            if let NativeTagMutation::Upsert {
                local_id,
                category_local_id: Some(parent),
                content_ids,
                ..
            } = tag
            {
                let id = native_id(local_id)?;
                let parent_id = native_id(parent)?;
                ensure!(
                    collection.tags.iter().any(|tag| tag.id == parent_id
                        && tag.attribute == 1
                        && tag.deleted != Some(true)),
                    "Authored native MyTag category is missing"
                );
                ensure!(
                    collection.tags.iter().any(|tag| tag.id == id
                        && tag.parent_id.as_deref() == Some(parent_id)
                        && tag.deleted != Some(true)),
                    "Authored MyTag/category relationship is missing"
                );
                let actual: BTreeSet<_> = collection
                    .tag_links
                    .iter()
                    .filter(|link| link.tag_id == id && link.deleted != Some(true))
                    .map(|link| link.content_id.as_str())
                    .collect();
                ensure!(
                    actual == content_ids.iter().map(String::as_str).collect(),
                    "Authored MyTag membership differs from the frozen plan"
                );
                checked_tags += 1;
            }
        }
    }
    let mut artwork_files = 0;
    for resolved in &op.resolved {
        let native = collection
            .tracks
            .iter()
            .find(|track| track.identity.content_id == resolved.identity.content_id)
            .context("Authored native track disappeared")?;
        verify_authored_sql_cues(
            native,
            &resolved.owned_cue_ids,
            &collection.records["djmdCue"],
        )?;
        let analysis = native
            .source
            .analysis_path
            .as_deref()
            .context("Authored native analysis reference is missing")?;
        let dat_path = relative_child(
            &case.target.asset_root,
            Path::new(analysis.trim_start_matches('/')),
        )?;
        ensure!(
            dat_path
                .extension()
                .is_some_and(|extension| extension == "DAT"),
            "Authored analysis must reference DAT"
        );
        let ext_path = dat_path.with_extension("EXT");
        regular(&ext_path)?;
        let duration: f64 = library.conn()?.query_row(
            "SELECT duration FROM tracks WHERE id=?1",
            [&resolved.local_id],
            |row| row.get(0),
        )?;
        verify_authored_analysis(
            &AnlzFile::parse(&fs::read(dat_path)?, ParseLimits::default())?,
            &AnlzFile::parse(&fs::read(ext_path)?, ParseLimits::default())?,
            duration,
        )?;
        if resolved.local_id == SECOND_TRACK {
            let proof = op
                .sources
                .iter()
                .find(|source| source.id == SECOND_TRACK)
                .and_then(|source| source.native_media.as_ref())
                .context("Authored new-track native descriptor is missing")?;
            ensure!(
                native.source.file_type == Some(5)
                    && native.source.sample_rate == Some(proof.sample_rate)
                    && native.source.file_size == Some(proof.size_bytes),
                "Authored FLAC native descriptor differs from its measured source"
            );
            if op
                .request
                .decisions
                .get(SECOND_TRACK)
                .and_then(|decisions| decisions.get("artwork"))
                == Some(&NativeFieldDecision::ClearRemote)
            {
                ensure!(
                    native
                        .source
                        .artwork_path
                        .as_deref()
                        .is_none_or(str::is_empty),
                    "Authored clear-artwork publication retained a native artwork reference"
                );
                continue;
            }
            let runtime = cuerithm_media::MediaRuntime::from_bundle(&PathBuf::from(
                std::env::var_os("CUERITHM_MEDIA_RUNTIME")
                    .context("Explicit media runtime is required")?,
            ))?;
            let artwork = cuerithm_dj_sync::native_rekordbox_artwork(
                case.second_source
                    .as_ref()
                    .context("Authored covered source is missing")?,
                &runtime,
                cancel,
            )?
            .context("Authored second source contains no embedded artwork")?;
            let main = PathBuf::from(
                native
                    .source
                    .artwork_path
                    .as_deref()
                    .context("Authored native artwork reference is missing")?
                    .trim_start_matches('/'),
            );
            let stem = main
                .file_stem()
                .and_then(|stem| stem.to_str())
                .context("Authored native artwork filename is invalid")?;
            for (suffix, expected) in [
                ("", artwork.main),
                ("_m", artwork.medium),
                ("_s", artwork.small),
            ] {
                let path = main.with_file_name(format!("{stem}{suffix}.jpg"));
                let path = relative_child(&case.target.asset_root, &path)?;
                ensure!(
                    fs::read(path)? == expected,
                    "Authored native artwork size differs from deterministic source normalization"
                );
                artwork_files += 1;
            }
        }
    }
    Ok(
        json!({"checked_playlists":checked_playlists,"checked_tag_memberships":checked_tags,"checked_artwork_files":artwork_files,"checked_native_tracks":op.resolved.len(),"native_ui_verified":false,"native_playback_verified":false}),
    )
}

#[test]
fn bundled_extra_authorization_requires_exact_containment_hash_and_unique_identity() {
    let temporary = tempfile::tempdir().unwrap();
    let home = temporary.path().canonicalize().unwrap();
    let resources = home.join("Authored.app/Contents/Resources");
    fs::create_dir_all(&resources).unwrap();
    let case_root = home.join("private-case");
    fs::create_dir(&case_root).unwrap();
    let source = resources.join("authored-demo.wav");
    fs::write(&source, b"synthetic source bytes, not music").unwrap();
    let cancel = CancellationToken::default();
    let extra = NativeOwnedExtraSource {
        content_id: "native-demo".into(),
        uuid: "demo-uuid".into(),
        source: source.clone(),
        source_sha256: file_sha256_cancellable(&source, &cancel).unwrap(),
    };
    validate_extra_sources(
        std::slice::from_ref(&extra),
        &resources,
        &case_root,
        &cancel,
    )
    .unwrap();
    assert!(
        validate_extra_sources(
            &[extra.clone(), extra.clone()],
            &resources,
            &case_root,
            &cancel
        )
        .is_err()
    );
    let mut changed = extra.clone();
    changed.uuid.clear();
    assert!(validate_extra_sources(&[changed], &resources, &case_root, &cancel).is_err());
    let outside = home.join("arbitrary.wav");
    fs::write(&outside, b"synthetic source bytes, not music").unwrap();
    let mut changed = extra.clone();
    changed.source = outside;
    assert!(validate_extra_sources(&[changed], &resources, &case_root, &cancel).is_err());
    let copied = case_root.join("Music/native-demo.wav");
    fs::create_dir(copied.parent().unwrap()).unwrap();
    fs::copy(&source, &copied).unwrap();
    let mut copied_extra = extra.clone();
    copied_extra.source = copied;
    validate_extra_sources(&[copied_extra.clone()], &resources, &case_root, &cancel).unwrap();
    let authored = case_root.join("authored-media/unrelated.wav");
    fs::create_dir(authored.parent().unwrap()).unwrap();
    fs::copy(&source, &authored).unwrap();
    copied_extra.source = authored;
    assert!(validate_extra_sources(&[copied_extra], &resources, &case_root, &cancel).is_err());
    fs::write(&source, b"later source bytes").unwrap();
    assert!(validate_extra_sources(&[extra], &resources, &case_root, &cancel).is_err());
}

#[test]
fn native_source_authorization_keeps_authored_only_default_and_exact_extra_identity() {
    let fixture = super::fixture(false);
    let op = load(&fixture.library, &fixture.id).unwrap();
    let cancel = CancellationToken::default();
    let mut collection = ReadOnlyDatabase::open(&fixture.native, &super::key())
        .unwrap()
        .read_collection(Default::default(), &cancel)
        .unwrap();
    let mut case = Case {
        root: fixture._dir.path().canonicalize().unwrap(),
        work: fixture.library.data_dir().to_path_buf(),
        source: PathBuf::from("/missing/authored.flac"),
        second_source: None,
        schema_fingerprint: collection.schema_fingerprint.clone(),
        native_owned_extra_sources: vec![],
        target: op.target,
    };
    validate_native_sources(&case, &collection, &cancel).unwrap();
    let resources = case.target.installation.root.join("Contents/Resources");
    fs::create_dir_all(&resources).unwrap();
    let source = resources.join("authored-demo.wav");
    fs::write(&source, b"synthetic bundled bytes").unwrap();
    let mut extra = collection.tracks[0].clone();
    extra.identity.content_id = "demo".into();
    extra.identity.uuid = Some("demo-uuid".into());
    extra.source.resolved_path = Some(source.to_str().unwrap().into());
    collection.tracks.push(extra);
    assert!(validate_native_sources(&case, &collection, &cancel).is_err());
    case.native_owned_extra_sources
        .push(NativeOwnedExtraSource {
            content_id: "demo".into(),
            uuid: "demo-uuid".into(),
            source: source.clone(),
            source_sha256: file_sha256_cancellable(&source, &cancel).unwrap(),
        });
    validate_native_sources(&case, &collection, &cancel).unwrap();
    // Native app playback can update rows between separate closed-app phases.
    collection.tracks[1].raw_hash = "later-native-playback-row".into();
    validate_native_sources(&case, &collection, &cancel).unwrap();
    for field in ["id", "uuid", "path"] {
        let mut changed = collection.clone();
        match field {
            "id" => changed.tracks[1].identity.content_id = "another-id".into(),
            "uuid" => changed.tracks[1].identity.uuid = Some("another-uuid".into()),
            _ => {
                changed.tracks[1].source.resolved_path = Some(case.source.to_str().unwrap().into())
            }
        }
        assert!(validate_native_sources(&case, &changed, &cancel).is_err());
    }
    collection.tracks.pop();
    assert!(validate_native_sources(&case, &collection, &cancel).is_err());
}

fn authored_test_beats(duration: f64) -> Vec<Beat> {
    (0..(duration / (60. / 128.)).ceil() as usize)
        .map(|index| Beat {
            number: (index % 4 + 1) as u16,
            tempo: 12800,
            time_ms: (index as f64 * 468.75).floor() as u32,
        })
        .collect()
}

fn authored_test_cues() -> Vec<NativeCue> {
    [
        (0, 0.9375, None, false, "Authored cue"),
        (1, 1.875, Some(3.75), true, "Authored four-beat loop"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (slot, start, end, active_loop, name))| NativeCue {
        slot: NativeCueSlot::Hot(slot),
        start: NativeTime::from_seconds(start).unwrap(),
        end: end.map(|seconds| NativeTime::from_seconds(seconds).unwrap()),
        active_loop,
        order_first: index as u16,
        order_last: index as u16,
        comment: name.into(),
        memory_color: 0,
        color: [0; 4],
        loop_fraction: (0, 0),
    })
    .collect()
}

#[test]
fn authored_analysis_checks_full_grid_and_paired_native_cue_semantics() {
    use cuerithm_rekordbox::anlz::AnlzTag;
    let beats = authored_test_beats(10.);
    let cues = authored_test_cues();
    let make = |beats: &[Beat], cues: &[NativeCue], kind| {
        AnlzFile::parse(
            &new_native_analysis("/synthetic/authored.wav", beats, cues, kind).unwrap(),
            ParseLimits::default(),
        )
        .unwrap()
    };
    let dat = make(&beats, &cues, NativeAnalysisKind::Dat);
    let ext = make(&beats, &cues, NativeAnalysisKind::Ext);
    verify_authored_analysis(&dat, &ext, 10.).unwrap();
    let mut tags = ext.tags().to_vec();
    let fractions: Vec<_> = (0..beats.len())
        .map(|index| ((index % 4) * 750 % 1000) as u16)
        .collect();
    let mut native_grid = AnlzTag::microsecond_grid(&beats, &fractions, false, 0)
        .unwrap()
        .bytes()
        .to_vec();
    // Native rekordbox reserves this word; the Lexicon constructor writes its
    // one-based downbeat index instead. Author only this synthetic test tag.
    native_grid[48..52].copy_from_slice(&0u32.to_be_bytes());
    tags.push(AnlzTag::parse(&native_grid, ParseLimits::default()).unwrap());
    verify_authored_analysis(&dat, &AnlzFile::new(tags).unwrap(), 10.).unwrap();
    for field in ["origin", "tempo", "bar", "span"] {
        let mut changed = beats.clone();
        match field {
            "origin" => changed[0].time_ms = 1,
            "tempo" => changed[1].tempo = 12700,
            "bar" => changed[1].number = 3,
            _ => {
                changed.pop();
            }
        }
        assert!(verify_authored_grid(&changed, 10.).is_err());
    }
    for field in ["point", "slot", "loop-end", "active"] {
        let mut changed = cues.clone();
        match field {
            "point" => changed[0].start = NativeTime::from_seconds(0.938).unwrap(),
            "slot" => changed[0].slot = NativeCueSlot::Hot(2),
            "loop-end" => changed[1].end = Some(NativeTime::from_seconds(4.).unwrap()),
            _ => changed[1].active_loop = false,
        }
        assert!(
            verify_authored_analysis(&dat, &make(&beats, &changed, NativeAnalysisKind::Ext), 10.)
                .is_err()
        );
    }
}

#[test]
fn authored_sql_readback_checks_sdk_generated_hot_cues_and_synthesized_memory_marker() {
    let fixture = super::fixture(false);
    let collection = ReadOnlyDatabase::open(&fixture.native, &super::key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    let mut track = collection.tracks[0].clone();
    let prepared = prepare_native_rows(
        &collection,
        &NativePlanRequest {
            source: fixture.native.clone(),
            now: "2026-10-21 12:00:00.000 +00:00".into(),
            iso_now: "2026-10-21T12:00:00.000Z".into(),
            id_seed: [7; 32],
            tracks: vec![NativeTrackDraft {
                local_id: TRACK.into(),
                identity: Some(track.identity.clone()),
                expected_raw_hash: Some(track.raw_hash.clone()),
                source_path: PathBuf::from("/missing/authored.flac"),
                fields: BTreeMap::new(),
                cues: Some(authored_test_cues()),
                cue_bpm: Some(128.),
                cue_colors: None,
                replace_cues: vec![],
                analysis_path: None,
            }],
            entities: vec![],
            auxiliary_records: vec![],
            share_root: None,
        },
    )
    .unwrap();
    let records: Vec<_> = prepared
        .plan
        .rows
        .iter()
        .filter(|row| row.table == "djmdCue")
        .map(|row| TargetRecord {
            table: row.table.clone(),
            id: row.id.clone(),
            values: row.values.clone(),
            fingerprint: digest(&row.values).unwrap(),
        })
        .collect();
    let integer = |row: &TargetRecord, name: &str| match row.values.get(name) {
        Some(TargetValue::Integer(value)) => Some(*value),
        Some(TargetValue::Null) | None => None,
        _ => panic!("Synthetic native cue integer field has wrong type"),
    };
    let text = |row: &TargetRecord, name: &str| match row.values.get(name) {
        Some(TargetValue::Text(value)) => Some(value.clone()),
        Some(TargetValue::Null) | None => None,
        _ => panic!("Synthetic native cue text field has wrong type"),
    };
    track.cues = records
        .iter()
        .map(|row| {
            let kind = integer(row, "Kind").unwrap();
            ImportedCue {
                id: row.id.clone(),
                uuid: text(row, "UUID"),
                content_uuid: text(row, "ContentUUID"),
                kind,
                slot: if (1..=3).contains(&kind) {
                    Some((kind - 1) as u8)
                } else {
                    None
                },
                memory: kind == 0 || kind == 4,
                start_seconds: integer(row, "InMsec").map(|value| value as f64 / 1000.),
                end_seconds: integer(row, "OutMsec")
                    .filter(|value| *value >= 0)
                    .map(|value| value as f64 / 1000.),
                active_loop: kind == 4
                    || integer(row, "ActiveLoop").is_some_and(|value| value != 0),
                comment: text(row, "Comment").unwrap(),
                color: integer(row, "Color"),
                color_table_index: integer(row, "ColorTableIndex"),
                beat_loop_size: integer(row, "BeatLoopSize"),
                raw_hash: row.fingerprint.clone(),
            }
        })
        .collect();
    let owned = &prepared.tracks[0].owned_cue_ids;
    verify_authored_sql_cues(&track, owned, &records).unwrap();
    for field in [
        "point",
        "end",
        "slot",
        "active",
        "size",
        "uuid",
        "missing-marker",
        "marker-interval",
        "marker-activation",
    ] {
        let mut changed = track.clone();
        let mut changed_records = records.clone();
        let marker = changed.cues.iter().position(|cue| cue.kind == 4).unwrap();
        let point = changed.cues.iter().position(|cue| cue.kind == 1).unwrap();
        let hot_loop = changed.cues.iter().position(|cue| cue.kind == 2).unwrap();
        match field {
            "point" => changed.cues[point].start_seconds = Some(0.938),
            "end" => changed.cues[hot_loop].end_seconds = Some(4.),
            "slot" => changed.cues[hot_loop].slot = Some(2),
            "active" => changed.cues[hot_loop].active_loop = false,
            "size" => changed.cues[marker].beat_loop_size = Some(131073),
            "uuid" => changed.cues[marker].content_uuid = Some("foreign".into()),
            "missing-marker" => {
                changed.cues.remove(marker);
            }
            "marker-interval" => changed.cues[marker].end_seconds = Some(4.),
            _ => changed_records
                .iter_mut()
                .find(|row| row.id == changed.cues[marker].id)
                .unwrap()
                .values
                .insert("ActiveLoop".into(), TargetValue::Integer(1))
                .map(|_| ())
                .unwrap(),
        }
        assert!(verify_authored_sql_cues(&changed, owned, &changed_records).is_err());
    }
}

#[test]
fn operation_preservation_detects_unplanned_raw_rows_and_unowned_asset_changes() {
    let fixture = super::fixture(true);
    let op = load(&fixture.library, &fixture.id).unwrap();
    let case = Case {
        root: fixture._dir.path().canonicalize().unwrap(),
        work: fixture.library.data_dir().to_path_buf(),
        source: PathBuf::from("/missing/authored.flac"),
        second_source: None,
        schema_fingerprint: op.plan.schema_fingerprint.clone(),
        native_owned_extra_sources: vec![],
        target: op.target.clone(),
    };
    let cancel = CancellationToken::default();
    let key = super::key();
    let before = unowned_native_proof(&case, &fixture.library, &op, &key, &cancel).unwrap();
    publication::fixture_apply(&fixture.library, &fixture.id, &key, &cancel, &|| Ok(())).unwrap();
    require_unowned_native_preserved(&before, &case, &fixture.library, &op, &key, &cancel).unwrap();
    let connection = super::open(&fixture.native);
    connection
        .execute("INSERT INTO djmdArtist(ID) VALUES('unowned-new-row')", [])
        .unwrap();
    assert!(
        require_unowned_native_preserved(&before, &case, &fixture.library, &op, &key, &cancel)
            .is_err()
    );
    connection
        .execute("DELETE FROM djmdArtist WHERE ID='unowned-new-row'", [])
        .unwrap();
    drop(connection);
    require_unowned_native_preserved(&before, &case, &fixture.library, &op, &key, &cancel).unwrap();
    require_frozen_native_state(&case, &key, &cancel, &op.after).unwrap();
    let connection = super::open(&fixture.native);
    connection
        .execute(
            "UPDATE unknownRows SET payload=X'CAFE' WHERE id='external'",
            [],
        )
        .unwrap();
    assert!(require_frozen_native_state(&case, &key, &cancel, &op.after).is_err());
    drop(connection);
    fs::write(&fixture.old_asset, b"unexpected asset change").unwrap();
    assert!(
        require_unowned_native_preserved(&before, &case, &fixture.library, &op, &key, &cancel)
            .is_err()
    );
}

#[test]
fn later_native_edit_refusal_does_not_accept_generic_restore_errors() {
    assert!(is_later_native_edit_refusal(
        &anyhow::anyhow!("Native database changed after publication; restore refused")
            .context("outer")
    ));
    assert!(is_later_native_edit_refusal(&anyhow::anyhow!(
        "Original native assets changed; restoring old references would be unsafe"
    )));
    for message in [
        "Native synchronization ownership changed after publication; restore refused",
        "Application is open",
        "Missing native before-image",
        "A synced audio source changed after publication; restore refused",
        "Native before-image proof changed",
        "Native database changed after publication; restore refused: arbitrary detail",
    ] {
        assert!(!is_later_native_edit_refusal(&anyhow::anyhow!(
            message.to_owned()
        )));
    }
}

fn run_phase(case: &Case, phase: &str) -> Result<Value> {
    ensure!(
        matches!(
            phase,
            "review"
                | "preview"
                | "apply"
                | "noop"
                | "restore"
                | "refuse-restore"
                | "public-noop"
                | "interrupt"
                | "recover"
                | "edit-local"
                | "verify"
        ),
        "Unknown native acceptance phase"
    );
    case.guard()?;
    let cancel = CancellationToken::default();
    let key = key_for(&case.target, DesktopPlatform::MacOS, &cancel)?;
    let database = ReadOnlyDatabase::open(&case.target.database, &key)?;
    let schema = database.schema()?;
    ensure!(
        schema.fingerprint == case.schema_fingerprint,
        "Private native schema changed"
    );
    ensure!(
        accepted_write_capability(&case.target.installation, &schema)?.can_write()
            == (phase == "public-noop"),
        "Fixture phases require an unpromoted profile; public-noop requires an accepted production profile"
    );
    let collection = database.read_collection(Default::default(), &cancel)?;
    let native_paths = validate_native_sources(case, &collection, &cancel)?;
    let creating_second = case
        .second_source
        .as_ref()
        .is_some_and(|path| !native_paths.contains(&Some(path.as_path())));
    drop(database);
    let library = Library::open(&case.work.join("library.db"))?;
    let _lock = lock(&library)?;
    let mut state = case.state()?;
    let mut summary = json!({"phase":phase,"native_major":case.target.installation.major,"tables":schema.objects.iter().filter(|o|o.kind=="table").count(),"schema_objects":schema.objects.len(),"production_write_enabled":false});
    if phase == "public-noop" {
        drop(_lock);
        let before = native_proof(case, &cancel)?;
        let discovered: Vec<_> = library
            .discover_rekordbox_targets()?
            .into_iter()
            .filter(|target| {
                target.database == case.target.database
                    && target.options_file == case.target.options_file
                    && target.installation.executable == case.target.installation.executable
            })
            .collect();
        ensure!(
            discovered.len() == 1,
            "Public discovery did not uniquely match the exact private database, options, and executable"
        );
        let target = &discovered[0];
        let preflight = library.preflight_rekordbox_sync(&target.id, &cancel)?;
        ensure!(
            preflight.readable
                && preflight.writable
                && preflight.restrictions.is_empty()
                && preflight.schema_fingerprint.as_deref()
                    == Some(case.schema_fingerprint.as_str())
                && digest(&preflight.target)? == digest(target)?,
            "Public preflight did not accept the exact discovered native profile"
        );
        let media = cuerithm_media::MediaRuntime::from_bundle(&PathBuf::from(
            std::env::var_os("CUERITHM_MEDIA_RUNTIME")
                .context("Explicit bundled media runtime is required")?,
        ))?;
        let runtime = RekordboxSyncRuntime::current(Some(media))?;
        let mut selected = request(case);
        selected.target_id = target.id.clone();
        let preview = library.preview_rekordbox_sync(selected, &runtime, &cancel)?;
        let op = load(&library, &preview.id)?;
        let authored = case.sources();
        ensure!(
            digest(&op.target)? == digest(target)?
                && op.sources.len() == authored.len()
                && op.sources.iter().all(|source| authored
                    .iter()
                    .any(|(id, path)| source.id == *id && source.path == *path))
                && op.resolved.len() == 2
                && authored.len() == 2
                && op
                    .resolved
                    .iter()
                    .map(|track| track.local_id.as_str())
                    .collect::<BTreeSet<_>>()
                    == authored.iter().map(|(id, _)| *id).collect(),
            "Public preview escaped the two explicitly authored sources or discovered target"
        );
        ensure!(
            preview.can_apply && preview.changed_rows == 0 && op.plan.rows.is_empty(),
            "Public preview is not an applicable native row no-op"
        );
        ensure!(
            native_proof(case, &cancel)? == before,
            "Public discovery/preflight/preview changed native bytes or modification times"
        );
        let applied = library.apply_rekordbox_sync(&preview.id, &cancel)?;
        ensure!(
            applied.status == "applied"
                && applied.changed_rows == 0
                && load(&library, &preview.id)?.status == "applied",
            "Public no-op apply did not complete without native row changes"
        );
        ensure!(
            native_proof(case, &cancel)? == before,
            "Public no-op apply changed native bytes or modification times"
        );
        let restored = library.restore_rekordbox_sync(&preview.id, &cancel)?;
        ensure!(
            restored.status == "restored"
                && restored.changed_rows == 0
                && load(&library, &preview.id)?.status == "restored",
            "Public no-op restore did not complete without native row changes"
        );
        ensure!(
            native_proof(case, &cancel)? == before,
            "Public no-op restore changed native bytes or modification times"
        );
        summary["operation_id"] = json!(preview.id);
        summary["status"] = json!("noop_applied_and_restored");
        summary["changed_rows"] = json!(0);
        summary["production_write_enabled"] = json!(true);
        summary["public_api_verified"] = json!(true);
        return Ok(summary);
    }
    if phase == "edit-local" {
        ensure!(
            case.second_source.is_some(),
            "The reorder phase requires two authored tracks"
        );
        ensure_settled(&library, &case.target.database)?;
        let before = native_proof(case, &cancel)?;
        library.apply_playlist_edit(
            PlaylistEdit::ReorderMembers {
                id: PLAYLIST.into(),
                track_ids: vec![SECOND_TRACK.into(), TRACK.into()],
            },
            library.playlist_state()?.revision,
        )?;
        ensure!(
            library.manual_playlist_members(PLAYLIST)? == [SECOND_TRACK, TRACK],
            "Authored local reorder did not persist"
        );
        ensure!(
            native_proof(case, &cancel)? == before,
            "A local playlist edit modified the native target"
        );
        summary["reordered_members"] = json!(2);
        summary["native_write"] = json!(false);
        return Ok(summary);
    }
    if phase == "review" || phase == "preview" || phase == "noop" {
        let runtime_root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(PathBuf::from)
            .context("Explicit bundled media runtime is required")?;
        let runtime = cuerithm_media::MediaRuntime::from_bundle(&runtime_root)?;
        initialize_local(case, &library, &runtime, &cancel)?;
        // Restore and local edits invalidate captured smart membership. Refresh
        // through the public API before explicitly materializing it again.
        library.refresh_smart_lists(crate::model::now())?;
        ensure_settled(&library, &case.target.database)?;
        let before = native_proof(case, &cancel)?;
        let mut selected = request(case);
        if phase == "review" {
            selected.decisions.clear();
        }
        let preview = planning::preview(
            &library,
            selected,
            case.target.clone(),
            &key,
            Some(&runtime),
            &cancel,
            planning::PreviewPermit::AuthoredFixture,
        )?;
        let op = case.verify_operation(&library, &preview.id)?;
        write_private(
            &case
                .work
                .join(format!("private-preview-{}.json", preview.id)),
            &preview,
        )?;
        if phase == "review" {
            if creating_second {
                ensure!(
                    !preview.can_apply
                        && !preview.pending_entities.is_empty()
                        && preview
                            .fields
                            .iter()
                            .any(|field| field.track_id == SECOND_TRACK
                                && field.field == "title"
                                && field.decision.is_none()),
                    "The first new-track preview is not readable and gated on explicit choices"
                );
                ensure!(
                    op.resolved
                        .iter()
                        .all(|track| track.local_id != SECOND_TRACK),
                    "The undecided new track was staged"
                );
            }
            summary["reviewed_fields"] = json!(preview.fields.len());
            summary["unresolved_fields"] = json!(
                preview
                    .fields
                    .iter()
                    .filter(|field| field.decision.is_none())
                    .count()
            );
            summary["conflicts"] = json!(
                preview
                    .fields
                    .iter()
                    .filter(|field| matches!(field.result, NativeReconciliation::Conflict { .. }))
                    .count()
            );
            summary["pending_entities"] = json!(preview.pending_entities.len());
            summary["can_apply"] = json!(preview.can_apply);
            summary["review_operation_id"] = json!(preview.id);
            drop(_lock);
            library.discard_rekordbox_sync(&preview.id)?;
            ensure!(
                native_proof(case, &cancel)? == before,
                "Undecided review modified the native target"
            );
            return Ok(summary);
        }
        ensure!(
            op.resolved.len() == case.sources().len()
                && op
                    .resolved
                    .iter()
                    .any(|track| track.local_id == TRACK && !track.created)
                && (!creating_second
                    || op
                        .resolved
                        .iter()
                        .any(|track| track.local_id == SECOND_TRACK && track.created)),
            "Acceptance did not retain the imported identity and create only the explicitly authored second track"
        );
        ensure!(
            preview.can_apply && preview.skipped_fields == 0,
            "Authored acceptance has unresolved field choices"
        );
        if phase == "noop" {
            state.remember_noop(preview.id.clone());
            write_private(&case.state_path(), &state)?;
            ensure!(
                preview.changed_rows == 0 && op.plan.rows.is_empty(),
                "Repeated sync is not a native row no-op"
            );
            ensure!(
                publication::fixture_apply(&library, &preview.id, &key, &cancel, &|| case.guard())?
                    .changed_rows
                    == 0,
                "No-op publication changed native rows"
            );
            ensure!(
                native_proof(case, &cancel)? == before,
                "No-op rewrote native database or resource bytes/mtime"
            );
            summary["readback"] =
                verify_native_publication(case, &library, &preview.id, &key, &cancel)?;
        } else {
            if let Some(previous) = state.operation_id.take() {
                ensure!(
                    matches!(
                        load(&library, &previous)?.status.as_str(),
                        "applied" | "restored" | "rolled_back"
                    ),
                    "Previous private native operation is not settled"
                );
                state.earlier_operation_ids.push(previous);
            }
            state.operation_id = Some(preview.id.clone());
            write_private(&case.state_path(), &state)?;
            ensure!(
                native_proof(case, &cancel)? == before,
                "Preview modified the native database or assets"
            );
        }
        summary["operation_id"] = json!(preview.id);
        summary["tracks"] = json!(preview.tracks);
        summary["changed_rows"] = json!(preview.changed_rows);
        summary["reviewed_fields"] = json!(preview.fields.len());
        summary["materialized_smart_lists"] = json!(
            op.playlist_plan
                .iter()
                .flat_map(|p| &p.playlists)
                .filter(|p| matches!(
                    p,
                    NativePlaylistMutation::Upsert {
                        representation: NativePlaylistRepresentation::MaterializedSmart { .. },
                        ..
                    }
                ))
                .count()
        );
    } else {
        let selected_id = std::env::var("CUERITHM_NATIVE_ACCEPTANCE_OPERATION_ID").ok();
        let id = selected_id
            .as_deref()
            .or(state.operation_id.as_deref())
            .context("Run the private preview phase first")?;
        ensure!(
            state.permits_operation(id, phase),
            "Operation is outside this private acceptance history"
        );
        let operation = case.verify_operation(&library, id)?;
        if phase == "verify" {
            ensure!(
                operation.status == "applied",
                "Read-only verification requires an applied native operation"
            );
            let before = native_proof(case, &cancel)?;
            summary["readback"] = verify_native_publication(case, &library, id, &key, &cancel)?;
            ensure!(
                native_proof(case, &cancel)? == before,
                "Read-only verification modified native bytes or modification times"
            );
            summary["operation_id"] = json!(id);
            summary["status"] = json!("verified_applied");
            summary["native_write"] = json!(false);
            return Ok(summary);
        }
        if phase == "refuse-restore" {
            ensure!(
                operation.status == "applied",
                "Restore-refusal acceptance requires an applied operation"
            );
            let before = native_proof(case, &cancel)?;
            let error = publication::fixture_restore(&library, id, &key, &cancel, &|| case.guard())
                .err()
                .context("Later native edit unexpectedly allowed restore")?;
            if !is_later_native_edit_refusal(&error) {
                return Err(error.context(
                    "Restore failed without an exact later native database or asset refusal",
                ));
            }
            ensure!(
                load(&library, id)?.status == operation.status,
                "Refused restore changed the operation status"
            );
            ensure!(
                native_proof(case, &cancel)? == before,
                "Refused restore changed native bytes or modification times"
            );
            summary["operation_id"] = json!(id);
            summary["status"] = json!("restore_refused_later_native_edit");
            summary["refusal_guard"] = json!(if error.chain().any(|cause| cause.to_string()
                == "Original native assets changed; restoring old references would be unsafe")
            {
                "native_asset"
            } else {
                "native_database"
            });
            summary["native_write"] = json!(false);
            return Ok(summary);
        }
        let unowned_before = unowned_native_proof(case, &library, &operation, &key, &cancel)?;
        let result = match phase {
            "apply" => publication::fixture_apply(&library, id, &key, &cancel, &|| case.guard())?,
            "restore" => {
                publication::fixture_restore(&library, id, &key, &cancel, &|| case.guard())?
            }
            "recover" => {
                publication::fixture_recover(&library, id, &key, &cancel, &|| case.guard())?
            }
            "interrupt" => {
                let injected = Cell::new(false);
                let result = publication::fixture_apply(&library, id, &key, &cancel, &|| {
                    case.guard()?;
                    // Publisher callbacks can hold the library mutex. Observe
                    // the last committed phase through a separate connection.
                    let status: String = Connection::open(library.data_dir().join("library.db"))?
                        .query_row(
                        "SELECT status FROM rekordbox_sync_operations WHERE id=?1",
                        [id],
                        |r| r.get(0),
                    )?;
                    if status == "native_published" {
                        injected.set(true);
                        anyhow::bail!("Authored interruption after native publication");
                    }
                    Ok(())
                });
                ensure!(
                    result.is_err()
                        && injected.get()
                        && load(&library, id)?.status == "native_published",
                    "Authored interruption did not reach the recoverable native boundary"
                );
                require_unowned_native_preserved(
                    &unowned_before,
                    case,
                    &library,
                    &operation,
                    &key,
                    &cancel,
                )?;
                summary["operation_id"] = json!(id);
                require_frozen_native_state(case, &key, &cancel, &operation.after)?;
                summary["status"] = json!("interrupted_native_published");
                return Ok(summary);
            }
            _ => unreachable!(),
        };
        require_unowned_native_preserved(
            &unowned_before,
            case,
            &library,
            &operation,
            &key,
            &cancel,
        )?;
        let expected_state = if result.status == "restored" || result.status == "rolled_back" {
            &operation.before
        } else {
            &operation.after
        };
        require_frozen_native_state(case, &key, &cancel, expected_state)?;
        summary["unowned_native_preserved"] = json!(true);
        summary["operation_id"] = json!(result.id);
        summary["status"] = json!(result.status);
        summary["changed_rows"] = json!(result.changed_rows);
        summary["acknowledged_fields"] = json!(result.acknowledged_fields);
        if result.status == "applied" {
            summary["readback"] = verify_native_publication(case, &library, id, &key, &cancel)?;
        }
    }
    Ok(summary)
}

#[test]
#[ignore = "PRIVATE OPT-IN: requires an isolated application-created library, sentinel, signed executable, and closed rekordbox/agent"]
fn private_native_acceptance() {
    // All error details go only into the already authorized private case report.
    // Never print native paths, account options, library rows, or runtime keys.
    let case =
        Case::load().unwrap_or_else(|_| panic!("Private native acceptance configuration refused"));
    let phase = std::env::var("CUERITHM_NATIVE_ACCEPTANCE_PHASE").unwrap_or_default();
    match run_phase(&case, &phase) {
        Ok(summary) => {
            write_private(&case.work.join(format!("report-{phase}.json")), &summary)
                .expect("Write private native acceptance report");
            println!(
                "{}",
                serde_json::to_string(&summary).expect("Serialize native acceptance aggregates")
            );
        }
        Err(error) => {
            // SDK/key-provider diagnostics already redact key material. The
            // report stays private, and stdout contains only an aggregate state.
            let report = json!({"phase":phase,"status":"failed","error":format!("{error:#}"),"production_write_enabled":false});
            let report_name = if matches!(
                phase.as_str(),
                "review"
                    | "preview"
                    | "apply"
                    | "noop"
                    | "restore"
                    | "refuse-restore"
                    | "public-noop"
                    | "interrupt"
                    | "recover"
                    | "edit-local"
                    | "verify"
            ) {
                format!("report-{phase}.json")
            } else {
                "report-refused.json".into()
            };
            write_private(&case.work.join(report_name), &report)
                .expect("Write private native acceptance failure report");
            panic!("Private native acceptance phase failed; inspect its private report");
        }
    }
}

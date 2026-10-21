//! Explicitly opted-in, private native application acceptance. Never run in CI.
//!
//! The caller supplies a disposable, application-created native library and
//! authored media. This module never creates or changes its native SQL schema,
//! edits global settings, or grants a production write capability. Native UI
//! reopen, playback and visual inspection remain separate manual evidence.
use super::super::*;
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
}

struct Case {
    root: PathBuf,
    work: PathBuf,
    source: PathBuf,
    second_source: Option<PathBuf>,
    schema_fingerprint: String,
    target: RekordboxSyncTarget,
}

#[derive(Default, Serialize, Deserialize)]
struct CaseState {
    operation_id: Option<String>,
    noop_id: Option<String>,
    #[serde(default)]
    earlier_operation_ids: Vec<String>,
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
        let installation = TargetInstallation {
            root: app,
            executable,
            version,
            major: sentinel.application_major,
            version_source: "private-native-acceptance-plist".into(),
        };
        let id = digest(&json!([&installation, &database, &sentinel.agent_options]))?;
        let target = RekordboxSyncTarget {
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
    RekordboxSyncRequest {
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
    }
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
    let native_id = |local: &str| -> Result<&str> {
        op.receipts
            .iter()
            .find(|receipt| receipt.local_id == local)
            .map(|receipt| receipt.native_id.as_str())
            .context("Authored entity receipt is missing")
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
                    ensure!(
                        actual.parent_id.as_deref() == Some(native_id(parent)?),
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
        ensure!(
            native
                .cues
                .iter()
                .any(|cue| resolved.owned_cue_ids.contains(&cue.id)
                    && cue.end_seconds.is_some()
                    && cue.active_loop),
            "Authored active loop did not survive native readback"
        );
        let analysis = native
            .source
            .analysis_path
            .as_deref()
            .context("Authored native analysis reference is missing")?;
        ensure!(
            assets::grid_observation(&case.target, analysis)?
                .as_array()
                .is_some_and(|beats| beats.len() > 1),
            "Authored native grid did not survive readback"
        );
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

fn run_phase(case: &Case, phase: &str) -> Result<Value> {
    ensure!(
        matches!(
            phase,
            "review"
                | "preview"
                | "apply"
                | "noop"
                | "restore"
                | "interrupt"
                | "recover"
                | "edit-local"
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
        !accepted_write_capability(&case.target.installation, &schema)?.can_write(),
        "Private fixture acceptance must not rely on a production promotion"
    );
    let collection = database.read_collection(Default::default(), &cancel)?;
    let native_paths: BTreeSet<_> = collection
        .tracks
        .iter()
        .map(|track| track.source.resolved_path.as_deref().map(Path::new))
        .collect();
    let allowed_paths: BTreeSet<_> = case
        .sources()
        .into_iter()
        .map(|(_, path)| Some(path))
        .collect();
    ensure!(
        native_paths.contains(&Some(case.source.as_path()))
            && native_paths.is_subset(&allowed_paths)
            && native_paths.len() == collection.tracks.len(),
        "Native case contains a missing, duplicate, or unrelated authored source"
    );
    let creating_second = case
        .second_source
        .as_ref()
        .is_some_and(|path| !native_paths.contains(&Some(path.as_path())));
    drop(database);
    let library = Library::open(&case.work.join("library.db"))?;
    let _lock = lock(&library)?;
    let mut state = case.state()?;
    let mut summary = json!({"phase":phase,"native_major":case.target.installation.major,"tables":schema.objects.iter().filter(|o|o.kind=="table").count(),"schema_objects":schema.objects.len(),"production_write_enabled":false});
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
            state.noop_id = Some(preview.id.clone());
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
            state.operation_id.as_deref() == Some(id)
                || state.earlier_operation_ids.iter().any(|saved| saved == id),
            "Operation is outside this private acceptance history"
        );
        case.verify_operation(&library, id)?;
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
                summary["operation_id"] = json!(id);
                summary["status"] = json!("interrupted_native_published");
                return Ok(summary);
            }
            _ => unreachable!(),
        };
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
                    | "interrupt"
                    | "recover"
                    | "edit-local"
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

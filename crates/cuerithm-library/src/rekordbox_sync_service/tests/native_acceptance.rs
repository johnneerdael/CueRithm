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
    schema_fingerprint: String,
    target: RekordboxSyncTarget,
}

#[derive(Default, Serialize, Deserialize)]
struct CaseState {
    operation_id: Option<String>,
    noop_id: Option<String>,
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
                .all(|s| s.id == TRACK && s.path == self.source),
            "Persisted operation contains non-authored sources"
        );
        Ok(op)
    }
}

fn initialize_local(
    case: &Case,
    library: &Library,
    runtime: &cuerithm_media::MediaRuntime,
    cancel: &CancellationToken,
) -> Result<()> {
    let existing: Option<String> = library
        .conn()?
        .query_row("SELECT source_hash FROM tracks WHERE id=?1", [TRACK], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(hash) = existing {
        ensure!(
            hash == file_sha256_cancellable(&case.source, cancel)?,
            "Authored source changed after fixture preparation"
        );
        return Ok(());
    }
    ensure!(
        library
            .conn()?
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get::<_, u64>(0))?
            == 0,
        "Private harness library contains unrelated tracks"
    );
    let audio = runtime.decode_cancellable(&case.source, &case.work.join("decode"), cancel)?;
    ensure!(
        audio.clock.duration_seconds() > 5.,
        "Authored WAV must contain more than five playable seconds"
    );
    let publication = json!({
        "grid":{"bpm":120.0,"startTime":0.0},
        "cues":[
            {"name":"Authored cue","startTime":1.0,"type":"1","position":0,"color":null},
            {"name":"Authored four-beat loop","startTime":2.0,"endTime":4.0,"activeLoop":true,"type":"5","position":1,"color":null}
        ],
        "implementation_sha256":digest(&"private-authored-manual-publication-v1")?,
        "inputs_sha256":digest(&json!([&audio.source_sha256, 120, 0, 1, 2, 4]))?
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
    library.conn()?.execute("INSERT INTO tracks(id,path,title,artist,source_hash,state,analysis_json,metadata_json,musical_key,bpm,duration,created,updated,sort_title,sort_artist) VALUES(?1,?2,'CueRithm authored synchronized','CueRithm acceptance',?3,'ready',?4,?5,'8A',120,?6,1,1,'cuerithm authored synchronized','cuerithm acceptance')", params![TRACK, crate::model::path_bytes(&case.source), audio.source_sha256, analysis.to_string(), serde_json::to_string(&audio.info)?, audio.clock.duration_seconds()])?;
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
            track_ids: vec![TRACK.into()],
        },
        library.playlist_state()?.revision,
    )?;
    library.conn()?.execute_batch("INSERT INTO custom_tag_categories(id,name) VALUES('authored-category','CueRithm authored'); INSERT INTO custom_tags(id,category_id,name) VALUES('authored-tag','authored-category','Acceptance'); INSERT INTO track_custom_tags(track_id,tag_id) VALUES('authored-native-acceptance','authored-tag');")?;
    library.refresh_smart_lists(crate::model::now())?;
    let projected = library.dj_sync_library(DjSyncScope::Full)?;
    ensure!(
        projected
            .playlists
            .iter()
            .find(|p| p.id == SMART)
            .is_some_and(|p| p.track_ids == [TRACK]),
        "Authored smart fixture did not materialize its whole membership"
    );
    write_private(
        &case.work.join("fixture-proof.json"),
        &json!({"authored_manual_publication":true,"engine_accuracy_claim":false,"source_sha256":audio.source_sha256,"pcm_sha256":audio.pcm_sha256_cancellable(cancel)?,"frames":audio.frames,"clock":audio.clock,"publication":publication,"limitations":["single-track membership; multi-track ordering untested","WAV has no authored embedded artwork; keep-target decision only","variable grid acceptance requires a separate fixture"]}),
    )?;
    Ok(())
}

fn request(target: &RekordboxSyncTarget) -> RekordboxSyncRequest {
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
        target_id: target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options,
        decisions: BTreeMap::from([(
            TRACK.into(),
            [
                ("title", NativeFieldDecision::UseLocal),
                ("artist", NativeFieldDecision::UseLocal),
                ("source_path", NativeFieldDecision::AcceptEqual),
                ("cues", NativeFieldDecision::UseLocal),
                ("beat_grid", NativeFieldDecision::UseLocal),
                ("artwork", NativeFieldDecision::UseRemote),
            ]
            .into_iter()
            .map(|(f, d)| (f.into(), d))
            .collect(),
        )]),
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

fn run_phase(case: &Case, phase: &str) -> Result<Value> {
    ensure!(
        matches!(
            phase,
            "preview" | "apply" | "noop" | "restore" | "interrupt" | "recover"
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
    ensure!(
        collection.tracks.len() == 1
            && collection.tracks[0]
                .source
                .resolved_path
                .as_deref()
                .map(Path::new)
                == Some(case.source.as_path()),
        "Native case must contain exactly the authored imported track"
    );
    drop(database);
    let library = Library::open(&case.work.join("library.db"))?;
    let _lock = lock(&library)?;
    let mut state = case.state()?;
    let mut summary = json!({"phase":phase,"native_major":case.target.installation.major,"tables":schema.objects.iter().filter(|o|o.kind=="table").count(),"schema_objects":schema.objects.len(),"production_write_enabled":false});
    if phase == "preview" || phase == "noop" {
        let runtime_root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(PathBuf::from)
            .context("Explicit bundled media runtime is required")?;
        let runtime = cuerithm_media::MediaRuntime::from_bundle(&runtime_root)?;
        initialize_local(case, &library, &runtime, &cancel)?;
        ensure_settled(&library, &case.target.database)?;
        let before = native_proof(case, &cancel)?;
        let preview = planning::preview(
            &library,
            request(&case.target),
            case.target.clone(),
            &key,
            Some(&runtime),
            &cancel,
            planning::PreviewPermit::AuthoredFixture,
        )?;
        let op = case.verify_operation(&library, &preview.id)?;
        ensure!(
            op.resolved.len() == 1 && !op.resolved[0].created,
            "Acceptance must match the actual imported native track"
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
        } else {
            ensure!(
                state.operation_id.is_none(),
                "This private case already has its primary acceptance operation"
            );
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
        let id = state
            .operation_id
            .as_deref()
            .context("Run the private preview phase first")?;
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
                "preview" | "apply" | "noop" | "restore" | "interrupt" | "recover"
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

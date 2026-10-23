use anyhow::{Context, Result, ensure};
use cuerithm_library::{
    CurationService, CurationWorker, Library, PreparationService, PreparationSettings,
    TrackSummary, playback::Player,
};
use cuerithm_media::{MediaRuntime, PreparedAudio};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

#[derive(Serialize)]
pub struct Detail {
    pub track: TrackSummary,
    pub analysis: Option<camelot_core::FileAnalysis>,
    pub source_sha256: String,
    pub pcm_sha256: Option<String>,
    pub musical: Option<cuerithm_library::AcceptedMusicalData>,
    pub source_locator: Option<cuerithm_library::PortableLocator>,
    /// False for unlinked tracks or a file that is definitely gone.
    pub file_available: bool,
}
/// Shown wherever a track's file cannot be found.
pub const FILE_NOT_FOUND: &str = "File not found - move it back or run Maintenance";

pub struct Host {
    pub exit: crate::exit::ExitCoordinator,
    pub portable: crate::portable_commands::PortableState,
    pub libraries: cuerithm_library::LibraryGenerations,
    pub restart_requested: AtomicBool,
    pub maintenance: Arc<crate::maintenance::MaintenanceTasks>,
    pub builtin: crate::builtin::BuiltinModels,
    pub library: Library,
    pub runtime: MediaRuntime,
    pub preparation: PreparationService,
    pub import_processing: cuerithm_library::ImportProcessingService,
    pub import_selections: crate::import_commands::ImportSelections,
    pub provider_setup: cuerithm_library::ProviderSetupService,
    pub smart_playlists: cuerithm_library::SmartPlaylistService,
    pub curation: CurationService,
    pub curation_worker: Mutex<CurationWorker>,
    pub player: Mutex<Player>,
    pub loaded: Mutex<Option<(String, Arc<PreparedAudio>)>>,
    artwork_gate: Mutex<()>,
    metadata_gate: Mutex<()>,
    pub generation: AtomicU64,
    pub alive: AtomicBool,
}
impl Host {
    #[cfg(test)]
    pub fn open(root: &Path, runtime: &Path, initial_folder: Option<PathBuf>) -> Result<Self> {
        Self::open_with_provider_factory(root, runtime, initial_folder, |library| {
            library
                .provider_runtime_with(Arc::new(cuerithm_autotag::CatalogRuntime::development()?))
        })
    }
    pub fn open_with_release(
        root: &Path,
        runtime: &Path,
        initial_folder: Option<PathBuf>,
        release: &str,
    ) -> Result<Self> {
        Self::open_with_provider_factory(root, runtime, initial_folder, |library| {
            library.provider_runtime(release)
        })
    }
    fn open_with_provider_factory(
        root: &Path,
        runtime: &Path,
        initial_folder: Option<PathBuf>,
        provider_factory: impl FnOnce(&Library) -> Result<cuerithm_library::ProviderSetupService>,
    ) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let libraries = cuerithm_library::LibraryGenerations::new(root)?;
        let is_new =
            !root.join("library.sqlite").exists() && !root.join("active-library.json").exists();
        let library = libraries.open_active()?;
        if is_new {
            if let Some(path) = std::env::var_os("CUERITHM_BOOTSTRAP_SETTINGS") {
                let settings: PreparationSettings = serde_json::from_slice(&std::fs::read(path)?)?;
                library.save_settings(settings)?;
            } else if let Some(folder) = initial_folder {
                std::fs::create_dir_all(&folder)?;
                let mut settings = library.settings()?.settings;
                settings.watch_folders = vec![folder];
                library.save_settings(settings)?;
            }
        }
        let cache = root.join("cache/media");
        if cache.is_dir() {
            for entry in std::fs::read_dir(&cache)? {
                let entry = entry?;
                if entry.file_name().to_string_lossy().starts_with("cuerithm-")
                    && entry.file_type()?.is_dir()
                {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
        let builtin = crate::builtin::BuiltinModels::install(root)?;
        builtin.configure_library(&library)?;
        let runtime = MediaRuntime::from_bundle(runtime)?;
        let provider_setup = provider_factory(&library)?;
        let preparation = PreparationService::start_with_catalog_runtime(
            library.clone(),
            runtime.clone(),
            provider_setup.runtime().clone(),
        )?;
        let import_processing =
            cuerithm_library::ImportProcessingService::start(library.clone(), runtime.clone())?;
        let smart_playlists = cuerithm_library::SmartPlaylistService::start(library.clone())?;
        let curation = CurationService::new(library.clone(), Some(runtime.clone()));
        let curation_worker = Mutex::new(curation.start_background()?);
        Ok(Self {
            exit: crate::exit::ExitCoordinator::default(),
            portable: crate::portable_commands::PortableState::default(),
            libraries,
            restart_requested: AtomicBool::new(false),
            maintenance: Arc::new(crate::maintenance::MaintenanceTasks::default()),
            builtin,
            library,
            runtime,
            preparation,
            import_processing,
            import_selections: crate::import_commands::ImportSelections::default(),
            provider_setup,
            smart_playlists,
            curation,
            curation_worker,
            player: Mutex::new(Player::new()?),
            loaded: Mutex::new(None),
            artwork_gate: Mutex::new(()),
            metadata_gate: Mutex::new(()),
            generation: AtomicU64::new(0),
            alive: AtomicBool::new(true),
        })
    }
    pub fn detail(&self, id: &str) -> Result<Detail> {
        let analysis = self.library.analysis(id)?;
        let pcm_sha256 = analysis
            .as_ref()
            .and_then(|a| a.media.as_ref())
            .map(|m| m.pcm_sha256.clone())
            .filter(|s| !s.is_empty());
        Ok(Detail {
            track: self.library.track(id)?,
            analysis,
            source_sha256: self.library.source_hash_optional(id)?.unwrap_or_default(),
            musical: self.library.accepted_musical_data(id)?,
            source_locator: self.library.source_locator(id)?,
            pcm_sha256,
            file_available: !self.library.track_file_missing(id).unwrap_or(false),
        })
    }
    pub fn metadata_fields(&self) -> Result<Vec<String>> {
        let _gate = self
            .metadata_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Metadata refresh interrupted"))?;
        for id in self.library.metadata_refresh_candidates()? {
            if !self.alive.load(Ordering::Relaxed) {
                break;
            }
            // Offline or changed sources keep their last confirmed metadata.
            let _ = (|| -> Result<()> {
                let hash = self.library.source_hash(&id)?;
                let path = self.library.track_path(&id)?;
                ensure!(
                    cuerithm_media::file_sha256(&path)? == hash,
                    "Source changed"
                );
                let info = self.runtime.probe(&path)?;
                ensure!(
                    cuerithm_media::file_sha256(&path)? == hash,
                    "Source changed"
                );
                self.library.update_legacy_metadata(&id, &hash, &info)?;
                Ok(())
            })();
        }
        self.library.metadata_fields()
    }
    pub fn artwork(&self, id: &str, identity: &str) -> Result<Option<String>> {
        use base64::Engine as _;
        use std::io::Write;
        ensure!(
            identity.len() == 64 && identity.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid artwork identity"
        );
        let audio = self
            .loaded
            .lock()
            .map_err(|_| anyhow::anyhow!("Playback cache interrupted"))?
            .as_ref()
            .filter(|(track, a)| track == id && a.source_sha256 == identity)
            .map(|(_, a)| a.clone());
        if audio.is_none() && self.library.source_hash(id)? != identity {
            return Ok(None);
        }
        let _gate = self
            .artwork_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Artwork reader interrupted"))?;
        let folder = self.library.data_dir().join("cache/artwork");
        std::fs::create_dir_all(&folder)?;
        let cached = folder.join(identity);
        fn url(bytes: &[u8]) -> Option<String> {
            cuerithm_media::artwork_mime(bytes).map(|mime| {
                format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                )
            })
        }
        if let Ok(meta) = std::fs::metadata(&cached)
            && meta.len() <= 8 * 1024 * 1024
        {
            let bytes = std::fs::read(&cached)?;
            if bytes.is_empty() {
                return Ok(None);
            }
            if let Some(value) = url(&bytes) {
                return Ok(Some(value));
            }
        }
        let path = if let Some(ref audio) = audio {
            audio.snapshot_path.clone()
        } else {
            self.library.track_path(id)?
        };
        let art = self.runtime.artwork(&path)?;
        if audio.is_none() && cuerithm_media::file_sha256(&path)? != identity {
            return Ok(None);
        }
        let bytes = art.map(|a| a.bytes).unwrap_or_default();
        let mut temp = tempfile::NamedTempFile::new_in(&folder)?;
        temp.write_all(&bytes)?;
        temp.persist(&cached).map_err(|e| e.error)?;
        Ok(url(&bytes))
    }
    pub fn audio(&self, id: &str) -> Result<Arc<PreparedAudio>> {
        let loaded = self
            .loaded
            .lock()
            .map_err(|_| anyhow::anyhow!("Playback cache interrupted"))?;
        if let Some((track, audio)) = &*loaded
            && track == id
        {
            return Ok(audio.clone());
        }
        drop(loaded);
        Ok(Arc::new(self.runtime.decode(
            &self.source_path(id)?,
            &self.library.data_dir().join("cache/media"),
        )?))
    }
    /// The track's file, or a plain statement that it is not there.
    fn source_path(&self, id: &str) -> Result<PathBuf> {
        ensure!(!self.library.track_file_missing(id)?, FILE_NOT_FOUND);
        self.library.track_path(id)
    }
    pub fn load(&self, id: String, generation: u64) -> Result<Detail> {
        let audio = Arc::new(self.runtime.decode(
            &self.source_path(&id)?,
            &self.library.data_dir().join("cache/media"),
        )?);
        let mut detail = self.detail(&id)?;
        let pcm = audio.pcm_sha256()?;
        if detail.pcm_sha256.as_ref().is_some_and(|p| p != &pcm) {
            detail.analysis = None;
        }
        detail.source_sha256 = audio.source_sha256.clone();
        detail.pcm_sha256 = Some(pcm);
        let mut player = self
            .player
            .lock()
            .map_err(|_| anyhow::anyhow!("Player interrupted"))?;
        ensure!(
            self.generation.load(Ordering::Relaxed) == generation,
            "Playback load superseded"
        );
        player.load(id.clone(), audio.clone())?;
        *self
            .loaded
            .lock()
            .map_err(|_| anyhow::anyhow!("Playback cache interrupted"))? = Some((id, audio));
        Ok(detail)
    }
    pub fn begin_load(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }
    pub fn waveform(&self, id: &str, bins: usize) -> Result<cuerithm_library::waveform::Waveform> {
        ensure!(bins > 0 && bins <= 16384, "Invalid waveform resolution");
        let audio = self.audio(id)?;
        cuerithm_library::waveform::cached_waveform(
            audio.as_ref(),
            bins,
            &self.library.data_dir().join("cache/waveforms"),
        )
        .context("Load waveform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_only_media_loads_without_native_analysis_or_locator() {
        use cuerithm_dj_sync::import::{ArtifactRequest, FieldGroup, ImportPolicy, SourceProvider};
        use cuerithm_library::{
            ImportInspectRequest, ImportPreviewRequest, ImportSourceSelection, SourceBindingChoice,
            TrackQuery,
        };
        use cuerithm_media::CancellationToken;

        let temporary_root = std::env::temp_dir().canonicalize().unwrap();
        let temp = tempfile::tempdir_in(temporary_root).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let host = Host::open(
            &temp.path().join("app"),
            &root.join("tools/media-runtime/bin"),
            None,
        )
        .unwrap();
        let source = temp.path().join("imported.flac");
        std::fs::copy(
            root.join("crates/cuerithm-media/tests/fixtures/covered.flac"),
            &source,
        )
        .unwrap();
        let import_root = temp.path().join("VirtualDJ");
        std::fs::create_dir(&import_root).unwrap();
        let xml_path = source
            .to_string_lossy()
            .replace('&', "&amp;")
            .replace('"', "&quot;");
        let unavailable_xml_path = temp
            .path()
            .join("unavailable.flac")
            .to_string_lossy()
            .replace('&', "&amp;")
            .replace('"', "&quot;");
        std::fs::write(
            import_root.join("database.xml"),
            format!(r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{xml_path}"><Tags Title="Imported track"/></Song><Song FilePath="{unavailable_xml_path}"><Tags Title="Unavailable track"/></Song></VirtualDJ_Database>"#),
        )
        .unwrap();
        let cancel = CancellationToken::default();
        let inspection = host
            .library
            .inspect_import(
                ImportInspectRequest {
                    provider: SourceProvider::VirtualDj,
                    selection: ImportSourceSelection {
                        root: import_root,
                        artifacts: vec![ArtifactRequest::stable("database.xml")],
                    },
                    binding: SourceBindingChoice::Automatic,
                    origin: None,
                    path_mappings: vec![],
                    capture_file_tags: false,
                    include_history: false,
                    include_queue: false,
                },
                &cancel,
            )
            .unwrap();
        let preview = host
            .library
            .preview_import(
                ImportPreviewRequest {
                    inspection_id: inspection.inspection_id,
                    policy: ImportPolicy {
                        selected_fields: [FieldGroup::DescriptiveTags].into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap();
        assert_eq!(preview.blocked, 0);
        host.library
            .apply_import(&preview.preview_id, &cancel)
            .unwrap();
        let mut tracks = host.library.tracks(TrackQuery::default()).unwrap().rows;
        assert_eq!(tracks.len(), 2);
        let unavailable = tracks.remove(
            tracks
                .iter()
                .position(|track| track.title == "Unavailable track")
                .unwrap(),
        );
        assert!(!unavailable.import_state.unwrap().playback_allowed);
        assert_eq!(
            host.library.source_hash_optional(&unavailable.id).unwrap(),
            None
        );
        assert!(
            host.library
                .source_locator(&unavailable.id)
                .unwrap()
                .is_none()
        );
        assert!(
            host.library
                .accepted_musical_data(&unavailable.id)
                .unwrap()
                .is_none()
        );
        let unavailable_detail = host.detail(&unavailable.id).unwrap();
        assert!(!unavailable_detail.file_available);
        assert!(unavailable_detail.analysis.is_none());
        assert!(unavailable_detail.musical.is_none());
        assert!(unavailable_detail.source_sha256.is_empty());
        assert_eq!(
            host.load(unavailable.id, host.begin_load())
                .err()
                .unwrap()
                .to_string(),
            FILE_NOT_FOUND
        );
        let track = tracks.remove(0);
        let id = &track.id;
        assert!(track.import_state.unwrap().playback_allowed);
        assert!(host.library.jobs().unwrap().is_empty());
        let file_hash = cuerithm_media::file_sha256(&source).unwrap();
        assert_eq!(
            host.library.source_hash_optional(id).unwrap(),
            Some(file_hash.clone())
        );
        assert!(host.library.source_locator(id).unwrap().is_none());
        assert!(host.library.accepted_musical_data(id).unwrap().is_none());
        assert!(host.library.source_hash_optional("unknown-track").is_err());
        assert!(host.library.source_locator("unknown-track").is_err());
        assert!(host.library.accepted_musical_data("unknown-track").is_err());
        let detail = host.detail(id).unwrap();
        assert!(detail.file_available);
        assert!(detail.analysis.is_none());
        assert!(detail.musical.is_none());
        assert!(detail.source_locator.is_none());
        assert_eq!(detail.source_sha256, file_hash);
        assert!(detail.pcm_sha256.is_none());

        let loaded = host.load(id.clone(), host.begin_load()).unwrap();
        assert_eq!(loaded.source_sha256, file_hash);
        assert!(loaded.pcm_sha256.is_some());
        assert!(loaded.analysis.is_none());
        assert!(loaded.musical.is_none());
        assert!(loaded.source_locator.is_none());
        assert_eq!(
            host.player.lock().unwrap().status().track_id.as_deref(),
            Some(id.as_str())
        );
        assert!(host.library.analysis(id).unwrap().is_none());
        assert_eq!(host.library.cue_revision_count(id).unwrap(), 0);

        std::fs::remove_file(&source).unwrap();
        assert!(!host.detail(id).unwrap().file_available);
        assert_eq!(
            host.load(id.clone(), host.begin_load())
                .err()
                .unwrap()
                .to_string(),
            FILE_NOT_FOUND
        );
        host.preparation.shutdown();
        host.smart_playlists.shutdown();
        host.curation_worker.lock().unwrap().shutdown();
    }
    #[test]
    fn host_startup_recovers_a_torn_active_pointer_before_looking_up_its_path() {
        let temp = tempfile::tempdir().unwrap();
        let manager = cuerithm_library::LibraryGenerations::new(temp.path()).unwrap();
        drop(Library::open(&temp.path().join("library.sqlite")).unwrap());
        let stage = manager.create_staging().unwrap();
        drop(Library::open(&stage.database).unwrap());
        let seal = manager.seal(&stage.id).unwrap();
        manager.schedule(&stage.id, "legacy", &seal).unwrap();
        drop(manager.open_active().unwrap());
        std::fs::write(temp.path().join("active-library.json"), b"{torn").unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let host = Host::open(temp.path(), &root.join("tools/media-runtime/bin"), None).unwrap();
        assert_eq!(host.libraries.active_id().unwrap(), "legacy");
        assert_eq!(host.library.counts().unwrap().tracks, 0);
        host.preparation.shutdown();
        host.smart_playlists.shutdown();
        host.curation_worker.lock().unwrap().shutdown();
    }
    #[test]
    fn native_artwork_is_source_scoped_and_cached() {
        let d = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let host = Host::open(d.path(), &root.join("tools/media-runtime/bin"), None).unwrap();
        let source = root.join("crates/cuerithm-media/tests/fixtures/covered.flac");
        host.library
            .enqueue(&[source], host.library.settings().unwrap().revision)
            .unwrap();
        let id = host.library.jobs().unwrap()[0].track_id.clone();
        let identity = host.library.source_hash(&id).unwrap();
        let image = host.artwork(&id, &identity).unwrap().unwrap();
        assert!(image.starts_with("data:image/png;base64,"));
        assert_eq!(host.artwork(&id, &identity).unwrap().as_ref(), Some(&image));
        assert!(host.artwork(&id, &"0".repeat(64)).unwrap().is_none());
        host.preparation.shutdown();
    }
    #[test]
    #[ignore = "local throughput benchmark; set CUERITHM_BENCH_AUDIO"]
    fn benchmark_one_vs_two_workers_preserves_analysis_results() {
        let source =
            PathBuf::from(std::env::var_os("CUERITHM_BENCH_AUDIO").expect("Set benchmark source"));
        let runtime =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/media-runtime/bin");
        let mut reference = None;
        for workers in [1, 2] {
            let d = tempfile::tempdir().unwrap();
            let root = d.path().join("app");
            std::fs::create_dir_all(&root).unwrap();
            let lib = Library::open(&root.join("db")).unwrap();
            crate::builtin::BuiltinModels::install(&root)
                .unwrap()
                .configure_library(&lib)
                .unwrap();
            let mut paths = Vec::new();
            for index in 0..2 {
                let path = d.path().join(format!(
                    "{index}.{}",
                    source.extension().unwrap().to_string_lossy()
                ));
                std::fs::copy(&source, &path).unwrap();
                paths.push(path);
            }
            let service = PreparationService::start_with_workers(
                lib.clone(),
                MediaRuntime::from_bundle(&runtime).unwrap(),
                workers,
            )
            .unwrap();
            let start = std::time::Instant::now();
            service.enqueue_paths(&paths).unwrap();
            while lib.counts().unwrap().tracks != 2 {
                assert!(
                    start.elapsed() < std::time::Duration::from_secs(300),
                    "{:?}",
                    lib.jobs().unwrap()
                );
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            println!(
                "{workers} workers: {:.3}s for two tracks",
                start.elapsed().as_secs_f64()
            );
            service.shutdown();
            for job in lib.jobs().unwrap() {
                let value =
                    serde_json::to_value(lib.analysis(&job.track_id).unwrap().unwrap().analysis)
                        .unwrap();
                let values = serde_json::json!([
                    value["key"],
                    value["tempo"],
                    value["energy"],
                    value["lexicon_cues"]
                ]);
                if let Some(ref expected) = reference {
                    assert_eq!(&values, expected);
                } else {
                    reference = Some(values);
                }
            }
        }
    }
    #[test]
    fn desktop_prepares_audio_without_user_model_configuration() {
        let d = tempfile::tempdir().unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let host = Host::open(d.path(), &root.join("tools/media-runtime/bin"), None).unwrap();
        host.library.settings().unwrap().settings.ready().unwrap();
        host.preparation
            .enqueue_paths(&[
                root.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac")
            ])
            .unwrap();
        let start = std::time::Instant::now();
        while host.library.counts().unwrap().tracks != 1 {
            let jobs = host.library.jobs().unwrap();
            assert!(!jobs.iter().any(|j| j.state == "attention"), "{jobs:?}");
            assert!(start.elapsed() < std::time::Duration::from_secs(180));
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let id = host.library.jobs().unwrap()[0].track_id.clone();
        let analysis = host.library.analysis(&id).unwrap().unwrap();
        assert_eq!(analysis.openkeyscan_sample_rate, Some(44100));
        assert_eq!(
            analysis.analysis.tempo.as_ref().unwrap().method,
            camelot_core::lexicon_timing::METHOD
        );
        assert!(
            analysis
                .analysis
                .engine
                .contains("+lexicon-cue-webaudio-v1")
        );
        assert_eq!(
            analysis.analysis.energy.as_ref().unwrap().method,
            camelot_core::lexicon_energy::METHOD
        );
        assert!(
            !analysis
                .analysis
                .lexicon_cues
                .as_ref()
                .unwrap()
                .cues
                .is_empty()
        );
        host.preparation.shutdown();
        let previous = serde_json::to_value(&analysis).unwrap();
        let mut old_settings = host.library.settings().unwrap().settings;
        old_settings.engine.lexicon_timing = false;
        old_settings.engine.source_profile_timing = true;
        host.library.save_settings(old_settings).unwrap();
        assert!(host.builtin.configure_library(&host.library).unwrap());
        assert_eq!(
            serde_json::to_value(host.library.analysis(&id).unwrap().unwrap()).unwrap(),
            previous,
            "Default migration must preserve completed results"
        );
        assert_eq!(host.library.counts().unwrap().tracks, 1);
        assert_eq!(host.library.jobs().unwrap()[0].state, "completed");
    }
}

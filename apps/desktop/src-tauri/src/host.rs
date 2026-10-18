use anyhow::{Context, Result, ensure};
use cuerithm_library::{
    Library, PreparationService, PreparationSettings, TrackSummary, playback::Player,
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
}
pub struct Host {
    pub library: Library,
    pub runtime: MediaRuntime,
    pub preparation: PreparationService,
    pub player: Mutex<Player>,
    pub loaded: Mutex<Option<(String, Arc<PreparedAudio>)>>,
    pub generation: AtomicU64,
    pub alive: AtomicBool,
}
impl Host {
    pub fn open(root: &Path, runtime: &Path, initial_folder: Option<PathBuf>) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        let path = root.join("library.sqlite");
        let is_new = !path.exists();
        let library = Library::open(&path)?;
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
        let runtime = MediaRuntime::from_bundle(runtime)?;
        let preparation = PreparationService::start(library.clone(), runtime.clone())?;
        Ok(Self {
            library,
            runtime,
            preparation,
            player: Mutex::new(Player::new()?),
            loaded: Mutex::new(None),
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
            source_sha256: self.library.source_hash(id)?,
            pcm_sha256,
        })
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
            &self.library.track_path(id)?,
            &self.library.data_dir().join("cache/media"),
        )?))
    }
    pub fn load(&self, id: String, generation: u64) -> Result<Detail> {
        let audio = Arc::new(self.runtime.decode(
            &self.library.track_path(&id)?,
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

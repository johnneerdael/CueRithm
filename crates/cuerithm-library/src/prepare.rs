use crate::*;
use anyhow::{Context, Result, ensure};
use camelot_core::Engine;
#[cfg(feature = "autotag")]
use cuerithm_autotag::{AnalysisTags, TagInput, TagWritePlan, Tagger};
use cuerithm_media::{MediaRuntime, file_sha256};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationEvent {
    #[serde(default)]
    pub attempt: u32,
    pub sequence: u64,
    pub job_id: JobId,
    pub track_id: TrackId,
    pub stage: String,
    pub state: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub watch_errors: Vec<String>,
    pub last_error: Option<String>,
}
enum Control {
    Wake,
    Changed(Vec<PathBuf>),
}
type Subscribers = Arc<Mutex<Vec<SyncSender<PreparationEvent>>>>;
pub(crate) fn worker_count(available_cores: usize) -> usize {
    available_cores
        .saturating_sub(available_cores.div_ceil(4))
        .max(1)
}
pub struct PreparationService {
    library: Library,
    tx: SyncSender<Control>,
    stop: Arc<AtomicBool>,
    join: Mutex<Vec<JoinHandle<()>>>,
    subscribers: Subscribers,
    status: Arc<Mutex<ServiceStatus>>,
}
impl PreparationService {
    pub fn start(library: Library, runtime: MediaRuntime) -> Result<Self> {
        let count = worker_count(thread::available_parallelism().map_or(1, |n| n.get()));
        Self::start_with_workers(library, runtime, count)
    }
    pub fn start_with_workers(
        library: Library,
        runtime: MediaRuntime,
        count: usize,
    ) -> Result<Self> {
        Self::start_inner(
            library,
            runtime,
            count,
            #[cfg(feature = "autotag")]
            Arc::new(Tagger::new()?),
        )
    }
    #[cfg(feature = "autotag")]
    pub fn start_with_tagger(
        library: Library,
        runtime: MediaRuntime,
        count: usize,
        tagger: Arc<Tagger>,
    ) -> Result<Self> {
        Self::start_inner(library, runtime, count, tagger)
    }
    fn start_inner(
        library: Library,
        runtime: MediaRuntime,
        count: usize,
        #[cfg(feature = "autotag")] tagger: Arc<Tagger>,
    ) -> Result<Self> {
        ensure!(count > 0, "Preparation needs at least one worker");
        let (tx, rx) = mpsc::sync_channel(1024);
        let stop = Arc::new(AtomicBool::new(false));
        let subscribers = Arc::new(Mutex::new(Vec::new()));
        let status = Arc::new(Mutex::new(ServiceStatus::default()));
        let sequence = Arc::new(AtomicU64::new(0));
        #[cfg(feature = "autotag")]
        let album_groups = Arc::new(crate::album_context::AlbumGroups::default());
        // Invalidate persisted DSP caches on any executable/algorithm upgrade.
        let engine_identity = file_sha256(&std::env::current_exe()?)?;
        let mut joins = Vec::new();
        let mut receiver = Some(rx);
        for index in 0..count {
            let worker = Worker {
                library: library.clone(),
                runtime: runtime.clone(),
                stop: stop.clone(),
                subscribers: subscribers.clone(),
                status: status.clone(),
                sequence: sequence.clone(),
                engine: None,
                engine_identity: engine_identity.clone(),
                #[cfg(feature = "autotag")]
                tagger: tagger.clone(),
                #[cfg(feature = "autotag")]
                album_groups: album_groups.clone(),
            };
            let callback = tx.clone();
            let receiver = if index == 0 { receiver.take() } else { None };
            match thread::Builder::new()
                .name(format!("cuerithm-preparation-{index}"))
                .spawn(move || {
                    if let Some(rx) = receiver {
                        worker.run(rx, callback);
                    } else {
                        worker.run_analysis();
                    }
                }) {
                Ok(join) => joins.push(join),
                Err(error) => {
                    stop.store(true, Ordering::Relaxed);
                    for join in joins {
                        let _ = join.join();
                    }
                    return Err(error.into());
                }
            }
        }
        Ok(Self {
            library,
            tx,
            stop,
            join: Mutex::new(joins),
            subscribers,
            status,
        })
    }
    pub fn enqueue_paths(&self, paths: &[PathBuf]) -> Result<Vec<JobId>> {
        let snapshot = self.library.settings()?;
        let recursive = snapshot.settings.include_subfolders();
        let rev = snapshot.revision;
        let mut files = Vec::new();
        for path in paths {
            if path.is_dir() {
                for entry in walkdir::WalkDir::new(path)
                    .max_depth(if recursive { usize::MAX } else { 1 })
                    .follow_links(false)
                    .into_iter()
                    .filter_entry(|e| !e.path().starts_with(self.library.data_dir()))
                {
                    let entry = entry?;
                    if entry.file_type().is_file() && watch::supported_path(entry.path()) {
                        files.push(entry.into_path());
                    }
                }
            } else {
                files.push(path.clone());
            }
        }
        let jobs = self.library.enqueue(&files, rev)?;
        self.wake();
        Ok(jobs)
    }
    pub fn retry(&self, job: JobId, revision: Option<SettingsRevision>) -> Result<()> {
        self.library.retry(&job, revision)?;
        self.wake();
        Ok(())
    }
    pub fn reprepare(&self, track: TrackId) -> Result<()> {
        self.library.clear_track_analysis_cache(&track)?;
        let path = self.library.track_path(&track)?;
        let jobs = self.enqueue_paths(&[path])?;
        for job in jobs {
            self.retry(job, Some(self.library.settings()?.revision))?;
        }
        Ok(())
    }
    pub fn wake(&self) {
        let _ = self.tx.try_send(Control::Wake);
    }
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake();
        let joins = std::mem::take(&mut *self.join.lock().unwrap());
        for join in joins {
            let _ = join.join();
        }
        self.subscribers.lock().unwrap().clear();
    }
    pub fn subscribe(&self) -> Receiver<PreparationEvent> {
        let (tx, rx) = mpsc::sync_channel(256);
        self.subscribers.lock().unwrap().push(tx);
        rx
    }
    pub fn status(&self) -> ServiceStatus {
        self.status.lock().unwrap().clone()
    }
    pub fn retry_attention(&self) -> Result<()> {
        let revision = self.library.settings()?.revision;
        let mut errors = Vec::new();
        for job in self.library.jobs()? {
            if job.state == "attention"
                && let Err(e) = self.library.retry(&job.id, Some(revision.clone()))
            {
                errors.push(e.to_string());
            }
        }
        self.wake();
        ensure!(
            errors.is_empty(),
            "Some tracks could not be retried: {}",
            errors.join("; ")
        );
        Ok(())
    }
}
impl Drop for PreparationService {
    fn drop(&mut self) {
        self.shutdown();
    }
}
struct Worker {
    library: Library,
    runtime: MediaRuntime,
    stop: Arc<AtomicBool>,
    subscribers: Subscribers,
    status: Arc<Mutex<ServiceStatus>>,
    sequence: Arc<AtomicU64>,
    engine: Option<(SettingsRevision, Engine)>,
    engine_identity: String,
    #[cfg(feature = "autotag")]
    tagger: Arc<Tagger>,
    #[cfg(feature = "autotag")]
    album_groups: Arc<crate::album_context::AlbumGroups>,
}
impl Worker {
    fn emit(&self, job: &JobRun, stage: &str, state: &str) {
        let mut subscribers = self.subscribers.lock().unwrap();
        let event = PreparationEvent {
            attempt: job.attempt,
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed) + 1,
            job_id: job.id.clone(),
            track_id: job.track_id.clone(),
            stage: stage.into(),
            state: state.into(),
        };
        subscribers.retain(|tx| match tx.try_send(event.clone()) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }
    fn error(&self, error: anyhow::Error) {
        self.status.lock().unwrap().last_error = Some(format!("{error:#}"));
    }
    fn run_analysis(mut self) {
        while !self.stop.load(Ordering::Relaxed) {
            match self.library.claim_next() {
                Ok(Some(job)) => self.process(job),
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(error) => {
                    self.error(error);
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
    fn run(mut self, rx: Receiver<Control>, tx: SyncSender<Control>) {
        let mut discovery = watch::Discovery::default();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let Ok(event) = event
                    && !matches!(event.kind, notify::EventKind::Access(_))
                {
                    let _ = tx.try_send(Control::Changed(event.paths));
                }
            })
            .ok();
        let mut watched = std::collections::HashMap::new();
        let mut next_scan = Instant::now();
        let mut pending = HashSet::new();
        while !self.stop.load(Ordering::Relaxed) {
            while let Ok(control) = rx.try_recv() {
                match control {
                    Control::Changed(paths) => pending.extend(paths),
                    Control::Wake => next_scan = Instant::now(),
                }
            }
            let snapshot = match self.library.settings() {
                Ok(s) => s,
                Err(e) => {
                    self.error(e);
                    break;
                }
            };
            let folders = &snapshot.settings.watch_folders;
            let recursive = snapshot.settings.include_subfolders();
            if Instant::now() >= next_scan {
                let mut errors = Vec::new();
                for (old, old_recursive) in watched.clone() {
                    if !folders.contains(&old) || old_recursive != recursive {
                        if let Some(ref mut w) = watcher {
                            let _ = w.unwatch(&old);
                        }
                        watched.remove(&old);
                    }
                }
                for path in folders {
                    if !path.is_dir() {
                        errors.push(format!("Folder is offline: {}", path.display()));
                        continue;
                    }
                    if !watched.contains_key(path) {
                        if let Some(ref mut w) = watcher {
                            match w.watch(
                                path,
                                if recursive {
                                    RecursiveMode::Recursive
                                } else {
                                    RecursiveMode::NonRecursive
                                },
                            ) {
                                Ok(()) => {
                                    watched.insert(path.clone(), recursive);
                                }
                                Err(e) => errors.push(e.to_string()),
                            }
                        } else {
                            errors.push("Live folder notifications unavailable; periodic scanning is active".into());
                        }
                    }
                }
                self.status.lock().unwrap().watch_errors = errors;
                pending.extend(discovery.scan(folders, self.library.data_dir(), recursive));
                next_scan = Instant::now() + Duration::from_secs(5);
            }
            if !pending.is_empty() {
                let candidates: Vec<PathBuf> = pending
                    .drain()
                    .filter(|p| {
                        p.is_file()
                            && watch::in_scope(p, folders, recursive)
                            && watch::supported_path(p)
                            && !p.starts_with(self.library.data_dir())
                    })
                    .collect();
                if let Err(e) = self
                    .library
                    .enqueue_discovered(&candidates, snapshot.revision)
                {
                    self.error(e);
                }
            }
            match self.library.claim_next() {
                Ok(Some(job)) => self.process(job),
                Ok(None) => match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(Control::Changed(paths)) => pending.extend(paths),
                    Ok(Control::Wake) => next_scan = Instant::now(),
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                },
                Err(e) => {
                    self.error(e);
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
    fn process(&mut self, job: JobRun) {
        let mut stage = "setup";
        let outcome = (|| -> Result<()> {
            job.snapshot.settings.ready()?;
            stage = "waiting";
            self.emit(&job, stage, "running");
            watch::wait_for_stable(
                &job.path,
                Duration::from_millis(job.snapshot.settings.stability_millis),
                &self.stop,
            )?;
            if file_sha256(&job.path)? != job.source_sha256 {
                self.library.retry_for_run(&job)?;
                self.emit(&job, "queued", "superseded");
                return Ok(());
            }
            ensure!(
                !self.stop.load(Ordering::Relaxed),
                "Preparation is stopping"
            );
            if self
                .engine
                .as_ref()
                .is_none_or(|(revision, _)| revision != &job.settings_revision)
            {
                let engine = Engine::new(&job.snapshot.settings.engine)?;
                ensure!(
                    engine.model_identity() == job.snapshot.models,
                    "Models changed since settings were saved; save Settings again"
                );
                self.engine = Some((job.settings_revision.clone(), engine));
            }
            stage = "decoding";
            self.library.set_stage_for_run(&job, stage)?;
            self.emit(&job, stage, "running");
            let audio = self
                .runtime
                .decode(&job.path, &self.library.data_dir().join("cache/media"))?;
            ensure!(
                !self.stop.load(Ordering::Relaxed),
                "Preparation is stopping"
            );
            ensure!(
                audio.source_sha256 == job.source_sha256,
                "Source changed before analysis"
            );
            let mut result = if let Some(cached) =
                self.library
                    .cached_analysis(&job, &self.engine_identity, &audio)?
            {
                cached
            } else {
                stage = "analysis";
                self.library.set_stage_for_run(&job, stage)?;
                self.emit(&job, stage, "running");
                let result = self
                    .engine
                    .as_mut()
                    .context("Missing analysis engine")?
                    .1
                    .analyze_prepared(&audio)?;
                self.library
                    .cache_analysis(&job, &self.engine_identity, &result)?;
                result
            };
            ensure!(
                !self.stop.load(Ordering::Relaxed),
                "Preparation is stopping"
            );
            #[cfg(feature = "autotag")]
            if job.snapshot.settings.autotag_enabled || job.snapshot.settings.cleanup_enabled {
                let mut offline_config = cuerithm_autotag::AutotagConfig::default();
                offline_config.options.tags.clear();
                offline_config.write_energy = false;
                let config = if job.snapshot.settings.autotag_enabled {
                    job.snapshot
                        .settings
                        .tagging
                        .as_ref()
                        .context("Missing tagging settings")?
                } else {
                    &offline_config
                };
                if job.snapshot.settings.autotag_enabled {
                    stage = "matching";
                    self.library.set_stage_for_run(&job, stage)?;
                    self.emit(&job, stage, "running");
                }
                let input = TagInput {
                    path: job.path.clone(),
                    source_sha256: audio.source_sha256.clone(),
                    media: audio.info.clone(),
                    duration_seconds: audio.clock.duration_seconds(),
                };
                let folder = job.path.parent().context("Missing track folder")?;
                let hint = if job.snapshot.settings.autotag_enabled && config.options.album_tagging
                {
                    let members = self
                        .library
                        .incoming_album_members(folder, &job.settings_revision)?;
                    self.album_groups
                        .prepare(folder, &job.settings_revision, &members);
                    self.album_groups
                        .selected(folder, &job.settings_revision)
                        .map(|choice| choice.hint(&self.tagger, config, &self.stop))
                        .transpose()?
                        .flatten()
                } else {
                    None
                };
                let matched = if job.snapshot.settings.autotag_enabled {
                    self.tagger
                        .match_track_with_album(&input, config, &self.stop, hint.as_ref())?
                } else {
                    cuerithm_autotag::TagMatchOutcome {
                        status: cuerithm_autotag::MatchStatus::NoMatch,
                        matches: vec![],
                        attempted: vec![],
                        warnings: vec![],
                    }
                };
                ensure!(
                    !self.stop.load(Ordering::Relaxed),
                    "Preparation is stopping"
                );
                if job.snapshot.settings.autotag_enabled {
                    stage = "tagging";
                    self.library.set_stage_for_run(&job, stage)?;
                    self.emit(&job, stage, "running");
                }
                let key = result.analysis.key.as_ref();
                let measured = AnalysisTags {
                    camelot_key: key.map(|k| k.camelot.clone()),
                    standard_key: key.map(|k| k.key.name()),
                    bpm: crate::publication::tag_bpm(&result, &job.snapshot.settings)?,
                    energy: result.analysis.energy.as_ref().map(|e| e.level),
                };
                let mut plan = TagWritePlan::build(input, &matched, config, &measured)?;
                if job.snapshot.settings.cleanup_enabled {
                    stage = "cleanup";
                    self.library.set_stage_for_run(&job, stage)?;
                    self.emit(&job, stage, "running");
                    plan = plan
                        .with_custom_tags(self.library.track_custom_tags(&job.track_id)?)?
                        .with_cleanup(
                            job.snapshot
                                .settings
                                .cleanup
                                .as_ref()
                                .context("Missing cleanup settings")?
                                .clone(),
                        )?;
                }
                let mut staged = plan.stage_prepared(
                    &self.runtime,
                    &audio,
                    &self.library.data_dir().join("tagging"),
                )?;
                self.engine
                    .as_ref()
                    .context("Missing analysis engine")?
                    .1
                    .refresh_prepared_cues(
                        &audio,
                        &mut result,
                        staged
                            .native_metadata
                            .first(cuerithm_cleanup::MetadataField::Genre),
                    )?;
                ensure!(
                    !self.stop.load(Ordering::Relaxed),
                    "Preparation is stopping"
                );
                crate::publication::apply_for_track(
                    &self.library,
                    &job.track_id,
                    &mut result,
                    &job.snapshot.settings,
                )?;
                stage = "writing_tags";
                // The journal transaction sets this stage. Once it succeeds,
                // always enter publication, which settles obsolete attempts too.
                let journal = self.library.begin_tag_write(&job, &mut staged, &result)?;
                self.emit(&job, stage, "running");
                self.library.publish_preparation(&job, &journal)?;
                if job.snapshot.settings.autotag_enabled && config.options.album_tagging {
                    let release = matched
                        .matches
                        .iter()
                        .find(|m| m.track.platform == "beatport")
                        .and_then(|m| m.track.release_id.as_deref());
                    self.album_groups.observe(
                        folder,
                        &job.settings_revision,
                        &job.track_id,
                        release,
                        config.options.album_tagging_ratio,
                    );
                }
            } else {
                stage = "committing";
                self.library.set_stage_for_run(&job, stage)?;
                self.emit(&job, stage, "running");
                let native =
                    cuerithm_autotag::read_metadata(&audio.snapshot_path, &audio.info).ok();
                let genre = native
                    .as_ref()
                    .map(|metadata| metadata.first(cuerithm_cleanup::MetadataField::Genre))
                    .unwrap_or_else(|| audio.info.tags.get("genre").map(String::as_str));
                self.engine
                    .as_ref()
                    .context("Missing analysis engine")?
                    .1
                    .refresh_prepared_cues(&audio, &mut result, genre)?;
                crate::publication::apply_for_track(
                    &self.library,
                    &job.track_id,
                    &mut result,
                    &job.snapshot.settings,
                )?;
                self.library.apply_prepared_analysis_for_run(
                    &job,
                    &result,
                    &audio.info,
                    native.as_ref(),
                )?;
            }
            #[cfg(not(feature = "autotag"))]
            {
                stage = "committing";
                self.library.set_stage_for_run(&job, stage)?;
                self.emit(&job, stage, "running");
                crate::publication::apply_for_track(
                    &self.library,
                    &job.track_id,
                    &mut result,
                    &job.snapshot.settings,
                )?;
                self.library
                    .apply_analysis_for_run(&job, &result, &audio.info)?;
            }
            self.emit(&job, "completed", "completed");
            Ok(())
        })();
        if let Err(error) = outcome {
            if self.stop.load(Ordering::Relaxed) {
                return;
            }
            // A provider lookup can outlive an external metadata edit. The DSP and
            // staged tags belong to the old bytes; enqueue the new identity only
            // while this exact attempt still owns the track.
            if file_sha256(&job.path).is_ok_and(|hash| hash != job.source_sha256)
                && self.library.retry_for_run(&job).is_ok()
            {
                self.emit(&job, "queued", "superseded");
                return;
            }
            #[cfg(feature = "autotag")]
            if let Some(config) = job.snapshot.settings.tagging.as_ref()
                && config.options.album_tagging
                && let Some(folder) = job.path.parent()
            {
                self.album_groups.observe(
                    folder,
                    &job.settings_revision,
                    &job.track_id,
                    None,
                    config.options.album_tagging_ratio,
                );
            }
            if self
                .library
                .fail_for_run(&job, stage, &format!("{error:#}"))
                .is_ok()
            {
                self.emit(&job, stage, "attention");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::worker_count;

    #[test]
    fn workers_use_75_percent_of_available_cores_rounded_down() {
        for (cores, expected) in [
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 2),
            (4, 3),
            (8, 6),
            (14, 10),
            (16, 12),
            (64, 48),
        ] {
            assert_eq!(worker_count(cores), expected, "{cores} cores");
        }
    }
}

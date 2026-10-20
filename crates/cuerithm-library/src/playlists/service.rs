use super::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// Refreshes cached smart memberships without decoding, tagging or cue analysis.
pub struct SmartPlaylistService {
    stop: Arc<AtomicBool>,
    wake: mpsc::Sender<()>,
    worker: Mutex<Option<JoinHandle<()>>>,
    error: Arc<Mutex<Option<String>>>,
}
impl SmartPlaylistService {
    pub fn start(library: Library) -> Result<Self> {
        Self::start_with_clock(library, crate::now, Duration::from_secs(1))
    }
    fn start_with_clock(
        library: Library,
        clock: impl Fn() -> i64 + Send + 'static,
        interval: Duration,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let error = Arc::new(Mutex::new(None));
        let (wake, receiver) = mpsc::channel();
        let stopped = stop.clone();
        let last_error = error.clone();
        let worker = thread::Builder::new()
            .name("smart-playlists".into())
            .spawn(move || {
                loop {
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let result = (|| -> Result<()> {
                        let now = clock();
                        library.reconcile_automatic_playlists()?;
                        if library.smart_lists_need_refresh(now)? {
                            let evaluation =
                                library.evaluate_smart_lists_cancellable(now, Some(&stopped))?;
                            if !stopped.load(Ordering::Acquire) {
                                library.commit_smart_evaluation(evaluation, clock())?;
                            }
                        }
                        Ok(())
                    })();
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    *last_error.lock().unwrap_or_else(|e| e.into_inner()) =
                        result.err().map(|e| format!("{e:#}"));
                    match receiver.recv_timeout(interval) {
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        Ok(Self {
            stop,
            wake,
            worker: Mutex::new(Some(worker)),
            error,
        })
    }
    pub fn last_error(&self) -> Result<Option<String>> {
        Ok(self
            .error
            .lock()
            .map_err(|_| anyhow::anyhow!("Smart refresh status interrupted"))?
            .clone())
    }
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.wake.send(());
        let worker = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
        if worker.is_some_and(|w| w.join().is_err()) {
            *self.error.lock().unwrap_or_else(|e| e.into_inner()) =
                Some("Smart refresh worker stopped unexpectedly".into());
        }
    }
}
impl Drop for SmartPlaylistService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuerithm_smartlists::{DateUnit, Operator, RuleValue, SmartDefinition, SmartRule};
    use std::sync::atomic::AtomicI64;

    #[test]
    fn relative_memberships_expire_on_the_captured_clock_without_a_refresh_loop() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(&dir.path().join("db")).unwrap();
        let clock = Arc::new(AtomicI64::new(1_792_324_800_000));
        let source = dir.path().join("Clock test.flac");
        std::fs::write(&source, b"Authored playlist fixture; no decoding").unwrap();
        let id = library
            .enqueue(&[source], library.settings().unwrap().revision)
            .unwrap()[0]
            .clone();
        let track = library.jobs().unwrap()[0].track_id.clone();
        library
            .conn()
            .unwrap()
            .execute(
                "UPDATE tracks SET state='ready',created=?1 WHERE id=?2",
                params![clock.load(Ordering::Acquire) - 86_400_000 + 30_000, track],
            )
            .unwrap();
        library
            .apply_playlist_edit(
                PlaylistEdit::Create {
                    id: Some("recent".into()),
                    parent_id: None,
                    kind: PlaylistKind::Smartlist,
                    name: "Recent".into(),
                    definition: Some(SmartDefinition {
                        match_all: true,
                        rules: vec![SmartRule {
                            id: "date".into(),
                            field: "dateAdded".into(),
                            operator: Operator::DateRecent,
                            value: RuleValue::Relative {
                                count: 1,
                                unit: DateUnit::Days,
                            },
                            or: false,
                        }],
                    }),
                },
                library.playlist_state().unwrap().revision,
            )
            .unwrap();
        let prefs = library.get_playlist_preferences().unwrap();
        library
            .save_playlist_preferences(
                PlaylistPreferences {
                    time_zone: Some("UTC".into()),
                    ..prefs.preferences
                },
                prefs.revision,
            )
            .unwrap();
        let shared = clock.clone();
        let service = SmartPlaylistService::start_with_clock(
            library.clone(),
            move || shared.load(Ordering::Acquire),
            Duration::from_millis(10),
        )
        .unwrap();
        let wait = |members: Vec<String>| {
            let started = std::time::Instant::now();
            loop {
                if library
                    .playlist_evaluation_status_at("recent", clock.load(Ordering::Acquire))
                    .unwrap()
                    .state
                    == SmartState::Ready
                    && library.playlist_members("recent").unwrap() == members
                {
                    break;
                }
                assert!(started.elapsed() < Duration::from_secs(5));
                thread::sleep(Duration::from_millis(5));
            }
        };
        wait(vec![track]);
        let revision = library.revision().unwrap();
        thread::sleep(Duration::from_millis(40));
        assert_eq!(library.revision().unwrap(), revision);
        clock.fetch_add(60_001, Ordering::AcqRel);
        wait(vec![]);
        service.shutdown();
        assert!(service.last_error().unwrap().is_none());
        assert_eq!(library.jobs().unwrap()[0].id, id);
        assert_eq!(library.jobs().unwrap()[0].attempts, 0);
    }
}

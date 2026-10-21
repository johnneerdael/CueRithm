use super::*;
use crate::Library;
use anyhow::Result;
use cuerithm_media::{CancellationToken, MediaRuntime};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurationEvent {
    pub track_id: Option<String>,
    pub revision: u64,
    pub state: String,
}
struct Control {
    wake: Mutex<bool>,
    signal: Condvar,
    running: AtomicBool,
    flushing: AtomicBool,
    taxonomy_revision: AtomicU64,
    listeners: Mutex<Vec<mpsc::Sender<CurationEvent>>>,
}
#[derive(Clone)]
pub struct CurationService {
    pub(super) library: Library,
    pub(super) runtime: Option<MediaRuntime>,
    control: Arc<Control>,
}
pub struct CurationWorker {
    cancel: CancellationToken,
    control: Arc<Control>,
    thread: Option<std::thread::JoinHandle<()>>,
}
pub(super) struct PublicationLease<'a>(&'a AtomicBool);
impl Drop for PublicationLease<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
impl CurationService {
    pub fn new(library: Library, runtime: Option<MediaRuntime>) -> Self {
        Self {
            library,
            runtime,
            control: Arc::new(Control {
                wake: Mutex::new(true),
                signal: Condvar::new(),
                running: AtomicBool::new(false),
                flushing: AtomicBool::new(false),
                taxonomy_revision: AtomicU64::new(u64::MAX),
                listeners: Mutex::new(vec![]),
            }),
        }
    }
    pub(super) fn publication_lease(&self) -> Result<PublicationLease<'_>> {
        anyhow::ensure!(
            self.control
                .flushing
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok(),
            "Curate publication is running; wait for it before this action"
        );
        Ok(PublicationLease(&self.control.flushing))
    }
    pub fn subscribe(&self) -> mpsc::Receiver<CurationEvent> {
        let (tx, rx) = mpsc::channel();
        if let Ok(mut listeners) = self.control.listeners.lock() {
            listeners.push(tx)
        }
        rx
    }
    pub fn wake(&self) {
        if let Ok(mut wake) = self.control.wake.lock() {
            *wake = true;
            self.control.signal.notify_one();
        }
    }
    pub fn start_background(&self) -> Result<CurationWorker> {
        anyhow::ensure!(
            !self.control.running.swap(true, Ordering::SeqCst),
            "Curate worker already running"
        );
        let service = self.clone();
        let cancel = CancellationToken::default();
        let stop = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("curation-autosave".into())
            .spawn(move || {
                while !stop.is_cancelled() {
                    let _ = service.drain_pending(20, &stop, false);
                    let Ok(wake) = service.control.wake.lock() else {
                        break;
                    };
                    let Ok((mut wake, _)) = service.control.signal.wait_timeout_while(
                        wake,
                        std::time::Duration::from_millis(500),
                        |wake| !*wake && !stop.is_cancelled(),
                    ) else {
                        break;
                    };
                    *wake = false;
                }
                service.control.running.store(false, Ordering::SeqCst);
            })?;
        Ok(CurationWorker {
            cancel,
            control: self.control.clone(),
            thread: Some(thread),
        })
    }
    pub fn catalog(&self) -> Result<CurationCatalog> {
        super::taxonomy::catalog(self)
    }
    pub fn edit_taxonomy(
        &self,
        request: CurationTaxonomyRequest,
    ) -> Result<CurationTaxonomyReceipt> {
        super::taxonomy::edit(self, request)
    }
    pub fn snapshot(&self, ids: &[String]) -> Result<CurationSnapshot> {
        super::outbox::snapshot(self, ids)
    }
    pub fn update(&self, request: CurationUpdateRequest) -> Result<CurationReceipt> {
        super::outbox::update(self, request, false)
    }
    pub fn reexport(&self, request: CurationReexportRequest) -> Result<CurationReceipt> {
        super::outbox::update(
            self,
            CurationUpdateRequest {
                request_id: request.request_id,
                taxonomy_revision: request.taxonomy_revision,
                tracks: request.tracks,
                patch: Default::default(),
            },
            true,
        )
    }
    pub fn pending_page(&self, offset: usize, limit: usize) -> Result<CurationPendingPage> {
        super::outbox::pending(self, offset, limit)
    }
    pub fn history_page(&self, offset: usize, limit: usize) -> Result<CurationHistoryPage> {
        super::publication::history(self, offset, limit)
    }
    pub fn publication(&self, id: &str) -> Result<CurationPublication> {
        Ok(super::publication::load(self, id)?.public)
    }
    pub fn flush_pending(
        &self,
        limit: usize,
        cancel: &CancellationToken,
    ) -> Result<CurationFlushResult> {
        self.drain_pending(limit, cancel, true)
    }
    fn drain_pending(
        &self,
        limit: usize,
        cancel: &CancellationToken,
        force: bool,
    ) -> Result<CurationFlushResult> {
        if self
            .control
            .flushing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(CurationFlushResult {
                pending: self.pending_page(0, 1)?.total,
                ..Default::default()
            });
        }
        let _reset = PublicationLease(&self.control.flushing);
        let revision = super::storage::tax_revision(&*self.library.conn()?)?;
        if self
            .control
            .taxonomy_revision
            .swap(revision, Ordering::SeqCst)
            != revision
        {
            self.emit(CurationEvent {
                track_id: None,
                revision,
                state: "taxonomy".into(),
            });
        }
        super::publication::flush(self, limit, cancel, force)
    }
    pub fn retry(
        &self,
        track: &str,
        revision: u64,
        cancel: &CancellationToken,
    ) -> Result<CurationTrackState> {
        super::publication::retry(self, track, revision, cancel)
    }
    pub fn discard(&self, track: &str, revision: u64) -> Result<CurationTrackState> {
        super::outbox::discard(self, track, revision)
    }
    pub fn undo(&self, id: &str, cancel: &CancellationToken) -> Result<CurationPublication> {
        super::publication::undo(self, id, cancel)
    }
    pub(super) fn emit(&self, event: CurationEvent) {
        if let Ok(mut listeners) = self.control.listeners.lock() {
            listeners.retain(|sender| sender.send(event.clone()).is_ok());
        }
    }
}
impl CurationWorker {
    pub fn request_shutdown(&self) {
        self.cancel.cancel();
        if let Ok(mut wake) = self.control.wake.lock() {
            *wake = true;
            self.control.signal.notify_all();
        }
    }
    pub fn shutdown(&mut self) {
        self.request_shutdown();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for CurationWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
impl Library {
    pub(crate) fn recover_curation(&self) -> Result<()> {
        self.conn()?.execute("UPDATE curation_tracks SET state='recovery_required',error='Curate publication interrupted; retained evidence must be recovered' WHERE active_publication_id IS NOT NULL",[])?;
        self.conn()?.execute(
            "UPDATE curation_publications SET state='recovery_required' WHERE state='saving'",
            [],
        )?;
        Ok(())
    }
}

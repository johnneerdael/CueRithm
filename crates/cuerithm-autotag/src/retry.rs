//! Shared provider cooldowns and bounded retries; untyped failures are never guessed transient.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureKind {
    Authentication,
    Unavailable,
    RateLimited,
    Timeout,
    Transport,
    Parse,
    InvalidResponse,
    Unknown,
}
#[derive(Debug)]
pub struct BackendFailure {
    pub kind: ProviderFailureKind,
    pub message: String,
}
impl BackendFailure {
    pub fn new(kind: ProviderFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl fmt::Display for BackendFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for BackendFailure {}
pub(crate) fn classify(error: &anyhow::Error) -> ProviderFailureKind {
    classify_cause(error.as_ref(), 0).unwrap_or(ProviderFailureKind::Unknown)
}
fn classify_cause(
    error: &(dyn std::error::Error + 'static),
    depth: usize,
) -> Option<ProviderFailureKind> {
    if depth > 32 {
        return None;
    }
    if error.is::<onetagger_selected_providers::http::SolverFailure>() {
        // The transport already exhausted its bounded solver attempts.
        return Some(ProviderFailureKind::Unavailable);
    }
    if let Some(error) = error.downcast_ref::<BackendFailure>() {
        return Some(error.kind);
    }
    if let Some(error) = error.downcast_ref::<reqwest::Error>() {
        let kind = if error.is_timeout() {
            ProviderFailureKind::Timeout
        } else if let Some(status) = error.status() {
            match status.as_u16() {
                401 | 403 => ProviderFailureKind::Authentication,
                429 => ProviderFailureKind::RateLimited,
                500..=599 => ProviderFailureKind::Transport,
                _ => ProviderFailureKind::InvalidResponse,
            }
        } else if error.is_decode() {
            ProviderFailureKind::Parse
        } else if error.is_connect() || error.is_request() || error.is_body() {
            ProviderFailureKind::Transport
        } else {
            ProviderFailureKind::Unknown
        };
        return Some(kind);
    }
    // blocking Response implements Read by wrapping reqwest failures in io::Error.
    // Inspect the actual inner error; do not guess retryability from its text.
    if let Some(error) = error.downcast_ref::<std::io::Error>()
        && let Some(inner) = error.get_ref()
        && let Some(kind) = classify_cause(inner, depth + 1)
    {
        return Some(kind);
    }
    if error.is::<serde_json::Error>() {
        return Some(ProviderFailureKind::Parse);
    }
    error
        .source()
        .and_then(|cause| classify_cause(cause, depth + 1))
}
#[derive(Default)]
pub(crate) struct Cooldown(Mutex<Option<Instant>>);
impl Cooldown {
    pub fn run<T>(
        &self,
        cancel: &Arc<AtomicBool>,
        limit: Duration,
        operation: impl Fn() -> Result<T>,
    ) -> Result<T> {
        let deadline = Instant::now() + limit;
        for attempt in 0..3 {
            loop {
                ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
                let remaining = self
                    .0
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Provider cooldown interrupted"))?
                    .and_then(|until| until.checked_duration_since(Instant::now()));
                let Some(remaining) = remaining else {
                    break;
                };
                ensure!(
                    Instant::now() < deadline,
                    "Provider retry deadline exceeded"
                );
                std::thread::sleep(remaining.min(Duration::from_millis(25)));
            }
            ensure!(
                Instant::now() < deadline,
                "Provider retry deadline exceeded"
            );
            let result = operation();
            if Instant::now() >= deadline {
                return Err(BackendFailure::new(
                    ProviderFailureKind::Timeout,
                    "Provider retry deadline exceeded",
                )
                .into());
            }
            ensure!(!cancel.load(Ordering::Relaxed), "Tagging is stopping");
            match result {
                Err(error)
                    if attempt < 2
                        && matches!(
                            classify(&error),
                            ProviderFailureKind::RateLimited
                                | ProviderFailureKind::Timeout
                                | ProviderFailureKind::Transport
                        ) =>
                {
                    let until = Instant::now() + Duration::from_millis(250 * (1 << attempt));
                    let until = error.downcast_ref::<onetagger_selected_providers::beatport::TokenRefreshFailure>()
                        .map_or(until, |failure| until.max(failure.retry_after()));
                    let mut cooldown = self
                        .0
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Provider cooldown interrupted"))?;
                    *cooldown = Some(cooldown.map_or(until, |prior| prior.max(until)));
                }
                other => return other,
            }
        }
        unreachable!("three bounded attempts return or fail")
    }
}

/// Cooperatively expires an entire provider stage, including its gate and retry waits.
/// Native transports also bound each in-flight HTTP request; arbitrary external backends
/// must honor the cancellation contract rather than being detached on timeout.
pub(crate) fn bounded<T>(
    parent: &Arc<AtomicBool>,
    limit: Duration,
    operation: impl FnOnce(&Arc<AtomicBool>) -> Result<T>,
) -> Result<T> {
    let cancel = Arc::new(AtomicBool::new(parent.load(Ordering::Relaxed)));
    let finished = (Mutex::new(false), std::sync::Condvar::new());
    let deadline = Instant::now() + limit;
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut done = finished.0.lock().unwrap_or_else(|p| p.into_inner());
            while !*done {
                if parent.load(Ordering::Relaxed) || Instant::now() >= deadline {
                    cancel.store(true, Ordering::Relaxed);
                    break;
                }
                done = finished
                    .1
                    .wait_timeout(done, Duration::from_millis(25))
                    .unwrap_or_else(|p| p.into_inner())
                    .0;
            }
        });
        struct Finish<'a>(&'a (Mutex<bool>, std::sync::Condvar));
        impl Drop for Finish<'_> {
            fn drop(&mut self) {
                *self.0.0.lock().unwrap_or_else(|p| p.into_inner()) = true;
                self.0.1.notify_all();
            }
        }
        let finish = Finish(&finished);
        let result = operation(&cancel);
        drop(finish);
        if parent.load(Ordering::Relaxed) {
            anyhow::bail!("Tagging is stopping");
        }
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
            return Err(BackendFailure::new(
                ProviderFailureKind::Timeout,
                "Provider stage deadline exceeded",
            )
            .into());
        }
        result
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solver_exhaustion_is_unavailable_and_is_not_retried_by_provider_cooldown() {
        let calls = std::cell::Cell::new(0);
        let error = Cooldown::default()
            .run(
                &Arc::new(AtomicBool::new(false)),
                Duration::from_secs(1),
                || -> Result<()> {
                    calls.set(calls.get() + 1);
                    Err(onetagger_selected_providers::http::SolverFailure(
                        "Fixture solver is unavailable".into(),
                    )
                    .into())
                },
            )
            .unwrap_err();
        assert_eq!(calls.get(), 1);
        assert_eq!(classify(&error), ProviderFailureKind::Unavailable);
    }
    #[test]
    fn stage_deadline_interrupts_cooperative_backend_without_detaching_it() {
        let stopped = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let error = bounded(&stopped, Duration::from_millis(30), |cancel| {
            while !cancel.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(classify(&error), ProviderFailureKind::Timeout);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(!stopped.load(Ordering::Relaxed));
    }
}

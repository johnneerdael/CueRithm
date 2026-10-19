use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    /// Share cancellation with synchronous library evaluators using an atomic flag.
    pub fn as_atomic(&self) -> &AtomicBool {
        &self.0
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> anyhow::Result<()> {
        if self.is_cancelled() {
            return Err(MediaTaskError::Cancelled.into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaTaskError {
    Cancelled,
    TimedOut,
    OutputLimit,
}
impl std::fmt::Display for MediaTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Media scan cancelled",
            Self::TimedOut => "Media scan exceeded its time limit",
            Self::OutputLimit => "Media scan exceeded its output limit",
        })
    }
}
impl std::error::Error for MediaTaskError {}

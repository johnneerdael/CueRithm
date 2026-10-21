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
    /// Share the same cancellation authority with synchronous provider gates.
    pub fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
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

#[cfg(test)]
mod sharing_tests {
    use super::*;
    #[test]
    fn provider_flag_and_token_share_cancellation_authority() {
        let token = CancellationToken::default();
        let flag = token.shared_flag();
        assert!(std::ptr::eq(flag.as_ref(), token.as_atomic()));
        token.cancel();
        assert!(flag.load(Ordering::Acquire));
        let token = CancellationToken::default();
        token.shared_flag().store(true, Ordering::Release);
        assert!(token.check().is_err());
    }
}

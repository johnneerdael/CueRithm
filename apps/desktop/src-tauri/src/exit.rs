//! A frontend draft receipt precedes native shutdown, including application Quit.
use anyhow::{Result, ensure};
use std::sync::Mutex;

#[derive(Default)]
pub struct ExitCoordinator {
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    ready: bool,
    next: u64,
    pending: Option<String>,
    stopping: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ExitAction {
    Shutdown,
    Notify(String),
    Waiting,
}
impl ExitCoordinator {
    pub fn register(&self) -> Result<Option<String>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Exit state interrupted"))?;
        ensure!(!state.stopping, "Application is closing");
        state.ready = true;
        Ok(state.pending.clone())
    }
    pub fn unregister(&self) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Exit state interrupted"))?
            .ready = false;
        Ok(())
    }
    pub fn request(&self, has_windows: bool) -> Result<ExitAction> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Exit state interrupted"))?;
        if state.stopping || !has_windows {
            return Ok(ExitAction::Shutdown);
        }
        if state.pending.is_some() {
            return Ok(ExitAction::Waiting);
        }
        if !state.ready {
            return Ok(ExitAction::Shutdown);
        }
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Exit request counter exhausted"))?;
        let id = format!("exit-{}", state.next);
        state.pending = Some(id.clone());
        Ok(ExitAction::Notify(id))
    }
    pub fn complete(&self, id: &str) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Exit state interrupted"))?;
        ensure!(
            !state.stopping && state.pending.as_deref() == Some(id),
            "Exit request changed"
        );
        state.pending = None;
        state.stopping = true;
        Ok(())
    }
    pub fn cancel(&self, id: &str) -> Result<bool> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Exit state interrupted"))?;
        if !state.stopping && state.pending.as_deref() == Some(id) {
            state.pending = None;
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quit_waits_for_the_current_draft_receipt_and_rejects_stale_completion() {
        let gate = ExitCoordinator::default();
        assert_eq!(gate.request(true).unwrap(), ExitAction::Shutdown);
        assert_eq!(gate.register().unwrap(), None);
        let ExitAction::Notify(first) = gate.request(true).unwrap() else {
            panic!("Quit bypassed the draft receipt");
        };
        assert_eq!(gate.request(true).unwrap(), ExitAction::Waiting);
        assert!(gate.complete("old-request").is_err());
        assert!(gate.cancel(&first).unwrap());
        let ExitAction::Notify(second) = gate.request(true).unwrap() else {
            panic!("Retry did not request a new receipt");
        };
        assert_ne!(first, second);
        assert!(gate.complete(&first).is_err());
        gate.complete(&second).unwrap();
        assert_eq!(gate.request(true).unwrap(), ExitAction::Shutdown);
    }
    #[test]
    fn listener_replacement_recovers_pending_exit_and_closed_windows_need_no_receipt() {
        let gate = ExitCoordinator::default();
        gate.register().unwrap();
        let ExitAction::Notify(id) = gate.request(true).unwrap() else {
            panic!("Expected draft flush");
        };
        gate.unregister().unwrap();
        assert_eq!(gate.register().unwrap(), Some(id));
        assert_eq!(gate.request(false).unwrap(), ExitAction::Shutdown);
    }
}

use anyhow::{Result, ensure};
use cuerithm_media::CancellationToken;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
pub struct MaintenanceTasks {
    state: Mutex<TaskState>,
    idle: Condvar,
}
#[derive(Default)]
struct TaskState {
    stopping: bool,
    active: Option<(String, CancellationToken)>,
}
pub struct Lease {
    pub token: CancellationToken,
    id: String,
    owner: Arc<MaintenanceTasks>,
}
impl MaintenanceTasks {
    pub fn begin(self: &Arc<Self>, id: &str) -> Result<Lease> {
        ensure!(
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_')),
            "Invalid maintenance operation ID"
        );
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Maintenance state interrupted"))?;
        ensure!(!state.stopping, "Application is shutting down");
        ensure!(
            state.active.is_none(),
            "Another maintenance operation is still running"
        );
        let token = CancellationToken::default();
        state.active = Some((id.into(), token.clone()));
        Ok(Lease {
            token,
            id: id.into(),
            owner: self.clone(),
        })
    }
    pub fn cancel(&self, id: &str) -> Result<bool> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Maintenance state interrupted"))?;
        if let Some((active, token)) = &state.active
            && active == id
        {
            token.cancel();
            return Ok(true);
        }
        Ok(false)
    }
    pub fn stop(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.stopping = true;
        if let Some((_, token)) = &state.active {
            token.cancel();
        }
    }
    /// Called on the shutdown thread after cancellation, so decoder children are reaped.
    pub fn wait_until_idle(&self) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        while state.active.is_some() {
            state = self.idle.wait(state).unwrap_or_else(|p| p.into_inner());
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.owner.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.active.as_ref().is_some_and(|(id, _)| id == &self.id) {
            state.active = None;
            self.owner.idle.notify_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_operation_owns_cancellation_until_its_worker_has_finished() {
        let tasks = Arc::new(MaintenanceTasks::default());
        let first = tasks.begin("first").unwrap();
        assert!(tasks.begin("second").is_err());
        assert!(!tasks.cancel("other").unwrap());
        assert!(!first.token.is_cancelled());
        assert!(tasks.cancel("first").unwrap());
        assert!(first.token.check().is_err());
        assert!(tasks.begin("second").is_err());
        drop(first);
        let second = tasks.begin("second").unwrap();
        assert!(!second.token.is_cancelled());
        assert!(!tasks.cancel("first").unwrap());
        drop(second);
        assert!(tasks.begin("third").is_ok());
    }
    #[test]
    fn shutdown_cancels_active_work_and_refuses_new_work() {
        let tasks = Arc::new(MaintenanceTasks::default());
        let first = tasks.begin("first").unwrap();
        tasks.stop();
        assert!(first.token.is_cancelled());
        drop(first);
        assert!(tasks.begin("next").is_err());
    }
    #[test]
    fn invalid_operation_ids_cannot_reserve_the_slot() {
        let tasks = Arc::new(MaintenanceTasks::default());
        for id in ["".to_owned(), "has spaces".to_owned(), "x".repeat(129)] {
            assert!(tasks.begin(&id).is_err());
        }
        assert!(tasks.begin("valid-1").is_ok());
    }
}

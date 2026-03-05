use crate::gh::client::GhClient;
use crate::gh::models::{PreflightDiagnostics, RepoContext};
use crate::gh::{GhError, GhResult};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub gh: GhClient,
    pub startup_repo: Option<RepoContext>,
    pub diagnostics: Option<PreflightDiagnostics>,
    pub startup_error: Option<GhError>,
    pub startup_elapsed: Duration,
}

impl AppState {
    pub fn startup_ready(&self) -> GhResult<()> {
        if let Some(error) = &self.startup_error {
            return Err(error.clone());
        }

        Ok(())
    }
}

static APP_STATE: OnceLock<Arc<Mutex<AppState>>> = OnceLock::new();

pub fn set_app_state(state: AppState) {
    if let Some(shared) = APP_STATE.get() {
        *shared.lock().expect("app state lock poisoned") = state;
        return;
    }

    let _ = APP_STATE.set(Arc::new(Mutex::new(state)));
}

pub fn app_state() -> Arc<Mutex<AppState>> {
    APP_STATE.get().expect("app state not initialized").clone()
}

pub fn app_state_snapshot() -> AppState {
    app_state().lock().expect("app state lock poisoned").clone()
}

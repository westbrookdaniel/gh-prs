use crate::gh::client::GhClient;
use crate::gh::models::{PreflightDiagnostics, RepoContext};
use crate::gh::{GhError, GhResult};
use std::sync::Arc;
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

pub type SharedState = Arc<AppState>;

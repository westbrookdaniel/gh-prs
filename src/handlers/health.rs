use serde::Serialize;

use crate::handlers::state::SharedState;
use crate::http::{Request, Response};

const HEALTH_OK_STATUS: &str = "ok";
const HEALTH_DEGRADED_STATUS: &str = "degraded";

pub async fn health(_request: Request, state: SharedState) -> Response {
    #[derive(Serialize)]
    struct HealthResponse<'a> {
        status: &'a str,
        repo: Option<&'a str>,
        startup_ms: u128,
        message: Option<String>,
    }

    let repo_name = state
        .startup_repo
        .as_ref()
        .map(|repo| repo.name_with_owner.as_str());

    match &state.startup_error {
        Some(error) => {
            let body = serde_json::to_vec(&HealthResponse {
                status: HEALTH_DEGRADED_STATUS,
                repo: repo_name,
                startup_ms: state.startup_elapsed.as_millis(),
                message: Some(error.message()),
            })
            .unwrap_or_else(|_| {
                b"{\"status\":\"degraded\",\"message\":\"health serialization failed\"}".to_vec()
            });

            Response::new(503, "Service Unavailable")
                .header("Content-Type", "application/json")
                .body(body)
        }
        None => {
            let body = serde_json::to_vec(&HealthResponse {
                status: HEALTH_OK_STATUS,
                repo: repo_name,
                startup_ms: state.startup_elapsed.as_millis(),
                message: None,
            })
            .unwrap_or_else(|_| b"{\"status\":\"ok\"}".to_vec());

            Response::new(200, "OK")
                .header("Content-Type", "application/json")
                .body(body)
        }
    }
}

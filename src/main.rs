mod cache_store;
mod gh;
mod gh_parsing;
mod handlers;
mod http;
mod search;
mod startup;
mod telemetry;
mod views;

use crate::handlers::state::set_app_state;
use crate::handlers::{AppState, register};
use crate::http::{App, StaticDirOptions, request_id, security_headers, static_dir};
use crate::startup::{init_runtime_storage, parse_startup_config, run_startup_checks};
use crate::telemetry::{init_tracing, request_tracing};
use std::io;

fn main() -> io::Result<()> {
    let _telemetry = init_tracing()?;
    init_runtime_storage()?;

    let config = parse_startup_config()?;

    smol::block_on(async move {
        let startup = run_startup_checks(config.repo.as_deref()).await;

        set_app_state(AppState {
            gh: crate::gh::client::GhClient::default(),
            startup_repo: startup.repo,
            startup_error: startup.startup_error,
            startup_elapsed: startup.startup_elapsed,
        });

        let static_assets = StaticDirOptions {
            url_prefix: "/assets".to_string(),
            root: "assets".into(),
            cache_control: Some("public, max-age=60".to_string()),
            fallthrough: true,
            ..StaticDirOptions::default()
        };

        let app = App::new()
            .middleware(request_id())
            .middleware(request_tracing())
            .middleware(security_headers())
            .middleware(static_dir(static_assets));

        register(app).serve(&config.bind_addr).await
    })
}

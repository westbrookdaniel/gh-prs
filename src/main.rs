mod gh;
mod handlers;
mod http;
mod views;

use crate::gh::client::GhClient;
use crate::handlers::{AppState, register};
use crate::http::{
    App, Response, StaticDirOptions, logger, request_id, security_headers, static_dir,
};
use std::env;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3000";

#[derive(Debug, Clone)]
struct StartupConfig {
    bind_addr: String,
    repo: Option<String>,
}

#[derive(Debug, Clone)]
struct StartupResult {
    diagnostics: Option<crate::gh::models::PreflightDiagnostics>,
    repo: Option<crate::gh::models::RepoContext>,
    startup_error: Option<crate::gh::GhError>,
    startup_elapsed: Duration,
}

async fn not_found(_request: crate::http::Request) -> Response {
    Response::not_found().text_body("Not Found")
}

fn main() -> io::Result<()> {
    let config = parse_startup_config()?;

    smol::block_on(async move {
        let startup = run_startup_checks(config.repo.as_deref()).await;

        let state = Arc::new(AppState {
            gh: GhClient::default(),
            repo: startup.repo,
            diagnostics: startup.diagnostics,
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
            .middleware(security_headers())
            .middleware(logger())
            .middleware(static_dir(static_assets));

        register(app, state)
            .any("/*path", not_found)
            .serve(&config.bind_addr)
            .await
    })
}

async fn run_startup_checks(explicit_repo: Option<&str>) -> StartupResult {
    let started = Instant::now();
    let gh = GhClient::default();

    let diagnostics = match gh.preflight().await {
        Ok(value) => value,
        Err(error) => {
            return StartupResult {
                diagnostics: None,
                repo: None,
                startup_error: Some(error),
                startup_elapsed: started.elapsed(),
            };
        }
    };

    let repo = match gh.resolve_repo(explicit_repo).await {
        Ok(repo) => repo,
        Err(error) => {
            return StartupResult {
                diagnostics: Some(diagnostics),
                repo: None,
                startup_error: Some(error),
                startup_elapsed: started.elapsed(),
            };
        }
    };

    StartupResult {
        diagnostics: Some(diagnostics),
        repo: Some(repo),
        startup_error: None,
        startup_elapsed: started.elapsed(),
    }
}

fn parse_startup_config() -> io::Result<StartupConfig> {
    let mut args = env::args().skip(1).peekable();
    let mut bind_addr = DEFAULT_BIND_ADDR.to_string();
    let mut repo = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--repo requires OWNER/REPO")
                })?;
                repo = Some(value);
            }
            "--port" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--port requires a numeric value",
                    )
                })?;
                let port = value.parse::<u16>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid --port value")
                })?;
                bind_addr = format!("127.0.0.1:{port}");
            }
            "--bind" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--bind requires host:port")
                })?;
                bind_addr = value;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {arg}"),
                ));
            }
        }
    }

    validate_local_bind(&bind_addr)?;

    Ok(StartupConfig { bind_addr, repo })
}

fn validate_local_bind(bind: &str) -> io::Result<()> {
    if !bind.starts_with("127.0.0.1:") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "server must bind to 127.0.0.1 only",
        ));
    }

    let port = bind
        .split_once(':')
        .and_then(|(_, value)| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid bind address"))?;

    if port == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bind port must be greater than zero",
        ));
    }

    Ok(())
}

fn print_usage() {
    println!("gh-prs");
    println!("  --repo OWNER/REPO   Optional explicit repository");
    println!("  --port PORT         Optional local port (default: 3000)");
    println!("  --bind HOST:PORT    Optional bind address (must use 127.0.0.1)");
}

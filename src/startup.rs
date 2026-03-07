use crate::gh::GhError;
use crate::gh::client::GhClient;
use crate::gh::models::RepoContext;
use crate::{cache_store, cache_store::SqliteCacheStore};
use std::env;
use std::io;
use std::time::{Duration, Instant};

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3000";

#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub bind_addr: String,
    pub repo: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StartupResult {
    pub repo: Option<RepoContext>,
    pub startup_error: Option<GhError>,
    pub startup_elapsed: Duration,
}

pub async fn run_startup_checks(explicit_repo: Option<&str>) -> StartupResult {
    let started = Instant::now();
    let gh = GhClient::default();
    let _ = gh.cache_db_path();

    match gh.preflight().await {
        Ok(_) => {}
        Err(error) => {
            return StartupResult {
                repo: None,
                startup_error: Some(error),
                startup_elapsed: started.elapsed(),
            };
        }
    }

    let repo = gh.resolve_repo(explicit_repo).await.ok();

    StartupResult {
        repo,
        startup_error: None,
        startup_elapsed: started.elapsed(),
    }
}

pub fn init_runtime_storage() -> io::Result<()> {
    let store = SqliteCacheStore::open_default()?;
    let _ = smol::block_on(store.prune_expired());
    let app_home = cache_store::default_app_home()?;
    tracing::info!(path = %app_home.display(), "cache home ready");
    tracing::info!(path = %store.db_path().display(), "cache database ready");
    Ok(())
}

pub fn parse_startup_config() -> io::Result<StartupConfig> {
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

pub fn validate_local_bind(bind: &str) -> io::Result<()> {
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

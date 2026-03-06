use crate::gh::commands::{CommandResult, GhCommand};
use crate::gh::{CommandClass, GhError, GhResult};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub trait CommandRunner: Send + Sync {
    fn run(&self, command: GhCommand) -> GhResult<CommandResult>;
}

pub struct SystemCommandRunner {
    binary: String,
}

impl SystemCommandRunner {
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

impl Default for SystemCommandRunner {
    fn default() -> Self {
        Self::new("gh")
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, command: GhCommand) -> GhResult<CommandResult> {
        let started = Instant::now();
        let mut process = Command::new(&self.binary);
        process
            .args(&command.args)
            .stdin(if command.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = process
            .spawn()
            .map_err(|err| map_spawn_error(err, command.class))?;

        if let Some(input) = command.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&input).map_err(|err| {
                    GhError::Internal(format!(
                        "failed writing stdin for {}: {err}",
                        command.class.as_str()
                    ))
                })?;
            }
        }

        let stdout_handle = spawn_pipe_reader(child.stdout.take());
        let stderr_handle = spawn_pipe_reader(child.stderr.take());

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = collect_pipe_output(stdout_handle)?;
                    let stderr = collect_pipe_output(stderr_handle)?;
                    log_command_completion(command.class, started.elapsed(), status.code());
                    if status.success() {
                        return Ok(CommandResult {
                            stdout,
                            stderr,
                            code: status.code(),
                        });
                    }
                    return Err(map_nonzero_exit(
                        command.class,
                        status.code(),
                        &stderr,
                        command.repo_hint,
                        command.pr_number,
                    ));
                }
                Ok(None) => {
                    if started.elapsed() >= command.timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = collect_pipe_output(stdout_handle);
                        let _ = collect_pipe_output(stderr_handle);
                        return Err(GhError::CommandTimeout {
                            class: command.class,
                            timeout: command.timeout,
                        });
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = collect_pipe_output(stdout_handle);
                    let _ = collect_pipe_output(stderr_handle);
                    return Err(GhError::Internal(format!(
                        "failed waiting on {}: {err}",
                        command.class.as_str()
                    )));
                }
            }
        }
    }
}

fn map_spawn_error(error: io::Error, class: CommandClass) -> GhError {
    if error.kind() == io::ErrorKind::NotFound {
        return GhError::GhNotInstalled;
    }
    GhError::Internal(format!("failed spawning {}: {error}", class.as_str()))
}

fn map_nonzero_exit(
    class: CommandClass,
    code: Option<i32>,
    stderr: &str,
    repo_hint: Option<String>,
    pr_number: Option<u64>,
) -> GhError {
    let stderr_lower = stderr.to_ascii_lowercase();
    if stderr_lower.contains("not logged into")
        || stderr_lower.contains("authenticate")
        || stderr_lower.contains("gh auth login")
    {
        return GhError::NotAuthenticated;
    }

    if stderr_lower.contains("could not resolve to a repository")
        || stderr_lower.contains("repository not found")
        || stderr_lower.contains("not a git repository")
        || stderr_lower.contains("permission denied")
    {
        return GhError::RepositoryUnavailable {
            repo: repo_hint.unwrap_or_else(|| "unknown".to_string()),
        };
    }

    if stderr_lower.contains("pull request not found")
        || stderr_lower.contains("could not resolve to a pullrequest")
        || stderr_lower.contains("no pull requests found")
    {
        return GhError::PullRequestNotFound {
            number: pr_number.unwrap_or(0),
        };
    }

    GhError::CommandFailed {
        class,
        code,
        stderr: stderr.to_string(),
    }
}

fn log_command_completion(class: CommandClass, duration: Duration, code: Option<i32>) {
    println!(
        "[gh] class={} duration_ms={} exit_code={}",
        class.as_str(),
        duration.as_millis(),
        code.map_or_else(|| "unknown".to_string(), |value| value.to_string())
    );
}

fn spawn_pipe_reader(
    reader: Option<impl Read + Send + 'static>,
) -> Option<thread::JoinHandle<io::Result<Vec<u8>>>> {
    reader.map(|mut reader| {
        thread::spawn(move || {
            let mut output = Vec::new();
            reader.read_to_end(&mut output)?;
            Ok(output)
        })
    })
}

fn collect_pipe_output(
    handle: Option<thread::JoinHandle<io::Result<Vec<u8>>>>,
) -> GhResult<String> {
    let Some(handle) = handle else {
        return Ok(String::new());
    };

    let bytes = handle
        .join()
        .map_err(|_| GhError::Internal("command reader thread panicked".to_string()))?
        .map_err(|error| GhError::Internal(format!("failed reading command output: {error}")))?;

    Ok(String::from_utf8_lossy(&bytes).to_string())
}

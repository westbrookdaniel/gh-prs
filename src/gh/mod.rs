pub mod cache;
pub mod client;
pub mod models;

use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandClass {
    PreflightVersion,
    PreflightAuth,
    ResolveRepo,
    PullRequestSearch,
    PullRequestList,
    PullRequestDetail,
    PullRequestFiles,
    IssueComments,
    PullRequestReviews,
    PullRequestReviewComments,
    SubmitComment,
    SubmitReview,
}

impl CommandClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreflightVersion => "preflight.version",
            Self::PreflightAuth => "preflight.auth",
            Self::ResolveRepo => "repo.resolve",
            Self::PullRequestSearch => "pr.search",
            Self::PullRequestList => "pr.list",
            Self::PullRequestDetail => "pr.detail",
            Self::PullRequestFiles => "pr.files",
            Self::IssueComments => "pr.issue_comments",
            Self::PullRequestReviews => "pr.reviews",
            Self::PullRequestReviewComments => "pr.review_comments",
            Self::SubmitComment => "pr.comment",
            Self::SubmitReview => "pr.review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GhError {
    GhNotInstalled,
    NotAuthenticated,
    RepositoryUnavailable {
        repo: String,
    },
    PullRequestNotFound {
        number: u64,
    },
    CommandTimeout {
        class: CommandClass,
        timeout: Duration,
    },
    CommandFailed {
        class: CommandClass,
        code: Option<i32>,
        stderr: String,
    },
    ParseFailure {
        class: CommandClass,
        details: String,
    },
    InvalidInput {
        field: String,
        details: String,
    },
    Internal(String),
}

impl GhError {
    pub fn title(&self) -> &'static str {
        match self {
            Self::GhNotInstalled => "GitHub CLI Missing",
            Self::NotAuthenticated => "GitHub CLI Not Authenticated",
            Self::RepositoryUnavailable { .. } => "Repository Unavailable",
            Self::PullRequestNotFound { .. } => "Pull Request Not Found",
            Self::CommandTimeout { .. } => "GitHub CLI Timed Out",
            Self::CommandFailed { .. } => "GitHub CLI Command Failed",
            Self::ParseFailure { .. } => "Unexpected GitHub CLI Output",
            Self::InvalidInput { .. } => "Invalid Input",
            Self::Internal(_) => "Internal Error",
        }
    }

    pub fn remediation(&self) -> &'static str {
        match self {
            Self::GhNotInstalled => "Install `gh` and ensure it is available on PATH.",
            Self::NotAuthenticated => "Run `gh auth login` and retry.",
            Self::RepositoryUnavailable { .. } => {
                "Confirm the repo exists and your `gh` account has access."
            }
            Self::PullRequestNotFound { .. } => {
                "Check the PR number and verify it exists in the selected repository."
            }
            Self::CommandTimeout { .. } => {
                "Retry in a moment; if it persists, check network connectivity."
            }
            Self::CommandFailed { .. } => {
                "Review the command output details and fix the upstream `gh` issue."
            }
            Self::ParseFailure { .. } => "Update `gh` to a current version and retry the request.",
            Self::InvalidInput { .. } => "Correct the input and retry.",
            Self::Internal(_) => "Retry, and inspect logs if the issue continues.",
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::InvalidInput { .. } => 400,
            Self::PullRequestNotFound { .. } => 404,
            Self::GhNotInstalled
            | Self::NotAuthenticated
            | Self::RepositoryUnavailable { .. }
            | Self::CommandTimeout { .. } => 503,
            Self::CommandFailed { .. } | Self::ParseFailure { .. } => 502,
            Self::Internal(_) => 500,
        }
    }

    pub fn details(&self) -> Option<String> {
        match self {
            Self::CommandFailed {
                class,
                code,
                stderr,
            } => {
                let trimmed = stderr.trim();
                let stderr_line = if trimmed.is_empty() {
                    "(no stderr output)".to_string()
                } else {
                    trimmed.lines().next().unwrap_or_default().to_string()
                };
                Some(format!(
                    "class={} exit_code={} stderr={}",
                    class.as_str(),
                    code.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
                    stderr_line
                ))
            }
            Self::CommandTimeout { class, timeout } => Some(format!(
                "class={} timeout_ms={}",
                class.as_str(),
                timeout.as_millis()
            )),
            Self::ParseFailure { class, details } => {
                Some(format!("class={} details={details}", class.as_str()))
            }
            Self::InvalidInput { field, details } => Some(format!("{field}: {details}")),
            Self::Internal(details) => Some(details.clone()),
            Self::RepositoryUnavailable { repo } => Some(format!("repo={repo}")),
            Self::PullRequestNotFound { number } => Some(format!("number={number}")),
            Self::GhNotInstalled | Self::NotAuthenticated => None,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::GhNotInstalled => "The `gh` executable was not found on PATH.".to_string(),
            Self::NotAuthenticated => {
                "The current GitHub CLI session is not authenticated.".to_string()
            }
            Self::RepositoryUnavailable { repo } => {
                format!("Unable to access repository `{repo}`.")
            }
            Self::PullRequestNotFound { number } => {
                format!("Pull request #{number} was not found.")
            }
            Self::CommandTimeout { class, timeout } => format!(
                "The `{}` command timed out after {}s.",
                class.as_str(),
                timeout.as_secs()
            ),
            Self::CommandFailed {
                class,
                code,
                stderr,
            } => {
                let line = stderr.trim().lines().next().unwrap_or("unknown error");
                format!(
                    "The `{}` command failed with exit code {}: {}",
                    class.as_str(),
                    code.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
                    line
                )
            }
            Self::ParseFailure { class, details } => format!(
                "Failed to parse output from `{}`: {}",
                class.as_str(),
                details
            ),
            Self::InvalidInput { field, details } => format!("Invalid `{field}`: {details}"),
            Self::Internal(details) => format!("Internal error: {details}"),
        }
    }
}

impl fmt::Display for GhError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for GhError {}

pub type GhResult<T> = Result<T, GhError>;

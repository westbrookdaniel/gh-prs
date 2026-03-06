use std::time::Duration;

use crate::gh::{GhError, GhResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEvent {
    Approve,
    Comment,
    RequestChanges,
}

impl ReviewEvent {
    pub fn parse(value: &str) -> GhResult<Self> {
        match value.trim() {
            "approve" => Ok(Self::Approve),
            "comment" => Ok(Self::Comment),
            "request_changes" => Ok(Self::RequestChanges),
            _ => Err(GhError::InvalidInput {
                field: "event".to_string(),
                details: "expected approve|comment|request_changes".to_string(),
            }),
        }
    }

    pub fn gh_flag(self) -> &'static str {
        match self {
            Self::Approve => "--approve",
            Self::Comment => "--comment",
            Self::RequestChanges => "--request-changes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    pub fn parse(value: &str) -> GhResult<Self> {
        match value.trim() {
            "merge" => Ok(Self::Merge),
            "squash" => Ok(Self::Squash),
            "rebase" => Ok(Self::Rebase),
            _ => Err(GhError::InvalidInput {
                field: "method".to_string(),
                details: "expected merge|squash|rebase".to_string(),
            }),
        }
    }

    pub fn gh_flag(self) -> &'static str {
        match self {
            Self::Merge => "--merge",
            Self::Squash => "--squash",
            Self::Rebase => "--rebase",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestStateTransition {
    Close,
    Reopen,
    Ready,
}

impl PullRequestStateTransition {
    pub fn parse(value: &str) -> GhResult<Self> {
        match value.trim() {
            "close" => Ok(Self::Close),
            "reopen" => Ok(Self::Reopen),
            "ready" => Ok(Self::Ready),
            _ => Err(GhError::InvalidInput {
                field: "state".to_string(),
                details: "expected close|reopen|ready".to_string(),
            }),
        }
    }

    pub fn as_gh_subcommand(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::Reopen => "reopen",
            Self::Ready => "ready",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct GhCommand {
    pub class: crate::gh::CommandClass,
    pub args: Vec<String>,
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
    pub repo_hint: Option<String>,
    pub pr_number: Option<u64>,
}

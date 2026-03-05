use crate::gh::cache::{CachedOutput, cache_get, cache_invalidate_prefix, cache_set};
use crate::gh::models::{
    MAX_SEARCH_LIMIT, PreflightDiagnostics, PullRequestConversation, PullRequestDetail,
    PullRequestFile, PullRequestListItem, PullRequestReview, PullRequestReviewComment,
    PullRequestSearchItem, PullRequestStatus, RepoContext, parse_issue_comments,
    parse_preflight_auth, parse_pull_request_detail, parse_pull_request_files,
    parse_pull_request_list, parse_pull_request_review_comments, parse_pull_request_reviews,
    parse_repo_context,
};
use crate::gh::{CommandClass, GhError, GhResult};
use crate::search::SearchArgs;
use serde::Deserialize;
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_WRITE_BODY_BYTES: usize = 64 * 1024;
static CLIENT_NAMESPACE_COUNTER: AtomicU64 = AtomicU64::new(1);

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

    fn gh_flag(self) -> &'static str {
        match self {
            Self::Approve => "--approve",
            Self::Comment => "--comment",
            Self::RequestChanges => "--request-changes",
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
    pub class: CommandClass,
    pub args: Vec<String>,
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
    pub repo_hint: Option<String>,
    pub pr_number: Option<u64>,
}

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
                        "failed writing to stdin for {}: {err}",
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
                    let duration = started.elapsed();
                    let result = CommandResult {
                        stdout,
                        stderr: stderr.clone(),
                        code: status.code(),
                    };
                    log_command_completion(command.class, duration, result.code);

                    if status.success() {
                        return Ok(result);
                    }

                    return Err(map_nonzero_exit(
                        command.class,
                        result.code,
                        &result.stderr,
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
                        let duration = started.elapsed();
                        println!(
                            "[gh] class={} duration_ms={} result=timeout",
                            command.class.as_str(),
                            duration.as_millis()
                        );
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

pub struct GhClient {
    runner: Arc<dyn CommandRunner>,
    timeout: Duration,
    cache_namespace: String,
}

impl Clone for GhClient {
    fn clone(&self) -> Self {
        Self {
            runner: Arc::clone(&self.runner),
            timeout: self.timeout,
            cache_namespace: self.cache_namespace.clone(),
        }
    }
}

impl Default for GhClient {
    fn default() -> Self {
        Self {
            runner: Arc::new(SystemCommandRunner::default()),
            timeout: DEFAULT_COMMAND_TIMEOUT,
            cache_namespace: next_cache_namespace(),
        }
    }
}

impl GhClient {
    #[cfg(test)]
    pub fn with_runner(runner: Arc<dyn CommandRunner>, timeout: Duration) -> Self {
        Self {
            runner,
            timeout,
            cache_namespace: next_cache_namespace(),
        }
    }

    pub async fn run_raw_command(
        &self,
        class: CommandClass,
        args: Vec<String>,
    ) -> GhResult<CommandResult> {
        self.run_command(GhCommand {
            class,
            args,
            stdin: None,
            timeout: self.timeout,
            repo_hint: None,
            pr_number: None,
        })
        .await
    }

    async fn run_raw_command_with_context(
        &self,
        class: CommandClass,
        args: Vec<String>,
        repo_hint: Option<String>,
        pr_number: Option<u64>,
    ) -> GhResult<CommandResult> {
        self.run_command(GhCommand {
            class,
            args,
            stdin: None,
            timeout: self.timeout,
            repo_hint,
            pr_number,
        })
        .await
    }

    async fn run_raw_command_with_stdin(
        &self,
        class: CommandClass,
        args: Vec<String>,
        stdin: Vec<u8>,
        repo_hint: Option<String>,
        pr_number: Option<u64>,
    ) -> GhResult<CommandResult> {
        self.run_command(GhCommand {
            class,
            args,
            stdin: Some(stdin),
            timeout: self.timeout,
            repo_hint,
            pr_number,
        })
        .await
    }

    async fn run_command(&self, command: GhCommand) -> GhResult<CommandResult> {
        let runner = Arc::clone(&self.runner);
        smol::unblock(move || runner.run(command)).await
    }

    fn cache_key(&self, key: &str) -> String {
        format!("{}:{key}", self.cache_namespace)
    }

    pub async fn preflight(&self) -> GhResult<PreflightDiagnostics> {
        let version_cache_key = self.cache_key("preflight|version");
        let version_result = if let Some(cached) = cache_get(&version_cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PreflightVersion.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command(
                    CommandClass::PreflightVersion,
                    vec!["--version".to_string()],
                )
                .await?;
            cache_set(
                &version_cache_key,
                120,
                CachedOutput::from_result(&computed),
            );
            println!(
                "[gh] class={} cache=store",
                CommandClass::PreflightVersion.as_str()
            );
            computed
        };

        let gh_version = version_result
            .stdout
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();

        if gh_version.is_empty() {
            return Err(GhError::ParseFailure {
                class: CommandClass::PreflightVersion,
                details: "missing version output".to_string(),
            });
        }

        let auth_cache_key = self.cache_key("preflight|auth");
        let auth_result = if let Some(cached) = cache_get(&auth_cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PreflightAuth.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command(
                    CommandClass::PreflightAuth,
                    vec![
                        "auth".to_string(),
                        "status".to_string(),
                        "--json".to_string(),
                        "hosts".to_string(),
                    ],
                )
                .await?;
            cache_set(&auth_cache_key, 120, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::PreflightAuth.as_str()
            );
            computed
        };

        let authenticated_hosts =
            parse_preflight_auth(&auth_result.stdout).map_err(|details| GhError::ParseFailure {
                class: CommandClass::PreflightAuth,
                details,
            })?;

        if authenticated_hosts.is_empty() {
            return Err(GhError::NotAuthenticated);
        }

        Ok(PreflightDiagnostics {
            gh_version,
            authenticated_hosts,
        })
    }

    pub async fn resolve_repo(&self, explicit_repo: Option<&str>) -> GhResult<RepoContext> {
        let (args, cache_key) = if let Some(repo) = explicit_repo {
            let repo = validate_repo_identifier(repo)?;
            (
                vec![
                    "repo".to_string(),
                    "view".to_string(),
                    repo.clone(),
                    "--json".to_string(),
                    "nameWithOwner,url,viewerPermission,defaultBranchRef".to_string(),
                ],
                self.cache_key(&format!("repo|resolve|{repo}")),
            )
        } else {
            (
                vec![
                    "repo".to_string(),
                    "view".to_string(),
                    "--json".to_string(),
                    "nameWithOwner,url,viewerPermission,defaultBranchRef".to_string(),
                ],
                self.cache_key("repo|resolve|context"),
            )
        };

        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::ResolveRepo.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command(CommandClass::ResolveRepo, args)
                .await?;
            cache_set(&cache_key, 120, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::ResolveRepo.as_str()
            );
            computed
        };

        parse_repo_context(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::ResolveRepo,
            details,
        })
    }

    pub async fn list_pull_requests(&self, repo: &str) -> GhResult<Vec<PullRequestListItem>> {
        let repo = validate_repo_identifier(repo)?;
        let cache_key = self.cache_key(&format!("pr|list|{repo}"));
        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PullRequestList.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command(
                    CommandClass::PullRequestList,
                    vec![
                        "pr".to_string(),
                        "list".to_string(),
                        "-R".to_string(),
                        repo.clone(),
                        "--state".to_string(),
                        "all".to_string(),
                        "-L".to_string(),
                        "100".to_string(),
                        "--json".to_string(),
                        "number,title,state,isDraft,author,createdAt,updatedAt,url,reviewDecision,reviewRequests,comments".to_string(),
                    ],
                )
                .await?;
            cache_set(&cache_key, 90, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::PullRequestList.as_str()
            );
            computed
        };

        parse_pull_request_list(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::PullRequestList,
            details,
        })
    }

    pub async fn accessible_repositories(&self) -> GhResult<Vec<String>> {
        let cache_key = self.cache_key("repo|accessible");
        if let Some(cached) = cache_get(&cache_key) {
            let repos = cached
                .stdout
                .lines()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<String>>();
            return Ok(repos);
        }

        let mut owners = vec!["@me".to_string()];
        if let Ok(orgs_result) = self
            .run_raw_command(
                CommandClass::RepoList,
                vec!["org".to_string(), "list".to_string(), "--limit".to_string(), "100".to_string()],
            )
            .await
        {
            for org in orgs_result
                .stdout
                .lines()
                .map(str::trim)
                .filter(|value| {
                    !value.is_empty()
                        && value
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
                })
            {
                owners.push(org.to_string());
            }
        }

        owners.sort();
        owners.dedup();

        let mut repos = Vec::new();
        for owner in owners {
            let args = vec![
                "repo".to_string(),
                "list".to_string(),
                owner,
                "--limit".to_string(),
                "200".to_string(),
                "--json".to_string(),
                "nameWithOwner,isArchived".to_string(),
            ];

            let Ok(result) = self.run_raw_command(CommandClass::RepoList, args).await else {
                continue;
            };

            if let Ok(mut values) = parse_accessible_repositories(&result.stdout) {
                repos.append(&mut values);
            }
        }

        repos.sort();
        repos.dedup();

        cache_set(
            &cache_key,
            300,
            CachedOutput {
                stdout: repos.join("\n"),
                stderr: String::new(),
                code: Some(0),
            },
        );

        Ok(repos)
    }

    pub async fn search_pull_requests(
        &self,
        query: &SearchArgs,
    ) -> GhResult<Vec<PullRequestSearchItem>> {
        let limit = query.limit.clamp(1, MAX_SEARCH_LIMIT);

        let mut args = vec![
            "search".to_string(),
            "prs".to_string(),
            "--limit".to_string(),
            limit.to_string(),
            "--json".to_string(),
            "number,title,state,isDraft,author,createdAt,updatedAt,url,repository,commentsCount"
                .to_string(),
            "--sort".to_string(),
            query.sort.as_query_value().to_string(),
            "--order".to_string(),
            query.order.as_query_value().to_string(),
        ];

        let mut repos = query.repos.clone();
        if repos.is_empty() {
            if let Some(repo) = &query.repo {
                repos.push(repo.clone());
            }
        }

        if repos.is_empty() {
            args.push("--owner".to_string());
            args.push(query.org.clone().unwrap_or_else(|| "@me".to_string()));
        }

        for repo in &repos {
            args.push("--repo".to_string());
            args.push(repo.clone());
        }

        if query.status != PullRequestStatus::All {
            args.push("--state".to_string());
            match query.status {
                PullRequestStatus::Open => args.push("open".to_string()),
                PullRequestStatus::Closed => args.push("closed".to_string()),
                PullRequestStatus::Merged => args.push("closed".to_string()),
                PullRequestStatus::All => {}
            }
        }

        if let Some(author) = &query.author {
            args.push("--author".to_string());
            args.push(author.clone());
        }

        if let Some(title) = &query.title {
            args.push("--match".to_string());
            args.push("title".to_string());
            args.push(title.clone());
        }

        if query.status == PullRequestStatus::Merged {
            args.push("--merged".to_string());
        }

        let cache_key = self.cache_key(&format!(
            "pr|search|{}|{}|{}|{}|{}|{}|{}",
            query.org.as_deref().unwrap_or("@me"),
            if repos.is_empty() {
                "-".to_string()
            } else {
                repos.join(",")
            },
            query.status.as_query_value(),
            query.title.as_deref().unwrap_or("-"),
            query.author.as_deref().unwrap_or("-"),
            query.sort.as_query_value(),
            query.order.as_query_value(),
        ));

        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PullRequestSearch.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command(CommandClass::PullRequestSearch, args)
                .await?;
            cache_set(&cache_key, 90, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::PullRequestSearch.as_str()
            );
            computed
        };

        parse_search_items(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::PullRequestSearch,
            details,
        })
    }

    pub async fn pull_request_conversation(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<PullRequestConversation> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;

        let detail = self.pull_request_detail(&repo, number).await?;

        let issue_comments = self.issue_comments(&repo, number).await?;
        let reviews = self.pull_request_reviews(&repo, number).await?;
        let review_comments = self.pull_request_review_comments(&repo, number).await?;

        Ok(PullRequestConversation {
            detail,
            issue_comments,
            reviews,
            review_comments,
        })
    }

    pub async fn pull_request_files(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<PullRequestFile>> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;

        let cache_key = self.cache_key(&format!("pr|files|{repo}|{number}"));

        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PullRequestFiles.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command_with_context(
                    CommandClass::PullRequestFiles,
                    vec![
                        "api".to_string(),
                        format!("repos/{repo}/pulls/{number}/files?per_page=100"),
                    ],
                    Some(repo.clone()),
                    Some(number),
                )
                .await?;
            cache_set(&cache_key, 45, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::PullRequestFiles.as_str()
            );
            computed
        };

        parse_pull_request_files(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::PullRequestFiles,
            details,
        })
    }

    pub async fn submit_comment(&self, repo: &str, number: u64, body: &str) -> GhResult<()> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;
        let body = normalize_write_body(body)?;

        self.run_raw_command_with_stdin(
            CommandClass::SubmitComment,
            vec![
                "pr".to_string(),
                "comment".to_string(),
                number.to_string(),
                "-R".to_string(),
                repo.clone(),
                "--body-file".to_string(),
                "-".to_string(),
            ],
            body.into_bytes(),
            Some(repo.clone()),
            Some(number),
        )
        .await?;

        cache_invalidate_prefix(&self.cache_key("pr|"));

        Ok(())
    }

    pub async fn submit_review(
        &self,
        repo: &str,
        number: u64,
        event: ReviewEvent,
        body: &str,
    ) -> GhResult<()> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;
        let body = normalize_write_body(body)?;

        self.run_raw_command_with_stdin(
            CommandClass::SubmitReview,
            vec![
                "pr".to_string(),
                "review".to_string(),
                number.to_string(),
                "-R".to_string(),
                repo.clone(),
                event.gh_flag().to_string(),
                "--body-file".to_string(),
                "-".to_string(),
            ],
            body.into_bytes(),
            Some(repo.clone()),
            Some(number),
        )
        .await?;

        cache_invalidate_prefix(&self.cache_key("pr|"));

        Ok(())
    }

    async fn pull_request_detail(&self, repo: &str, number: u64) -> GhResult<PullRequestDetail> {
        let cache_key = self.cache_key(&format!("pr|detail|{repo}|{number}"));

        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PullRequestDetail.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command_with_context(
                    CommandClass::PullRequestDetail,
                    vec![
                        "pr".to_string(),
                        "view".to_string(),
                        number.to_string(),
                        "-R".to_string(),
                        repo.to_string(),
                        "--json".to_string(),
                        "number,title,body,state,isDraft,author,createdAt,updatedAt,url,baseRefName,headRefName,mergeStateStatus,mergeable,reviewDecision,reviewRequests,latestReviews,statusCheckRollup,commits,files,comments".to_string(),
                    ],
                    Some(repo.to_string()),
                    Some(number),
                )
                .await?;
            cache_set(&cache_key, 45, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::PullRequestDetail.as_str()
            );
            computed
        };

        parse_pull_request_detail(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::PullRequestDetail,
            details,
        })
    }

    async fn issue_comments(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<crate::gh::models::IssueComment>> {
        let cache_key = self.cache_key(&format!("pr|issue_comments|{repo}|{number}"));

        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::IssueComments.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command_with_context(
                    CommandClass::IssueComments,
                    vec![
                        "api".to_string(),
                        format!("repos/{repo}/issues/{number}/comments?per_page=100"),
                    ],
                    Some(repo.to_string()),
                    Some(number),
                )
                .await?;
            cache_set(&cache_key, 45, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::IssueComments.as_str()
            );
            computed
        };

        parse_issue_comments(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::IssueComments,
            details,
        })
    }

    async fn pull_request_reviews(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<PullRequestReview>> {
        let cache_key = self.cache_key(&format!("pr|reviews|{repo}|{number}"));

        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PullRequestReviews.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command_with_context(
                    CommandClass::PullRequestReviews,
                    vec![
                        "api".to_string(),
                        format!("repos/{repo}/pulls/{number}/reviews?per_page=100"),
                    ],
                    Some(repo.to_string()),
                    Some(number),
                )
                .await?;
            cache_set(&cache_key, 45, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::PullRequestReviews.as_str()
            );
            computed
        };

        parse_pull_request_reviews(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::PullRequestReviews,
            details,
        })
    }

    async fn pull_request_review_comments(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<PullRequestReviewComment>> {
        let cache_key = self.cache_key(&format!("pr|review_comments|{repo}|{number}"));

        let result = if let Some(cached) = cache_get(&cache_key) {
            println!(
                "[gh] class={} cache=hit",
                CommandClass::PullRequestReviewComments.as_str()
            );
            CommandResult {
                stdout: cached.stdout,
                stderr: cached.stderr,
                code: cached.code,
            }
        } else {
            let computed = self
                .run_raw_command_with_context(
                    CommandClass::PullRequestReviewComments,
                    vec![
                        "api".to_string(),
                        format!("repos/{repo}/pulls/{number}/comments?per_page=100"),
                    ],
                    Some(repo.to_string()),
                    Some(number),
                )
                .await?;
            cache_set(&cache_key, 45, CachedOutput::from_result(&computed));
            println!(
                "[gh] class={} cache=store",
                CommandClass::PullRequestReviewComments.as_str()
            );
            computed
        };

        parse_pull_request_review_comments(&result.stdout).map_err(|details| {
            GhError::ParseFailure {
                class: CommandClass::PullRequestReviewComments,
                details,
            }
        })
    }
}

fn next_cache_namespace() -> String {
    let id = CLIENT_NAMESPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("gh-client-{id}")
}

fn map_spawn_error(err: io::Error, class: CommandClass) -> GhError {
    if err.kind() == io::ErrorKind::NotFound {
        return GhError::GhNotInstalled;
    }
    GhError::Internal(format!("failed spawning {}: {err}", class.as_str()))
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
        .map_err(|err| GhError::Internal(format!("failed reading command output: {err}")))?;

    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn validate_repo_identifier(repo: &str) -> GhResult<String> {
    let repo = repo.trim();
    let (owner, name) = repo.split_once('/').ok_or_else(|| GhError::InvalidInput {
        field: "repo".to_string(),
        details: "expected OWNER/REPO".to_string(),
    })?;

    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return Err(GhError::InvalidInput {
            field: "repo".to_string(),
            details: "expected OWNER/REPO".to_string(),
        });
    }

    if !owner
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(GhError::InvalidInput {
            field: "repo".to_string(),
            details: "owner contains invalid characters".to_string(),
        });
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err(GhError::InvalidInput {
            field: "repo".to_string(),
            details: "repo contains invalid characters".to_string(),
        });
    }

    Ok(format!("{owner}/{name}"))
}

fn validate_pr_number(number: u64) -> GhResult<u64> {
    if number == 0 {
        return Err(GhError::InvalidInput {
            field: "number".to_string(),
            details: "must be greater than zero".to_string(),
        });
    }
    Ok(number)
}

fn normalize_write_body(body: &str) -> GhResult<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(GhError::InvalidInput {
            field: "body".to_string(),
            details: "body cannot be empty".to_string(),
        });
    }

    if body.len() > MAX_WRITE_BODY_BYTES {
        return Err(GhError::InvalidInput {
            field: "body".to_string(),
            details: format!("body must be <= {} bytes", MAX_WRITE_BODY_BYTES),
        });
    }

    Ok(body.to_string())
}

#[derive(Debug, Deserialize)]
struct SearchRepositoryRaw {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SearchUserRaw {
    login: Option<String>,
    name: Option<String>,
    #[serde(rename = "avatarUrl")]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PullRequestSearchItemRaw {
    number: u64,
    title: String,
    state: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    author: Option<SearchUserRaw>,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    url: String,
    #[serde(rename = "commentsCount")]
    comments_count: Option<usize>,
    repository: Option<SearchRepositoryRaw>,
}

#[derive(Debug, Deserialize)]
struct AccessibleRepositoryRaw {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
    #[serde(rename = "isArchived", default)]
    is_archived: bool,
}

fn parse_search_items(json: &str) -> Result<Vec<PullRequestSearchItem>, String> {
    let raw_items: Vec<PullRequestSearchItemRaw> =
        serde_json::from_str(json).map_err(|err| err.to_string())?;

    Ok(raw_items
        .into_iter()
        .map(|raw| {
            let repository_name_with_owner = raw
                .repository
                .and_then(|repo| repo.name_with_owner.or(repo.name))
                .unwrap_or_else(|| "unknown/unknown".to_string());

            PullRequestSearchItem {
                repository_name_with_owner,
                number: raw.number,
                title: raw.title,
                state: raw.state.to_ascii_uppercase(),
                is_draft: raw.is_draft,
                author: extract_search_user(raw.author.clone()),
                author_avatar_url: extract_search_user_avatar_url(raw.author),
                created_at: raw.created_at,
                updated_at: raw.updated_at,
                url: raw.url,
                comment_count: raw.comments_count.unwrap_or(0),
            }
        })
        .collect())
}

fn parse_accessible_repositories(json: &str) -> Result<Vec<String>, String> {
    let values: Vec<AccessibleRepositoryRaw> =
        serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(values
        .into_iter()
        .filter(|repo| !repo.is_archived)
        .map(|repo| repo.name_with_owner)
        .collect())
}

fn extract_search_user(user: Option<SearchUserRaw>) -> String {
    user.and_then(|candidate| candidate.login.or(candidate.name))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn extract_search_user_avatar_url(user: Option<SearchUserRaw>) -> String {
    user.and_then(|candidate| candidate.avatar_url)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        CommandResult, CommandRunner, DEFAULT_COMMAND_TIMEOUT, GhClient, GhCommand, ReviewEvent,
    };
    use crate::gh::models::{PullRequestOrder, PullRequestSort, PullRequestStatus};
    use crate::gh::{CommandClass, GhError};
    use crate::search::SearchArgs;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Duration;

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock() -> &'static Mutex<()> {
        TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[derive(Default)]
    struct MockRunner {
        responses: Mutex<VecDeque<Result<CommandResult, GhError>>>,
        seen: Mutex<Vec<GhCommand>>,
    }

    impl MockRunner {
        fn with_responses(responses: Vec<Result<CommandResult, GhError>>) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn seen_commands(&self) -> Vec<GhCommand> {
            self.seen.lock().expect("seen lock").clone()
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, command: GhCommand) -> Result<CommandResult, GhError> {
            self.seen.lock().expect("seen lock").push(command);
            self.responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .unwrap_or_else(|| {
                    Err(GhError::Internal(
                        "missing mock response for command".to_string(),
                    ))
                })
        }
    }

    fn ok(stdout: &str) -> Result<CommandResult, GhError> {
        Ok(CommandResult {
            stdout: stdout.to_string(),
            stderr: String::new(),
            code: Some(0),
        })
    }

    #[test]
    fn review_event_parser_accepts_known_values() {
        let _guard = test_lock().lock().expect("test lock");
        assert_eq!(
            ReviewEvent::parse("approve").expect("approve"),
            ReviewEvent::Approve
        );
        assert_eq!(
            ReviewEvent::parse("comment").expect("comment"),
            ReviewEvent::Comment
        );
        assert_eq!(
            ReviewEvent::parse("request_changes").expect("request changes"),
            ReviewEvent::RequestChanges
        );
        assert!(ReviewEvent::parse("bad").is_err());
    }

    #[test]
    fn preflight_requires_authenticated_hosts() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![
                ok("gh version 2.72.0"),
                ok(r#"{"hosts":{}}"#),
            ]));
            let client = GhClient::with_runner(runner, DEFAULT_COMMAND_TIMEOUT);

            let result = client.preflight().await;
            assert!(matches!(result, Err(GhError::NotAuthenticated)));
        });
    }

    #[test]
    fn list_command_uses_expected_arguments() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );

            let result = client.list_pull_requests("acme/widgets").await;
            assert!(result.is_ok());

            let commands = runner.seen_commands();
            assert_eq!(commands.len(), 1);
            let command = &commands[0];
            assert_eq!(command.class, CommandClass::PullRequestList);
            assert_eq!(
                command.args,
                vec![
                    "pr",
                    "list",
                    "-R",
                    "acme/widgets",
                    "--state",
                    "all",
                    "-L",
                    "100",
                    "--json",
                    "number,title,state,isDraft,author,createdAt,updatedAt,url,reviewDecision,reviewRequests,comments",
                ]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<String>>()
            );
        });
    }

    #[test]
    fn detail_path_runs_all_conversation_commands() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![
                ok(r#"{
                    "number":1,
                    "title":"T",
                    "body":"",
                    "state":"OPEN",
                    "isDraft":false,
                    "author":{"login":"alice"},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-01T00:00:00Z",
                    "url":"https://example/pr/1",
                    "baseRefName":"main",
                    "headRefName":"feat",
                    "mergeStateStatus":"CLEAN",
                    "mergeable":"MERGEABLE",
                    "reviewDecision":null,
                    "reviewRequests":[],
                    "latestReviews":[],
                    "statusCheckRollup":null,
                    "commits":{"totalCount":1},
                    "files":{"totalCount":2}
                }"#),
                ok("[]"),
                ok("[]"),
                ok("[]"),
            ]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );

            let result = client.pull_request_conversation("acme/widgets", 1).await;
            assert!(result.is_ok());

            let classes = runner
                .seen_commands()
                .into_iter()
                .map(|command| command.class)
                .collect::<Vec<CommandClass>>();
            assert_eq!(
                classes,
                vec![
                    CommandClass::PullRequestDetail,
                    CommandClass::IssueComments,
                    CommandClass::PullRequestReviews,
                    CommandClass::PullRequestReviewComments,
                ]
            );
        });
    }

    #[test]
    fn write_actions_send_body_over_stdin() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok(""), ok("")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );

            client
                .submit_comment("acme/widgets", 44, "hello from ui")
                .await
                .expect("comment should succeed");
            client
                .submit_review("acme/widgets", 44, ReviewEvent::Approve, "ship it")
                .await
                .expect("review should succeed");

            let commands = runner.seen_commands();
            assert_eq!(commands.len(), 2);
            assert_eq!(
                commands[0].stdin.as_deref(),
                Some("hello from ui".as_bytes())
            );
            assert!(commands[1].args.contains(&"--approve".to_string()));
            assert_eq!(commands[1].stdin.as_deref(), Some("ship it".as_bytes()));
        });
    }

    #[test]
    fn nonzero_exit_maps_to_repo_unavailable() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![Err(
                GhError::RepositoryUnavailable {
                    repo: "acme/widgets".to_string(),
                },
            )]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );

            let result = client.resolve_repo(Some("acme/widgets")).await;
            assert!(matches!(
                result,
                Err(GhError::RepositoryUnavailable { repo }) if repo == "acme/widgets"
            ));
        });
    }

    #[test]
    fn timeout_error_propagates() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![Err(
                GhError::CommandTimeout {
                    class: CommandClass::PullRequestList,
                    timeout: Duration::from_secs(2),
                },
            )]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                Duration::from_secs(2),
            );

            let result = client.list_pull_requests("acme/widgets").await;
            assert!(matches!(
                result,
                Err(GhError::CommandTimeout {
                    class: CommandClass::PullRequestList,
                    ..
                })
            ));
        });
    }

    #[test]
    fn validate_repo_and_body_inputs() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );

            let bad_repo = client.list_pull_requests("not-valid").await;
            assert!(
                matches!(bad_repo, Err(GhError::InvalidInput { field, .. }) if field == "repo")
            );

            let bad_body = client.submit_comment("acme/widgets", 1, "   ").await;
            assert!(
                matches!(bad_body, Err(GhError::InvalidInput { field, .. }) if field == "body")
            );
        });
    }

    #[test]
    fn search_command_uses_query_filters() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );
            let query = SearchArgs {
                org: Some("westbrookdaniel".to_string()),
                repos: vec!["westbrookdaniel/blogs".to_string()],
                repo: Some("westbrookdaniel/blogs".to_string()),
                status: PullRequestStatus::Open,
                title: Some("security".to_string()),
                author: Some("alice".to_string()),
                sort: PullRequestSort::Updated,
                order: PullRequestOrder::Desc,
                limit: 100,
                view: None,
            };

            let result = client.search_pull_requests(&query).await;
            assert!(result.is_ok());

            let command = runner
                .seen_commands()
                .into_iter()
                .next()
                .expect("one command");
            assert_eq!(command.class, CommandClass::PullRequestSearch);
            assert!(command.args.contains(&"--repo".to_string()));
            assert!(command.args.contains(&"westbrookdaniel/blogs".to_string()));
            assert!(command.args.contains(&"--state".to_string()));
            assert!(command.args.contains(&"open".to_string()));
            assert!(command.args.contains(&"--author".to_string()));
            assert!(command.args.contains(&"alice".to_string()));
            assert!(command.args.contains(&"--match".to_string()));
            assert!(command.args.contains(&"title".to_string()));
            assert!(command.args.contains(&"security".to_string()));
        });
    }

    #[test]
    fn merged_filter_adds_merged_flag() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );
            let query = SearchArgs {
                status: PullRequestStatus::Merged,
                ..SearchArgs::default()
            };

            let _ = client.search_pull_requests(&query).await;
            let command = runner
                .seen_commands()
                .into_iter()
                .next()
                .expect("one command");
            assert!(command.args.contains(&"@me".to_string()));
            assert!(command.args.contains(&"--merged".to_string()));
            assert!(command.args.contains(&"closed".to_string()));
        });
    }

    #[test]
    fn submit_comment_invalidates_read_cache() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]"), ok(""), ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );

            let query = SearchArgs::default();
            let _ = client.search_pull_requests(&query).await;
            let _ = client.submit_comment("acme/widgets", 10, "hello").await;
            let _ = client.search_pull_requests(&query).await;

            let classes = runner
                .seen_commands()
                .into_iter()
                .map(|command| command.class)
                .collect::<Vec<CommandClass>>();
            assert_eq!(
                classes,
                vec![
                    CommandClass::PullRequestSearch,
                    CommandClass::SubmitComment,
                    CommandClass::PullRequestSearch,
                ]
            );
        });
    }

    #[test]
    fn pull_request_files_command_uses_rest_api_endpoint() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                DEFAULT_COMMAND_TIMEOUT,
            );

            let _ = client.pull_request_files("acme/widgets", 8).await;
            let command = runner
                .seen_commands()
                .into_iter()
                .next()
                .expect("one command");
            assert_eq!(command.class, CommandClass::PullRequestFiles);
            assert!(
                command
                    .args
                    .contains(&"repos/acme/widgets/pulls/8/files?per_page=100".to_string())
            );
        });
    }

    #[test]
    fn parses_accessible_repositories_payload() {
        let payload = r#"[
            {"nameWithOwner":"acme/widgets","isArchived":false},
            {"nameWithOwner":"acme/legacy","isArchived":true}
        ]"#;

        let parsed = super::parse_accessible_repositories(payload).expect("should parse");
        assert_eq!(parsed, vec!["acme/widgets".to_string()]);
    }
}

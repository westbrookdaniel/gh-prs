use crate::cache_store::SqliteCacheStore;
pub use crate::gh::commands::{
    CommandResult, GhCommand, MergeMethod, PullRequestStateTransition, ReviewEvent,
};
use crate::gh::models::{
    MAX_SEARCH_LIMIT, PreflightDiagnostics, PullRequestConversation, PullRequestFile,
    PullRequestSearchItem, PullRequestStatus, RepoContext,
};
pub use crate::gh::runner::CommandRunner;
use crate::gh::runner::SystemCommandRunner;
use crate::gh::validation::{validate_pr_number, validate_repo_identifier};
use crate::gh::{CommandClass, GhError, GhResult};
use crate::gh_parsing::{parse_preflight_auth, parse_pull_request_files, parse_repo_context};
use crate::search::SearchArgs;
use serde::{Serialize, de::DeserializeOwned};
use std::future::Future;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(12);
const CACHE_NAMESPACE: &str = "gh:v1";

const CACHE_PREFLIGHT: CachePolicy = CachePolicy::seconds(120, 120);
const CACHE_REPO_RESOLVE: CachePolicy = CachePolicy::seconds(120, 300);
const CACHE_REPO_LIST: CachePolicy = CachePolicy::seconds(90, 600);
const CACHE_SEARCH: CachePolicy = CachePolicy::seconds(45, 300);
const CACHE_CONVERSATION: CachePolicy = CachePolicy::seconds(45, 300);
const CACHE_FILES: CachePolicy = CachePolicy::seconds(45, 300);

mod pull_request_ops;

#[derive(Debug, Clone)]
pub struct CachedValue<T> {
    pub value: T,
    pub is_stale: bool,
}

#[derive(Debug, Clone, Copy)]
struct CachePolicy {
    stale_after: Duration,
    ttl: Duration,
}

impl CachePolicy {
    const fn seconds(stale_after: u64, ttl: u64) -> Self {
        Self {
            stale_after: Duration::from_secs(stale_after),
            ttl: Duration::from_secs(ttl),
        }
    }
}

#[derive(Clone)]
pub struct GhClient {
    runner: Arc<dyn CommandRunner>,
    timeout: Duration,
    cache: Arc<SqliteCacheStore>,
}

impl GhClient {
    pub fn new() -> io::Result<Self> {
        let cache = SqliteCacheStore::open_default()?;
        Ok(Self {
            runner: Arc::new(SystemCommandRunner::default()),
            timeout: DEFAULT_COMMAND_TIMEOUT,
            cache: Arc::new(cache),
        })
    }

    #[cfg(test)]
    pub fn with_runner(runner: Arc<dyn CommandRunner>, timeout: Duration) -> Self {
        let cache = SqliteCacheStore::open(test_cache_db_path()).expect("open test sqlite cache");
        Self {
            runner,
            timeout,
            cache: Arc::new(cache),
        }
    }

    pub fn cache_db_path(&self) -> &Path {
        self.cache.db_path()
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

    pub async fn preflight(&self) -> GhResult<PreflightDiagnostics> {
        self.cached_or_refresh("preflight|diagnostics", CACHE_PREFLIGHT, || async {
            let version = self
                .run_raw_command(
                    CommandClass::PreflightVersion,
                    vec!["--version".to_string()],
                )
                .await?;
            let gh_version = version
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

            let auth = self
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

            let authenticated_hosts =
                parse_preflight_auth(&auth.stdout).map_err(|details| GhError::ParseFailure {
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
        })
        .await
    }

    pub async fn resolve_repo(&self, explicit_repo: Option<&str>) -> GhResult<RepoContext> {
        let (repo_arg, cache_key) = if let Some(repo) = explicit_repo {
            let repo = validate_repo_identifier(repo)?;
            (Some(repo.clone()), format!("repo|resolve|{repo}"))
        } else {
            (None, "repo|resolve|context".to_string())
        };

        self.cached_or_refresh(&cache_key, CACHE_REPO_RESOLVE, || async {
            let mut args = vec![
                "repo".to_string(),
                "view".to_string(),
                "--json".to_string(),
                "nameWithOwner,url,viewerPermission,defaultBranchRef".to_string(),
            ];
            if let Some(repo) = repo_arg {
                args.insert(2, repo);
            }

            let result = self
                .run_raw_command(CommandClass::ResolveRepo, args)
                .await?;
            parse_repo_context(&result.stdout).map_err(|details| GhError::ParseFailure {
                class: CommandClass::ResolveRepo,
                details,
            })
        })
        .await
    }

    pub async fn cached_accessible_repositories(
        &self,
    ) -> GhResult<Option<CachedValue<Vec<String>>>> {
        self.cache_get("repo|accessible").await
    }

    pub async fn refresh_accessible_repositories(&self) -> GhResult<Vec<String>> {
        let mut owners = vec![String::new()];
        if let Ok(orgs_result) = self
            .run_raw_command(
                CommandClass::RepoList,
                vec![
                    "org".to_string(),
                    "list".to_string(),
                    "--limit".to_string(),
                    "100".to_string(),
                ],
            )
            .await
        {
            for org in orgs_result.stdout.lines().map(str::trim).filter(|value| {
                !value.is_empty()
                    && value
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
            }) {
                owners.push(org.to_string());
            }
        }

        owners.sort();
        owners.dedup();

        let mut repos = Vec::new();
        for owner in owners {
            let mut args = vec!["repo".to_string(), "list".to_string()];
            if !owner.is_empty() {
                args.push(owner);
            }
            args.extend([
                "--limit".to_string(),
                "200".to_string(),
                "--json".to_string(),
                "nameWithOwner,isArchived".to_string(),
            ]);

            let Ok(result) = self.run_raw_command(CommandClass::RepoList, args).await else {
                continue;
            };

            if let Ok(mut values) = parse_accessible_repositories(&result.stdout) {
                repos.append(&mut values);
            }
        }

        repos.sort();
        repos.dedup();
        self.cache_set("repo|accessible", CACHE_REPO_LIST, &repos)
            .await?;
        Ok(repos)
    }

    pub async fn cached_search_pull_requests(
        &self,
        query: &SearchArgs,
    ) -> GhResult<Option<CachedValue<Vec<PullRequestSearchItem>>>> {
        self.cache_get(&search_cache_key(query)).await
    }

    pub async fn refresh_search_pull_requests(
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
        if repos.is_empty() && let Some(repo) = &query.repo {
            repos.push(repo.clone());
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
            args.push(
                match query.status {
                    PullRequestStatus::Open => "open",
                    PullRequestStatus::Closed => "closed",
                    PullRequestStatus::Merged => "closed",
                    PullRequestStatus::All => "open",
                }
                .to_string(),
            );
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

        let result = self
            .run_raw_command(CommandClass::PullRequestSearch, args)
            .await?;
        let items =
            parse_search_items(&result.stdout).map_err(|details| GhError::ParseFailure {
                class: CommandClass::PullRequestSearch,
                details,
            })?;

        self.cache_set(&search_cache_key(query), CACHE_SEARCH, &items)
            .await?;
        Ok(items)
    }

    #[cfg(test)]
    pub async fn search_pull_requests(
        &self,
        query: &SearchArgs,
    ) -> GhResult<Vec<PullRequestSearchItem>> {
        if let Some(cached) = self.cached_search_pull_requests(query).await? {
            return Ok(cached.value);
        }

        self.refresh_search_pull_requests(query).await
    }

    pub async fn cached_pull_request_conversation(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Option<CachedValue<PullRequestConversation>>> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;
        self.cache_get(&format!("pr|conversation|{repo}|{number}"))
            .await
    }

    pub async fn refresh_pull_request_conversation(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<PullRequestConversation> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;

        let detail = self.fetch_pull_request_detail(&repo, number).await?;
        let issue_comments = self.fetch_issue_comments(&repo, number).await?;
        let reviews = self.fetch_pull_request_reviews(&repo, number).await?;
        let review_comments = self
            .fetch_pull_request_review_comments(&repo, number)
            .await?;

        let conversation = PullRequestConversation {
            detail,
            issue_comments,
            reviews,
            review_comments,
        };

        self.cache_set(
            &format!("pr|conversation|{repo}|{number}"),
            CACHE_CONVERSATION,
            &conversation,
        )
        .await?;

        Ok(conversation)
    }

    pub async fn cached_pull_request_files(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Option<CachedValue<Vec<PullRequestFile>>>> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;
        self.cache_get(&format!("pr|files|{repo}|{number}")).await
    }

    pub async fn refresh_pull_request_files(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<PullRequestFile>> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;

        let result = self
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

        let files =
            parse_pull_request_files(&result.stdout).map_err(|details| GhError::ParseFailure {
                class: CommandClass::PullRequestFiles,
                details,
            })?;

        self.cache_set(&format!("pr|files|{repo}|{number}"), CACHE_FILES, &files)
            .await?;
        Ok(files)
    }

    async fn cache_get<T: DeserializeOwned>(&self, key: &str) -> GhResult<Option<CachedValue<T>>> {
        let cache_key = self.cache_key(key);
        let Some(entry) = self
            .cache
            .get(&cache_key)
            .await
            .map_err(|err| GhError::Internal(format!("cache read failed for {key}: {err}")))?
        else {
            return Ok(None);
        };

        let value = match serde_json::from_slice::<T>(&entry.payload) {
            Ok(value) => value,
            Err(_) => {
                let _ = self.cache.invalidate_prefix(&cache_key).await;
                return Ok(None);
            }
        };

        Ok(Some(CachedValue {
            value,
            is_stale: entry.is_stale,
        }))
    }

    async fn cache_set<T: Serialize>(
        &self,
        key: &str,
        policy: CachePolicy,
        value: &T,
    ) -> GhResult<()> {
        let payload = serde_json::to_vec(value).map_err(|err| {
            GhError::Internal(format!("failed serializing cache payload for {key}: {err}"))
        })?;

        self.cache
            .set(
                &self.cache_key(key),
                &payload,
                policy.stale_after,
                policy.ttl,
            )
            .await
            .map_err(|err| GhError::Internal(format!("cache write failed for {key}: {err}")))
    }

    async fn cached_or_refresh<T, F, Fut>(
        &self,
        key: &str,
        policy: CachePolicy,
        refresh: F,
    ) -> GhResult<T>
    where
        T: Clone + Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: Future<Output = GhResult<T>>,
    {
        if let Some(cached) = self.cache_get::<T>(key).await? {
            return Ok(cached.value);
        }
        let value = refresh().await?;
        self.cache_set(key, policy, &value).await?;
        Ok(value)
    }

    fn cache_key(&self, key: &str) -> String {
        format!("{CACHE_NAMESPACE}:{key}")
    }
}

impl Default for GhClient {
    fn default() -> Self {
        match Self::new() {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(error = %error, "failed opening default sqlite cache; using fallback");
                let fallback = std::env::temp_dir().join("gh-prs").join("cache.db");
                let cache = SqliteCacheStore::open(fallback)
                    .expect("failed to create fallback sqlite cache store");
                Self {
                    runner: Arc::new(SystemCommandRunner::default()),
                    timeout: DEFAULT_COMMAND_TIMEOUT,
                    cache: Arc::new(cache),
                }
            }
        }
    }
}

fn search_cache_key(query: &SearchArgs) -> String {
    let mut repos = query.repos.clone();
    repos.sort();
    repos.dedup();
    format!(
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
    )
}

#[derive(Debug, serde::Deserialize)]
struct SearchRepositoryRaw {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SearchUserRaw {
    login: Option<String>,
    name: Option<String>,
    #[serde(rename = "avatarUrl")]
    avatar_url: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
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

#[derive(Debug, serde::Deserialize)]
struct AccessibleRepositoryRaw {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
    #[serde(rename = "isArchived", default)]
    is_archived: bool,
}

fn parse_search_items(json: &str) -> Result<Vec<PullRequestSearchItem>, String> {
    let raw_items: Vec<PullRequestSearchItemRaw> =
        serde_json::from_str(json).map_err(|error| error.to_string())?;

    Ok(raw_items
        .into_iter()
        .map(|raw| PullRequestSearchItem {
            repository_name_with_owner: raw
                .repository
                .and_then(|repo| repo.name_with_owner.or(repo.name))
                .unwrap_or_else(|| "unknown/unknown".to_string()),
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
        })
        .collect())
}

fn parse_accessible_repositories(json: &str) -> Result<Vec<String>, String> {
    let values: Vec<AccessibleRepositoryRaw> =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
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
fn test_cache_db_path() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir()
        .join(format!("gh-prs-gh-client-tests-{nanos}"))
        .join("cache.db")
}

#[cfg(test)]
mod tests {
    use super::{CommandResult, CommandRunner, GhClient, GhCommand, ReviewEvent};
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
                .unwrap_or_else(|| Err(GhError::Internal("missing mock response".to_string())))
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
    fn cached_search_short_circuits_second_network_call() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                Duration::from_secs(5),
            );

            let query = SearchArgs::default();
            let first = client.search_pull_requests(&query).await.expect("first");
            let second = client.search_pull_requests(&query).await.expect("second");

            assert!(first.is_empty());
            assert!(second.is_empty());
            assert_eq!(runner.seen_commands().len(), 1);
        });
    }

    #[test]
    fn merged_filter_adds_merged_flag() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                Duration::from_secs(5),
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
            assert!(command.args.contains(&"--merged".to_string()));
            assert!(command.args.contains(&"closed".to_string()));
        });
    }

    #[test]
    fn write_actions_send_stdin_and_invalidate_cached_search() {
        let _guard = test_lock().lock().expect("test lock");
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_responses(vec![ok("[]"), ok(""), ok("[]")]));
            let client = GhClient::with_runner(
                Arc::clone(&runner) as Arc<dyn CommandRunner>,
                Duration::from_secs(5),
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

            let _ = client
                .search_pull_requests(&query)
                .await
                .expect("initial search");
            client
                .submit_comment("acme/widgets", 44, "hello from ui")
                .await
                .expect("comment should succeed");
            let _ = client
                .search_pull_requests(&query)
                .await
                .expect("search after write");

            let commands = runner.seen_commands();
            assert_eq!(commands.len(), 3);
            assert_eq!(commands[1].class, CommandClass::SubmitComment);
            assert_eq!(
                commands[1].stdin.as_deref(),
                Some("hello from ui".as_bytes())
            );
            assert_eq!(commands[2].class, CommandClass::PullRequestSearch);
        });
    }
}

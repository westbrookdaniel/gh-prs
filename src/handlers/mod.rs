pub mod changes;
pub mod context;
pub mod detail;
pub mod format;
pub mod forms;
pub mod health;
pub mod list;
pub mod load;
pub mod not_found;
pub mod routes;
pub mod state;
pub mod write;

pub use routes::register;
pub use state::AppState;

#[cfg(test)]
mod tests {
    use super::changes::pull_request_changes;
    use super::detail::pull_request_detail;
    use super::health::health;
    use super::list::list_pull_requests;
    use super::routes::root_redirect;
    use super::state::{AppState, set_app_state};
    use super::write::{merge_pull_request, submit_comment, update_pull_request_state};
    use crate::gh::CommandClass;
    use crate::gh::GhError;
    use crate::gh::client::{CommandResult, CommandRunner, GhClient, GhCommand};
    use crate::gh::models::PullRequestStatus;
    use crate::gh::models::RepoContext;
    use crate::http::{Request, Response};
    use crate::search::SearchArgs;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Duration;

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock() -> &'static Mutex<()> {
        TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn acquire_test_lock() -> std::sync::MutexGuard<'static, ()> {
        test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[derive(Default)]
    struct MockRunner {
        responses: Mutex<VecDeque<Result<CommandResult, GhError>>>,
        classed_responses: Mutex<HashMap<CommandClass, VecDeque<Result<CommandResult, GhError>>>>,
        seen: Mutex<Vec<GhCommand>>,
    }

    impl MockRunner {
        fn with_responses(responses: Vec<Result<CommandResult, GhError>>) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
                classed_responses: Mutex::new(HashMap::new()),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn with_class_responses(
            responses: Vec<(CommandClass, Result<CommandResult, GhError>)>,
        ) -> Self {
            let mut classed_responses = HashMap::new();
            for (class, response) in responses {
                classed_responses
                    .entry(class)
                    .or_insert_with(VecDeque::new)
                    .push_back(response);
            }

            Self {
                responses: Mutex::new(VecDeque::new()),
                classed_responses: Mutex::new(classed_responses),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn seen_commands(&self) -> Vec<GhCommand> {
            self.seen.lock().expect("seen lock").clone()
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, command: GhCommand) -> Result<CommandResult, GhError> {
            self.seen.lock().expect("seen lock").push(command.clone());

            if let Some(response) = self
                .classed_responses
                .lock()
                .expect("classed responses lock")
                .get_mut(&command.class)
                .and_then(VecDeque::pop_front)
            {
                return response;
            }

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

    fn request(raw: &str) -> Request {
        Request::from_bytes(raw.as_bytes()).expect("request should parse")
    }

    fn request_with_number(raw: &str, number: &str) -> Request {
        request(raw).with_params(
            [("number".to_string(), number.to_string())]
                .into_iter()
                .collect(),
        )
    }

    fn body_text(response: &Response) -> String {
        let bytes = response.to_http_bytes();
        let marker = b"\r\n\r\n";
        let body_index = bytes
            .windows(marker.len())
            .position(|chunk| chunk == marker)
            .map(|idx| idx + marker.len())
            .unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[body_index..]).to_string()
    }

    fn header(response: &Response, name: &str) -> Option<String> {
        let text = String::from_utf8_lossy(&response.to_http_bytes()).to_string();
        for line in text.lines().skip(1) {
            if line.trim().is_empty() {
                break;
            }
            if let Some((header_name, value)) = line.split_once(':')
                && header_name.trim().eq_ignore_ascii_case(name)
            {
                return Some(value.trim().to_string());
            }
        }
        None
    }

    fn state_with_responses(responses: Vec<Result<CommandResult, GhError>>) -> AppState {
        let runner = Arc::new(MockRunner::with_responses(responses));
        state_with_runner(runner)
    }

    fn state_with_class_responses(
        responses: Vec<(CommandClass, Result<CommandResult, GhError>)>,
    ) -> AppState {
        let runner = Arc::new(MockRunner::with_class_responses(responses));
        state_with_runner(runner)
    }

    fn state_with_runner(runner: Arc<MockRunner>) -> AppState {
        AppState {
            gh: GhClient::with_runner(runner, Duration::from_secs(3)),
            startup_repo: Some(RepoContext {
                name_with_owner: "acme/widgets".to_string(),
                url: "https://github.com/acme/widgets".to_string(),
                viewer_permission: "WRITE".to_string(),
                default_branch: "main".to_string(),
            }),
            startup_error: None,
            startup_elapsed: Duration::from_millis(7),
        }
    }

    #[test]
    fn root_redirects_to_pr_list() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            let response =
                root_redirect(request("GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).await;
            assert_eq!(response.status_code(), 303);
            assert_eq!(header(&response, "Location").as_deref(), Some("/prs"));
        });
    }

    #[test]
    fn list_handler_renders_rows() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            let state = state_with_responses(vec![ok(r#"[
                {
                    "repository": {"nameWithOwner": "acme/widgets"},
                    "number":7,
                    "title":"Improve auth",
                    "state":"open",
                    "isDraft":false,
                    "author":{"login":"alice"},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-02T00:00:00Z",
                    "url":"https://example/pr/7",
                    "commentsCount":2
                }
            ]"#)]);
            state
                .gh
                .refresh_search_pull_requests(&SearchArgs::default())
                .await
                .expect("prime search cache");
            set_app_state(state);

            let response =
                list_pull_requests(request("GET /prs HTTP/1.1\r\nHost: localhost\r\n\r\n")).await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("Improve auth"));
            assert!(body.contains("/repos/acme/widgets/prs/7"));
            assert!(body.contains("acme/widgets"));
        });
    }

    #[test]
    fn list_handler_renders_skeleton_without_cache() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            set_app_state(state_with_responses(vec![]));

            let response =
                list_pull_requests(request("GET /prs HTTP/1.1\r\nHost: localhost\r\n\r\n")).await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("table-like-row-loading"));
            assert!(body.contains("global-status-spinner"));
            assert!(body.contains("data-needs-refresh=\"true\""));
        });
    }

    #[test]
    fn list_handler_nocache_fetches_fresh_results() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            set_app_state(state_with_class_responses(vec![
                (CommandClass::RepoList, ok("")),
                (CommandClass::RepoList, ok("[]")),
                (
                    CommandClass::PullRequestSearch,
                    ok(r#"[
                {
                    "repository": {"nameWithOwner": "acme/widgets"},
                    "number":7,
                    "title":"Improve auth",
                    "state":"open",
                    "isDraft":false,
                    "author":{"login":"alice"},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-02T00:00:00Z",
                    "url":"https://example/pr/7",
                    "commentsCount":2
                }
            ]"#),
                ),
            ]));

            let response = list_pull_requests(request(
                "GET /prs?nocache=1 HTTP/1.1\r\nHost: localhost\r\n\r\n",
            ))
            .await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("Improve auth"));
            assert!(body.contains("data-needs-refresh=\"false\""));
        });
    }

    #[test]
    fn list_handler_nocache_reuses_cached_repo_options() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            let runner = Arc::new(MockRunner::with_class_responses(vec![
                (CommandClass::RepoList, ok("")),
                (
                    CommandClass::RepoList,
                    ok(r#"[
                        {
                            "nameWithOwner":"acme/widgets",
                            "isArchived":false
                        }
                    ]"#),
                ),
                (CommandClass::PullRequestSearch, ok("[]")),
            ]));
            let state = state_with_runner(Arc::clone(&runner));
            state
                .gh
                .refresh_accessible_repositories()
                .await
                .expect("prime repo cache");
            set_app_state(state);

            let response = list_pull_requests(request(
                "GET /prs?nocache=1 HTTP/1.1\r\nHost: localhost\r\n\r\n",
            ))
            .await;

            assert_eq!(response.status_code(), 200);
            let seen = runner.seen_commands();
            let repo_list_count = seen
                .iter()
                .filter(|command| command.class == CommandClass::RepoList)
                .count();
            let search_count = seen
                .iter()
                .filter(|command| command.class == CommandClass::PullRequestSearch)
                .count();
            assert_eq!(repo_list_count, 2);
            assert_eq!(search_count, 1);
        });
    }

    #[test]
    fn detail_handler_renders_sections() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            let state = state_with_class_responses(vec![
                (
                    CommandClass::PullRequestDetail,
                    ok(r#"{
                    "number":7,
                    "title":"Improve auth",
                    "body":"Body",
                    "state":"OPEN",
                    "isDraft":false,
                    "author":{"login":"alice"},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-02T00:00:00Z",
                    "url":"https://example/pr/7",
                    "baseRefName":"main",
                    "headRefName":"feature",
                    "mergeStateStatus":"CLEAN",
                    "mergeable":"MERGEABLE",
                    "reviewDecision":"REVIEW_REQUIRED",
                    "reviewRequests":[],
                    "latestReviews":[],
                    "statusCheckRollup":null,
                    "commits":{"totalCount":3},
                    "files":{"totalCount":5}
                }"#),
                ),
                (CommandClass::IssueComments, ok("[]")),
                (CommandClass::PullRequestReviews, ok("[]")),
                (CommandClass::PullRequestReviewComments, ok("[]")),
            ]);
            state
                .gh
                .refresh_pull_request_conversation("acme/widgets", 7)
                .await
                .expect("prime conversation cache");
            set_app_state(state);

            let response = pull_request_detail(
                request_with_number(
                    "GET /repos/acme/widgets/prs/7 HTTP/1.1\r\nHost: localhost\r\n\r\n",
                    "7",
                )
                .with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )
            .await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("PR #7"));
            assert!(body.contains("Post Comment"));
            assert!(body.contains("Approve"));
        });
    }

    #[test]
    fn changes_handler_renders_diff_tab() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            let state = state_with_class_responses(vec![
                (
                    CommandClass::PullRequestDetail,
                    ok(r#"{
                    "number":7,
                    "title":"Improve auth",
                    "body":"Body",
                    "state":"OPEN",
                    "isDraft":false,
                    "author":{"login":"alice"},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-02T00:00:00Z",
                    "url":"https://example/pr/7",
                    "baseRefName":"main",
                    "headRefName":"feature",
                    "mergeStateStatus":"CLEAN",
                    "mergeable":"MERGEABLE",
                    "reviewDecision":"REVIEW_REQUIRED",
                    "reviewRequests":[],
                    "latestReviews":[],
                    "statusCheckRollup":null,
                    "commits":{"totalCount":3},
                    "files":{"totalCount":5}
                }"#),
                ),
                (CommandClass::IssueComments, ok("[]")),
                (CommandClass::PullRequestReviews, ok("[]")),
                (CommandClass::PullRequestReviewComments, ok("[]")),
                (
                    CommandClass::PullRequestFiles,
                    ok(r#"[
                    {
                        "filename":"src/main.rs",
                        "status":"modified",
                        "additions":5,
                        "deletions":1,
                        "changes":6,
                        "patch":"@@ -1 +1 @@\n-a\n+b",
                        "blob_url":"https://example/blob"
                    }
                ]"#),
                ),
            ]);
            state
                .gh
                .refresh_pull_request_conversation("acme/widgets", 7)
                .await
                .expect("prime conversation cache");
            state
                .gh
                .refresh_pull_request_files("acme/widgets", 7)
                .await
                .expect("prime files cache");
            set_app_state(state);

            let response = pull_request_changes(
                request_with_number(
                    "GET /repos/acme/widgets/prs/7/changes HTTP/1.1\r\nHost: localhost\r\n\r\n",
                    "7",
                )
                .with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )
            .await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("Changes"));
            assert!(body.contains("src/main.rs"));
        });
    }

    #[test]
    fn changes_handler_renders_partial_cache_when_files_are_missing() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            let state = state_with_class_responses(vec![
                (
                    CommandClass::PullRequestDetail,
                    ok(r#"{
                    "number":7,
                    "title":"Improve auth",
                    "body":"Body",
                    "state":"OPEN",
                    "isDraft":false,
                    "author":{"login":"alice"},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-02T00:00:00Z",
                    "url":"https://example/pr/7",
                    "baseRefName":"main",
                    "headRefName":"feature",
                    "mergeStateStatus":"CLEAN",
                    "mergeable":"MERGEABLE",
                    "reviewDecision":"REVIEW_REQUIRED",
                    "reviewRequests":[],
                    "latestReviews":[],
                    "statusCheckRollup":null,
                    "commits":{"totalCount":3},
                    "files":{"totalCount":5}
                }"#),
                ),
                (CommandClass::IssueComments, ok("[]")),
                (CommandClass::PullRequestReviews, ok("[]")),
                (CommandClass::PullRequestReviewComments, ok("[]")),
            ]);
            state
                .gh
                .refresh_pull_request_conversation("acme/widgets", 7)
                .await
                .expect("prime conversation cache");
            set_app_state(state);

            let response = pull_request_changes(
                request_with_number(
                    "GET /repos/acme/widgets/prs/7/changes HTTP/1.1\r\nHost: localhost\r\n\r\n",
                    "7",
                )
                .with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )
            .await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("PR #7 · Improve auth"));
            assert!(body.contains("data-needs-refresh=\"true\""));
            assert!(body.contains("loading-box"));
            assert!(body.contains("global-status-spinner"));
        });
    }

    #[test]
    fn comment_post_redirects_back_to_pr() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            set_app_state(state_with_responses(vec![ok("")]));
            let raw = "POST /repos/acme/widgets/prs/7/comment?org=acme HTTP/1.1\r\nHost: localhost\r\nContent-Length: 15\r\n\r\nbody=hello+team";
            let response = submit_comment(
                request(raw).with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )
            .await;

            assert_eq!(response.status_code(), 303);
            let location = header(&response, "Location").unwrap_or_default();
            assert_eq!(location, "/repos/acme/widgets/prs/7?org=acme");
        });
    }

    #[test]
    fn comment_post_renders_error_page_when_startup_failed() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            set_app_state(AppState {
                gh: GhClient::with_runner(Arc::new(MockRunner::default()), Duration::from_secs(3)),
                startup_repo: None,
                startup_error: Some(GhError::NotAuthenticated),
                startup_elapsed: Duration::from_millis(15),
            });

            let response = submit_comment(
                request("POST /prs/7/comment HTTP/1.1\r\nHost: localhost\r\nContent-Length: 8\r\n\r\nbody=hey")
                    .with_params(
                        [("number".to_string(), "7".to_string())]
                            .into_iter()
                            .collect(),
                    ),
            )
            .await;

            assert_eq!(response.status_code(), 503);
            let body = body_text(&response);
            assert!(body.contains("GitHub CLI Not Authenticated"));
            assert!(body.contains("Run `gh auth login` and retry."));
        });
    }

    #[test]
    fn merge_post_redirects_back_to_pr() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            set_app_state(state_with_responses(vec![ok("")]));

            let response = merge_pull_request(
                request_with_number(
                    "POST /repos/acme/widgets/prs/7/merge HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 30\r\n\r\nmethod=squash&delete_branch=on",
                    "7",
                )
                .with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )
            .await;

            assert_eq!(response.status_code(), 303);
            let location = header(&response, "Location").expect("location");
            assert_eq!(location, "/repos/acme/widgets/prs/7");
        });
    }

    #[test]
    fn state_post_redirects_back_to_pr() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            set_app_state(state_with_responses(vec![ok("")]));

            let response = update_pull_request_state(
                request_with_number(
                    "POST /repos/acme/widgets/prs/7/state HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 11\r\n\r\nstate=close",
                    "7",
                )
                .with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )
            .await;

            assert_eq!(response.status_code(), 303);
            let location = header(&response, "Location").expect("location");
            assert_eq!(location, "/repos/acme/widgets/prs/7");
        });
    }

    #[test]
    fn health_reports_degraded_when_startup_failed() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            set_app_state(AppState {
                gh: GhClient::with_runner(Arc::new(MockRunner::default()), Duration::from_secs(3)),
                startup_repo: None,
                startup_error: Some(GhError::NotAuthenticated),
                startup_elapsed: Duration::from_millis(15),
            });

            let response = health(request("GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")).await;

            assert_eq!(response.status_code(), 503);
            let body = body_text(&response);
            assert!(body.contains("\"status\":\"degraded\""));
        });
    }

    #[test]
    fn list_links_preserve_query_context() {
        let _guard = acquire_test_lock();
        smol::block_on(async {
            let state = state_with_responses(vec![ok(r#"[
                {
                    "repository": {"nameWithOwner": "acme/widgets"},
                    "number":7,
                    "title":"Improve auth",
                    "state":"open",
                    "isDraft":false,
                    "author":{"login":"alice"},
                    "createdAt":"2026-01-01T00:00:00Z",
                    "updatedAt":"2026-01-02T00:00:00Z",
                    "url":"https://example/pr/7",
                    "commentsCount":2
                }
            ]"#)]);
            let query = SearchArgs {
                org: Some("acme".to_string()),
                status: PullRequestStatus::Open,
                ..SearchArgs::default()
            };
            state
                .gh
                .refresh_search_pull_requests(&query)
                .await
                .expect("prime filtered search cache");
            set_app_state(state);

            let response = list_pull_requests(request(
                "GET /prs?org=acme&status=open HTTP/1.1\r\nHost: localhost\r\n\r\n",
            ))
            .await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("/repos/acme/widgets/prs/7?org=acme&amp;status=open"));
        });
    }
}

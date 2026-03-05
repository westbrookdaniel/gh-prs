pub mod changes;
pub mod context;
pub mod detail;
pub mod flash;
pub mod format;
pub mod forms;
pub mod health;
pub mod list;
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
    use super::write::submit_comment;
    use crate::gh::GhError;
    use crate::gh::client::{CommandResult, CommandRunner, GhClient};
    use crate::gh::models::RepoContext;
    use crate::http::{Request, Response};
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
    }

    impl MockRunner {
        fn with_responses(responses: Vec<Result<CommandResult, GhError>>) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
            }
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, _command: crate::gh::client::GhCommand) -> Result<CommandResult, GhError> {
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
            if let Some((header_name, value)) = line.split_once(':') {
                if header_name.trim().eq_ignore_ascii_case(name) {
                    return Some(value.trim().to_string());
                }
            }
        }
        None
    }

    fn state_with_responses(responses: Vec<Result<CommandResult, GhError>>) -> AppState {
        let runner = Arc::new(MockRunner::with_responses(responses));
        AppState {
            gh: GhClient::with_runner(runner, Duration::from_secs(3)),
            startup_repo: Some(RepoContext {
                name_with_owner: "acme/widgets".to_string(),
                url: "https://github.com/acme/widgets".to_string(),
                viewer_permission: "WRITE".to_string(),
                default_branch: "main".to_string(),
            }),
            diagnostics: None,
            startup_error: None,
            startup_elapsed: Duration::from_millis(7),
        }
    }

    #[test]
    fn root_redirects_to_pr_list() {
        smol::block_on(async {
            let _guard = test_lock().lock().expect("test lock");
            let response =
                root_redirect(request("GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")).await;
            assert_eq!(response.status_code(), 303);
            assert_eq!(header(&response, "Location").as_deref(), Some("/prs"));
        });
    }

    #[test]
    fn list_handler_renders_rows() {
        smol::block_on(async {
            let _guard = test_lock().lock().expect("test lock");
            set_app_state(state_with_responses(vec![ok("[]"), ok(r#"[
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
            ]"#)]));

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
    fn detail_handler_renders_sections() {
        smol::block_on(async {
            let _guard = test_lock().lock().expect("test lock");
            set_app_state(state_with_responses(vec![
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
                ok("[]"),
                ok("[]"),
                ok("[]"),
            ]));

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
            assert!(body.contains("Submit Comment"));
            assert!(body.contains("Submit Review"));
        });
    }

    #[test]
    fn changes_handler_renders_diff_tab() {
        smol::block_on(async {
            let _guard = test_lock().lock().expect("test lock");
            set_app_state(state_with_responses(vec![
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
                ok("[]"),
                ok("[]"),
                ok("[]"),
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
            ]));

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
    fn comment_post_redirects_with_success_flash() {
        smol::block_on(async {
            let _guard = test_lock().lock().expect("test lock");
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
            assert!(location.starts_with("/repos/acme/widgets/prs/7?org=acme"));
            assert!(location.contains("flash=success"));
        });
    }

    #[test]
    fn health_reports_degraded_when_startup_failed() {
        smol::block_on(async {
            let _guard = test_lock().lock().expect("test lock");
            set_app_state(AppState {
                gh: GhClient::with_runner(Arc::new(MockRunner::default()), Duration::from_secs(3)),
                startup_repo: None,
                diagnostics: None,
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
        smol::block_on(async {
            let _guard = test_lock().lock().expect("test lock");
            set_app_state(state_with_responses(vec![ok("[]"), ok(r#"[
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
            ]"#)]));

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

use askama::Template;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

use crate::gh::client::{GhClient, ReviewEvent};
use crate::gh::models::{PreflightDiagnostics, RepoContext};
use crate::gh::{GhError, GhResult};
use crate::http::{App, Request, Response};
use crate::views::{
    ErrorTemplate, FlashMessageView, PrDetailTemplate, PrListTemplate, parse_search_query,
};

const FLASH_MAX_LEN: usize = 220;
const HEALTH_OK_STATUS: &str = "ok";
const HEALTH_DEGRADED_STATUS: &str = "degraded";

#[derive(Clone)]
pub struct AppState {
    pub gh: GhClient,
    pub startup_repo: Option<RepoContext>,
    pub diagnostics: Option<PreflightDiagnostics>,
    pub startup_error: Option<GhError>,
    pub startup_elapsed: Duration,
}

impl AppState {
    pub fn startup_ready(&self) -> GhResult<()> {
        if let Some(error) = &self.startup_error {
            return Err(error.clone());
        }

        Ok(())
    }
}

pub fn register(app: App, state: Arc<AppState>) -> App {
    let root_state = Arc::clone(&state);
    let health_state = Arc::clone(&state);
    let list_state = Arc::clone(&state);
    let detail_repo_state = Arc::clone(&state);
    let detail_simple_state = Arc::clone(&state);
    let comment_repo_state = Arc::clone(&state);
    let comment_simple_state = Arc::clone(&state);
    let review_repo_state = Arc::clone(&state);
    let review_simple_state = Arc::clone(&state);

    app.get("/", move |request| {
        let state = Arc::clone(&root_state);
        async move { root_redirect(request, state).await }
    })
    .get("/health", move |request| {
        let state = Arc::clone(&health_state);
        async move { health(request, state).await }
    })
    .get("/prs", move |request| {
        let state = Arc::clone(&list_state);
        async move { list_pull_requests(request, state).await }
    })
    .get("/repos/:owner/:repo/prs/:number", move |request| {
        let state = Arc::clone(&detail_repo_state);
        async move { pull_request_detail(request, state).await }
    })
    .get("/prs/:number", move |request| {
        let state = Arc::clone(&detail_simple_state);
        async move { pull_request_detail(request, state).await }
    })
    .post("/repos/:owner/:repo/prs/:number/comment", move |request| {
        let state = Arc::clone(&comment_repo_state);
        async move { submit_comment(request, state).await }
    })
    .post("/prs/:number/comment", move |request| {
        let state = Arc::clone(&comment_simple_state);
        async move { submit_comment(request, state).await }
    })
    .post("/repos/:owner/:repo/prs/:number/review", move |request| {
        let state = Arc::clone(&review_repo_state);
        async move { submit_review(request, state).await }
    })
    .post("/prs/:number/review", move |request| {
        let state = Arc::clone(&review_simple_state);
        async move { submit_review(request, state).await }
    })
}

pub async fn root_redirect(_request: Request, _state: Arc<AppState>) -> Response {
    Response::new(303, "See Other")
        .header("Location", "/prs")
        .text_body("See Other")
}

pub async fn health(_request: Request, state: Arc<AppState>) -> Response {
    #[derive(Serialize)]
    struct HealthResponse<'a> {
        status: &'a str,
        repo: Option<&'a str>,
        startup_ms: u128,
        message: Option<String>,
    }

    let repo_name = state
        .startup_repo
        .as_ref()
        .map(|repo| repo.name_with_owner.as_str());

    match &state.startup_error {
        Some(error) => {
            let body = serde_json::to_vec(&HealthResponse {
                status: HEALTH_DEGRADED_STATUS,
                repo: repo_name,
                startup_ms: state.startup_elapsed.as_millis(),
                message: Some(error.message()),
            })
            .unwrap_or_else(|_| {
                b"{\"status\":\"degraded\",\"message\":\"health serialization failed\"}".to_vec()
            });

            Response::new(503, "Service Unavailable")
                .header("Content-Type", "application/json")
                .body(body)
        }
        None => {
            let body = serde_json::to_vec(&HealthResponse {
                status: HEALTH_OK_STATUS,
                repo: repo_name,
                startup_ms: state.startup_elapsed.as_millis(),
                message: None,
            })
            .unwrap_or_else(|_| b"{\"status\":\"ok\"}".to_vec());

            Response::new(200, "OK")
                .header("Content-Type", "application/json")
                .body(body)
        }
    }
}

pub async fn list_pull_requests(request: Request, state: Arc<AppState>) -> Response {
    let flash = flash_from_query(&request);

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let query = parse_search_query(&request);

    match state.gh.search_pull_requests(&query).await {
        Ok(items) => {
            let template = PrListTemplate::from_search_results(
                state.startup_repo.as_ref(),
                state.diagnostics.as_ref(),
                &query,
                items,
                flash,
            );
            render_template(200, "OK", &template)
        }
        Err(err) => return render_gh_error(err),
    }
}

pub async fn pull_request_detail(request: Request, state: Arc<AppState>) -> Response {
    let flash = flash_from_query(&request);

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let repo_name = match repo_from_request(&request, state.startup_repo.as_ref()) {
        Ok(repo) => repo,
        Err(err) => return render_gh_error(err),
    };

    let repo_context = RepoContext {
        name_with_owner: repo_name.clone(),
        url: format!("https://github.com/{repo_name}"),
        viewer_permission: "UNKNOWN".to_string(),
        default_branch: "main".to_string(),
    };

    let number = match parse_pr_number(&request) {
        Ok(number) => number,
        Err(err) => return render_gh_error(err),
    };

    match state.gh.pull_request_conversation(&repo_name, number).await {
        Ok(conversation) => {
            let template = PrDetailTemplate::from_conversation(&repo_context, conversation, flash);
            render_template(200, "OK", &template)
        }
        Err(err) => render_gh_error(err),
    }
}

pub async fn submit_comment(request: Request, state: Arc<AppState>) -> Response {
    if let Err(err) = state.startup_ready() {
        return redirect_with_flash("/prs", FlashMessageView::error(err.message()));
    }

    let repo_name = match repo_from_request(&request, state.startup_repo.as_ref()) {
        Ok(repo) => repo,
        Err(err) => return redirect_with_flash("/prs", FlashMessageView::error(err.message())),
    };

    let number = match parse_pr_number(&request) {
        Ok(number) => number,
        Err(err) => return redirect_with_flash("/prs", FlashMessageView::error(err.message())),
    };

    let form = match parse_form::<CommentForm>(&request) {
        Ok(form) => form,
        Err(err) => {
            return redirect_to_pr(
                &repo_name,
                number,
                FlashMessageView::error(format!("Unable to parse comment form: {err}")),
            );
        }
    };

    let result = state
        .gh
        .submit_comment(&repo_name, number, &form.body)
        .await;

    match result {
        Ok(()) => redirect_to_pr(
            &repo_name,
            number,
            FlashMessageView::success("Comment posted."),
        ),
        Err(err) => redirect_to_pr(&repo_name, number, FlashMessageView::error(err.message())),
    }
}

pub async fn submit_review(request: Request, state: Arc<AppState>) -> Response {
    if let Err(err) = state.startup_ready() {
        return redirect_with_flash("/prs", FlashMessageView::error(err.message()));
    }

    let repo_name = match repo_from_request(&request, state.startup_repo.as_ref()) {
        Ok(repo) => repo,
        Err(err) => return redirect_with_flash("/prs", FlashMessageView::error(err.message())),
    };

    let number = match parse_pr_number(&request) {
        Ok(number) => number,
        Err(err) => return redirect_with_flash("/prs", FlashMessageView::error(err.message())),
    };

    let form = match parse_form::<ReviewForm>(&request) {
        Ok(form) => form,
        Err(err) => {
            return redirect_to_pr(
                &repo_name,
                number,
                FlashMessageView::error(format!("Unable to parse review form: {err}")),
            );
        }
    };

    let event = match ReviewEvent::parse(&form.event) {
        Ok(event) => event,
        Err(err) => {
            return redirect_to_pr(&repo_name, number, FlashMessageView::error(err.message()));
        }
    };

    let result = state
        .gh
        .submit_review(&repo_name, number, event, &form.body)
        .await;

    match result {
        Ok(()) => {
            let event_text = match event {
                ReviewEvent::Approve => "Review submitted: approved.",
                ReviewEvent::Comment => "Review submitted: comment.",
                ReviewEvent::RequestChanges => "Review submitted: changes requested.",
            };
            redirect_to_pr(&repo_name, number, FlashMessageView::success(event_text))
        }
        Err(err) => redirect_to_pr(&repo_name, number, FlashMessageView::error(err.message())),
    }
}

fn repo_from_request(request: &Request, fallback_repo: Option<&RepoContext>) -> GhResult<String> {
    if let Some(repo) = request.query_param("repo") {
        return validate_repo_identifier(repo);
    }

    let owner = request.param("owner");
    let name = request.param("repo");

    if let (Some(owner), Some(name)) = (owner, name) {
        return validate_repo_identifier(&format!("{owner}/{name}"));
    }

    if let Some(repo) = fallback_repo {
        return validate_repo_identifier(&repo.name_with_owner);
    }

    Err(GhError::InvalidInput {
        field: "repo".to_string(),
        details: "missing repo context; provide ?repo=OWNER/REPO".to_string(),
    })
}

fn parse_pr_number(request: &Request) -> GhResult<u64> {
    let raw = request
        .param("number")
        .ok_or_else(|| GhError::InvalidInput {
            field: "number".to_string(),
            details: "missing route parameter".to_string(),
        })?;

    let number = raw.parse::<u64>().map_err(|_| GhError::InvalidInput {
        field: "number".to_string(),
        details: "must be a positive integer".to_string(),
    })?;

    if number == 0 {
        return Err(GhError::InvalidInput {
            field: "number".to_string(),
            details: "must be greater than zero".to_string(),
        });
    }

    Ok(number)
}

fn flash_from_query(request: &Request) -> Option<FlashMessageView> {
    let kind = request.query_param("flash")?;
    let message = request.query_param("message")?.trim();
    if message.is_empty() {
        return None;
    }

    let message = message.chars().take(FLASH_MAX_LEN).collect::<String>();

    match kind {
        "success" => Some(FlashMessageView::success(message)),
        "error" => Some(FlashMessageView::error(message)),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct CommentForm {
    body: String,
}

#[derive(Debug, Deserialize)]
struct ReviewForm {
    event: String,
    body: String,
}

fn parse_form<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, String> {
    serde_urlencoded::from_bytes::<T>(&request.body).map_err(|err| err.to_string())
}

fn redirect_to_pr(repo: &str, number: u64, flash: FlashMessageView) -> Response {
    if let Some((owner, name)) = repo.split_once('/') {
        return redirect_with_flash(&format!("/repos/{owner}/{name}/prs/{number}"), flash);
    }
    redirect_with_flash(&format!("/prs/{number}"), flash)
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

fn redirect_with_flash(location: &str, flash: FlashMessageView) -> Response {
    let query = serde_urlencoded::to_string([
        ("flash", flash.kind.as_str()),
        ("message", flash.message.as_str()),
    ])
    .unwrap_or_default();

    let destination = if query.is_empty() {
        location.to_string()
    } else {
        format!("{location}?{query}")
    };

    Response::new(303, "See Other")
        .header("Location", destination)
        .text_body("See Other")
}

fn render_gh_error(error: GhError) -> Response {
    let status = error.status_code();
    let reason = reason_phrase(status);
    let template = ErrorTemplate::new(
        status,
        error.title(),
        error.message(),
        error.remediation(),
        error.details(),
    );
    render_template(status, reason, &template)
}

fn render_template(status: u16, reason: &'static str, template: &impl Template) -> Response {
    match template.render() {
        Ok(html) => Response::new(status, reason)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(html),
        Err(err) => {
            Response::internal_server_error().text_body(format!("failed to render template: {err}"))
        }
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppState, health, list_pull_requests, parse_pr_number, pull_request_detail, root_redirect,
        submit_comment, submit_review,
    };
    use crate::gh::client::{CommandResult, CommandRunner, GhClient};
    use crate::gh::models::RepoContext;
    use crate::gh::{CommandClass, GhError};
    use crate::http::{Request, Response};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

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

    fn state_with_responses(responses: Vec<Result<CommandResult, GhError>>) -> Arc<AppState> {
        let runner = Arc::new(MockRunner::with_responses(responses));
        Arc::new(AppState {
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
        })
    }

    #[test]
    fn root_redirects_to_pr_list() {
        smol::block_on(async {
            let state = state_with_responses(vec![]);
            let response =
                root_redirect(request("GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"), state).await;
            assert_eq!(response.status_code(), 303);
            assert_eq!(header(&response, "Location").as_deref(), Some("/prs"));
        });
    }

    #[test]
    fn list_handler_renders_rows() {
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

            let response = list_pull_requests(
                request("GET /prs HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("Improve auth"));
            assert!(body.contains("/repos/acme/widgets/prs/7"));
            assert!(body.contains("acme/widgets"));
        });
    }

    #[test]
    fn list_handler_applies_org_and_repo_filters() {
        smol::block_on(async {
            let state = state_with_responses(vec![ok("[]")]);
            let response = list_pull_requests(
                request(
                    "GET /prs?org=westbrookdaniel&repo=blogs HTTP/1.1\r\nHost: localhost\r\n\r\n",
                ),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 200);
        });
    }

    #[test]
    fn list_handler_shows_limit_warning_when_result_count_hits_cap() {
        smol::block_on(async {
            let mut rows = String::new();
            rows.push('[');
            for idx in 1..=100 {
                if idx > 1 {
                    rows.push(',');
                }
                rows.push_str(&format!(
                    "{{\"repository\":{{\"nameWithOwner\":\"acme/widgets\"}},\"number\":{},\"title\":\"PR {}\",\"state\":\"open\",\"isDraft\":false,\"author\":{{\"login\":\"alice\"}},\"createdAt\":\"2026-01-01T00:00:00Z\",\"updatedAt\":\"2026-01-02T00:00:00Z\",\"url\":\"https://example/pr/{}\",\"commentsCount\":0}}",
                    idx, idx, idx
                ));
            }
            rows.push(']');

            let state = state_with_responses(vec![ok(&rows)]);
            let response = list_pull_requests(
                request("GET /prs HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 200);
            assert!(body_text(&response).contains("Some results may be hidden."));
        });
    }

    #[test]
    fn detail_route_uses_repo_query_fallback_when_params_absent() {
        smol::block_on(async {
            let state = state_with_responses(vec![
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
                    "reviewDecision":null,
                    "reviewRequests":[],
                    "latestReviews":[],
                    "statusCheckRollup":null,
                    "commits":{"totalCount":3},
                    "files":{"totalCount":5}
                }"#),
                ok("[]"),
                ok("[]"),
                ok("[]"),
            ]);

            let response = pull_request_detail(
                request_with_number(
                    "GET /prs/7?repo=acme/widgets HTTP/1.1\r\nHost: localhost\r\n\r\n",
                    "7",
                ),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 200);
            let body = body_text(&response);
            assert!(body.contains("repository: <a href=\"https://github.com/acme/widgets\">"));
        });
    }

    #[test]
    fn detail_handler_renders_sections() {
        smol::block_on(async {
            let state = state_with_responses(vec![
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
            ]);

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
                state,
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
    fn detail_handler_rejects_invalid_pr_number() {
        smol::block_on(async {
            let state = state_with_responses(vec![]);
            let response = pull_request_detail(
                request_with_number(
                    "GET /prs/not-a-number HTTP/1.1\r\nHost: localhost\r\n\r\n",
                    "not-a-number",
                ),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 400);
            assert!(body_text(&response).contains("Invalid `number`"));
        });
    }

    #[test]
    fn detail_handler_renders_not_found_error() {
        smol::block_on(async {
            let state = state_with_responses(vec![Err(GhError::PullRequestNotFound { number: 7 })]);

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
                state,
            )
            .await;

            assert_eq!(response.status_code(), 404);
            assert!(body_text(&response).contains("Pull Request Not Found"));
        });
    }

    #[test]
    fn list_handler_renders_error_page_when_command_fails() {
        smol::block_on(async {
            let state = state_with_responses(vec![Err(GhError::CommandFailed {
                class: CommandClass::PullRequestSearch,
                code: Some(1),
                stderr: "HTTP 502 from api".to_string(),
            })]);

            let response = list_pull_requests(
                request("GET /prs HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 502);
            assert!(body_text(&response).contains("GitHub CLI Command Failed"));
        });
    }

    #[test]
    fn comment_post_redirects_with_success_flash() {
        smol::block_on(async {
            let state = state_with_responses(vec![ok("")]);
            let raw = "POST /repos/acme/widgets/prs/7/comment HTTP/1.1\r\nHost: localhost\r\nContent-Length: 15\r\n\r\nbody=hello+team";
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
                state,
            )
            .await;

            assert_eq!(response.status_code(), 303);
            let location = header(&response, "Location").unwrap_or_default();
            assert!(location.starts_with("/repos/acme/widgets/prs/7?"));
            assert!(location.contains("flash=success"));
        });
    }

    #[test]
    fn review_post_redirects_with_failure_flash() {
        smol::block_on(async {
            let state = state_with_responses(vec![Err(GhError::CommandTimeout {
                class: CommandClass::SubmitReview,
                timeout: Duration::from_secs(2),
            })]);
            let body = "event=approve&body=looks+good";
            let raw = format!(
                "POST /repos/acme/widgets/prs/7/review HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let response = submit_review(
                request(&raw).with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 303);
            let location = header(&response, "Location").unwrap_or_default();
            assert!(location.contains("flash=error"));
        });
    }

    #[test]
    fn review_post_rejects_invalid_event() {
        smol::block_on(async {
            let state = state_with_responses(vec![]);
            let body = "event=invalid&body=needs+work";
            let raw = format!(
                "POST /repos/acme/widgets/prs/7/review HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );

            let response = submit_review(
                request(&raw).with_params(
                    [
                        ("owner".to_string(), "acme".to_string()),
                        ("repo".to_string(), "widgets".to_string()),
                        ("number".to_string(), "7".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
                state,
            )
            .await;
            assert_eq!(response.status_code(), 303);
            let location = header(&response, "Location").unwrap_or_default();
            assert!(location.contains("flash=error"));
            assert!(location.contains("Invalid+%60event%60"));
        });
    }

    #[test]
    fn comment_post_rejects_empty_body() {
        smol::block_on(async {
            let state = state_with_responses(vec![]);
            let raw = "POST /repos/acme/widgets/prs/7/comment HTTP/1.1\r\nHost: localhost\r\nContent-Length: 7\r\n\r\nbody=++";
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
                state,
            )
            .await;
            assert_eq!(response.status_code(), 303);
            let location = header(&response, "Location").unwrap_or_default();
            assert!(location.contains("flash=error"));
            assert!(location.contains("Invalid+%60body%60"));
        });
    }

    #[test]
    fn parse_pr_number_requires_positive_integer() {
        let zero = request("GET /prs/0 HTTP/1.1\r\nHost: localhost\r\n\r\n").with_params(
            [("number".to_string(), "0".to_string())]
                .into_iter()
                .collect(),
        );
        assert!(parse_pr_number(&zero).is_err());
    }

    #[test]
    fn health_reports_degraded_when_startup_failed() {
        smol::block_on(async {
            let state = Arc::new(AppState {
                gh: GhClient::with_runner(Arc::new(MockRunner::default()), Duration::from_secs(3)),
                startup_repo: None,
                diagnostics: None,
                startup_error: Some(GhError::NotAuthenticated),
                startup_elapsed: Duration::from_millis(15),
            });

            let response = health(
                request("GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n"),
                state,
            )
            .await;

            assert_eq!(response.status_code(), 503);
            let body = body_text(&response);
            assert!(body.contains("\"status\":\"degraded\""));
        });
    }
}

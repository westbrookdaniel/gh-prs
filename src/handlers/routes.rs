use crate::handlers::changes::pull_request_changes;
use crate::handlers::detail::pull_request_detail;
use crate::handlers::health::health;
use crate::handlers::list::list_pull_requests;
use crate::handlers::state::SharedState;
use crate::handlers::write::{submit_comment, submit_review};
use crate::http::{App, Request, Response};
use std::sync::Arc;

pub fn register(app: App, state: SharedState) -> App {
    let root_state = Arc::clone(&state);
    let health_state = Arc::clone(&state);
    let list_state = Arc::clone(&state);
    let detail_repo_state = Arc::clone(&state);
    let detail_simple_state = Arc::clone(&state);
    let changes_repo_state = Arc::clone(&state);
    let changes_simple_state = Arc::clone(&state);
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
    .get("/repos/:owner/:repo/prs/:number/changes", move |request| {
        let state = Arc::clone(&changes_repo_state);
        async move { pull_request_changes(request, state).await }
    })
    .get("/prs/:number", move |request| {
        let state = Arc::clone(&detail_simple_state);
        async move { pull_request_detail(request, state).await }
    })
    .get("/prs/:number/changes", move |request| {
        let state = Arc::clone(&changes_simple_state);
        async move { pull_request_changes(request, state).await }
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

pub async fn root_redirect(_request: Request, _state: SharedState) -> Response {
    Response::new(303, "See Other")
        .header("Location", "/prs")
        .text_body("See Other")
}

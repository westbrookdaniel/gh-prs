use crate::handlers::changes::pull_request_changes;
use crate::handlers::detail::pull_request_detail;
use crate::handlers::health::health;
use crate::handlers::list::list_pull_requests;
use crate::handlers::not_found::not_found;
use crate::handlers::write::{submit_comment, submit_review};
use crate::http::{App, Request, Response};

pub fn register(app: App) -> App {
    app.get("/", root_redirect)
        .get("/health", health)
        .get("/prs", list_pull_requests)
        .get("/repos/:owner/:repo/prs/:number", pull_request_detail)
        .get(
            "/repos/:owner/:repo/prs/:number/changes",
            pull_request_changes,
        )
        .get("/prs/:number", pull_request_detail)
        .get("/prs/:number/changes", pull_request_changes)
        .post("/repos/:owner/:repo/prs/:number/comment", submit_comment)
        .post("/prs/:number/comment", submit_comment)
        .post("/repos/:owner/:repo/prs/:number/review", submit_review)
        .post("/prs/:number/review", submit_review)
        .any("/*path", not_found)
}

pub async fn root_redirect(_request: Request) -> Response {
    Response::new(303, "See Other")
        .header("Location", "/prs")
        .text_body("See Other")
}

use crate::handlers::changes::pull_request_changes;
use crate::handlers::detail::pull_request_detail;
use crate::handlers::health::health;
use crate::handlers::list::list_pull_requests;
use crate::handlers::not_found::not_found;
use crate::handlers::stream::{stream_pr_changes, stream_pr_detail, stream_pr_list};
use crate::handlers::write::{
    merge_pull_request, submit_comment, submit_review, update_pull_request_state, update_reviewers,
};
use crate::http::{App, Request, Response};

pub fn register(app: App) -> App {
    app.get("/", root_redirect)
        .get("/health", health)
        .get("/prs", list_pull_requests)
        .get("/streams/prs", stream_pr_list)
        .get("/repos/:owner/:repo/prs/:number", pull_request_detail)
        .get("/streams/repos/:owner/:repo/prs/:number", stream_pr_detail)
        .get(
            "/repos/:owner/:repo/prs/:number/changes",
            pull_request_changes,
        )
        .get(
            "/streams/repos/:owner/:repo/prs/:number/changes",
            stream_pr_changes,
        )
        .get("/prs/:number", pull_request_detail)
        .get("/streams/prs/:number", stream_pr_detail)
        .get("/prs/:number/changes", pull_request_changes)
        .get("/streams/prs/:number/changes", stream_pr_changes)
        .post("/repos/:owner/:repo/prs/:number/comment", submit_comment)
        .post("/prs/:number/comment", submit_comment)
        .post("/repos/:owner/:repo/prs/:number/review", submit_review)
        .post("/prs/:number/review", submit_review)
        .post(
            "/repos/:owner/:repo/prs/:number/reviewers",
            update_reviewers,
        )
        .post("/prs/:number/reviewers", update_reviewers)
        .post("/repos/:owner/:repo/prs/:number/merge", merge_pull_request)
        .post("/prs/:number/merge", merge_pull_request)
        .post(
            "/repos/:owner/:repo/prs/:number/state",
            update_pull_request_state,
        )
        .post("/prs/:number/state", update_pull_request_state)
        .any("/*path", not_found)
}

pub async fn root_redirect(_request: Request) -> Response {
    Response::new(303, "See Other")
        .header("Location", "/prs")
        .text_body("See Other")
}

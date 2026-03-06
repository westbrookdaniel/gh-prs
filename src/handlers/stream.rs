use crate::gh::GhError;
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::search::SearchArgs;
use crate::views::{
    PrChangesContentTemplate, PrDetailContentTemplate, PrListResultsTemplate, changes_page_model,
    detail_page_model, list_page_model,
};
use askama::Template;
use datastar::consts::ElementPatchMode;
use datastar::patch_elements::PatchElements;

const STREAM_STATUS_SELECTOR: &str = "#stream-status";
const LIST_RESULTS_SELECTOR: &str = "#pr-list-results";
const DETAIL_CONTENT_SELECTOR: &str = "#pr-detail-content";
const CHANGES_CONTENT_SELECTOR: &str = "#pr-changes-content";

pub async fn stream_pr_list(request: Request) -> Response {
    let state = app_state_snapshot();
    if let Err(error) = state.startup_ready() {
        return stream_error_response(error, Some(LIST_RESULTS_SELECTOR));
    }

    let query = SearchArgs::from_request(&request);
    let maybe_cached = match state.gh.cached_search_pull_requests(&query).await {
        Ok(value) => value,
        Err(error) => return stream_error_response(error, Some(LIST_RESULTS_SELECTOR)),
    };

    let has_cached = maybe_cached.is_some();
    let should_refresh = maybe_cached.as_ref().map_or(true, |cached| cached.is_stale);
    if !should_refresh {
        return sse_noop();
    }

    let items = match state.gh.refresh_search_pull_requests(&query).await {
        Ok(items) => items,
        Err(error) => {
            return stream_error_response(
                error,
                if has_cached {
                    None
                } else {
                    Some(LIST_RESULTS_SELECTOR)
                },
            );
        }
    };

    let model = list_page_model(
        state.startup_repo.as_ref(),
        state.diagnostics.as_ref(),
        &query,
        Vec::new(),
        items,
        false,
        None,
        &request,
    );

    let html = match (PrListResultsTemplate {
        model: model.clone(),
    })
    .render()
    {
        Ok(html) => html,
        Err(error) => return stream_render_error(error.to_string(), Some(LIST_RESULTS_SELECTOR)),
    };

    sse_patch(LIST_RESULTS_SELECTOR, &html)
}

pub async fn stream_pr_detail(request: Request) -> Response {
    let state = app_state_snapshot();
    if let Err(error) = state.startup_ready() {
        return stream_error_response(error, Some(DETAIL_CONTENT_SELECTOR));
    }

    let repo_name = match repo_from_request(&request, state.startup_repo.as_ref()) {
        Ok(repo) => repo,
        Err(error) => return stream_error_response(error, Some(DETAIL_CONTENT_SELECTOR)),
    };
    let number = match parse_pr_number(&request) {
        Ok(number) => number,
        Err(error) => return stream_error_response(error, Some(DETAIL_CONTENT_SELECTOR)),
    };

    let maybe_cached = match state
        .gh
        .cached_pull_request_conversation(&repo_name, number)
        .await
    {
        Ok(value) => value,
        Err(error) => return stream_error_response(error, Some(DETAIL_CONTENT_SELECTOR)),
    };

    let has_cached = maybe_cached.is_some();
    let should_refresh = maybe_cached.as_ref().map_or(true, |cached| cached.is_stale);
    if !should_refresh {
        return sse_noop();
    }

    let conversation = match state
        .gh
        .refresh_pull_request_conversation(&repo_name, number)
        .await
    {
        Ok(conversation) => conversation,
        Err(error) => {
            return stream_error_response(
                error,
                if has_cached {
                    None
                } else {
                    Some(DETAIL_CONTENT_SELECTOR)
                },
            );
        }
    };

    let repo_context = crate::gh::models::RepoContext {
        name_with_owner: repo_name.clone(),
        url: format!("https://github.com/{repo_name}"),
        viewer_permission: "UNKNOWN".to_string(),
        default_branch: "main".to_string(),
    };

    let model = detail_page_model(&repo_context, conversation, false, None, &request);
    let html = match (PrDetailContentTemplate {
        model: model.clone(),
    })
    .render()
    {
        Ok(html) => html,
        Err(error) => {
            return stream_render_error(error.to_string(), Some(DETAIL_CONTENT_SELECTOR));
        }
    };

    sse_patch(DETAIL_CONTENT_SELECTOR, &html)
}

pub async fn stream_pr_changes(request: Request) -> Response {
    let state = app_state_snapshot();
    if let Err(error) = state.startup_ready() {
        return stream_error_response(error, Some(CHANGES_CONTENT_SELECTOR));
    }

    let repo_name = match repo_from_request(&request, state.startup_repo.as_ref()) {
        Ok(repo) => repo,
        Err(error) => return stream_error_response(error, Some(CHANGES_CONTENT_SELECTOR)),
    };
    let number = match parse_pr_number(&request) {
        Ok(number) => number,
        Err(error) => return stream_error_response(error, Some(CHANGES_CONTENT_SELECTOR)),
    };

    let maybe_conversation = match state
        .gh
        .cached_pull_request_conversation(&repo_name, number)
        .await
    {
        Ok(value) => value,
        Err(error) => return stream_error_response(error, Some(CHANGES_CONTENT_SELECTOR)),
    };
    let maybe_files = match state.gh.cached_pull_request_files(&repo_name, number).await {
        Ok(value) => value,
        Err(error) => return stream_error_response(error, Some(CHANGES_CONTENT_SELECTOR)),
    };

    let has_cached = maybe_conversation.is_some() && maybe_files.is_some();

    let should_refresh = match (&maybe_conversation, &maybe_files) {
        (Some(conversation), Some(files)) => conversation.is_stale || files.is_stale,
        _ => true,
    };
    if !should_refresh {
        return sse_noop();
    }

    let conversation = match state
        .gh
        .refresh_pull_request_conversation(&repo_name, number)
        .await
    {
        Ok(conversation) => conversation,
        Err(error) => {
            return stream_error_response(
                error,
                if has_cached {
                    None
                } else {
                    Some(CHANGES_CONTENT_SELECTOR)
                },
            );
        }
    };
    let files = match state.gh.refresh_pull_request_files(&repo_name, number).await {
        Ok(files) => files,
        Err(error) => {
            return stream_error_response(
                error,
                if has_cached {
                    None
                } else {
                    Some(CHANGES_CONTENT_SELECTOR)
                },
            );
        }
    };

    let repo_context = crate::gh::models::RepoContext {
        name_with_owner: repo_name.clone(),
        url: format!("https://github.com/{repo_name}"),
        viewer_permission: "UNKNOWN".to_string(),
        default_branch: "main".to_string(),
    };

    let model = changes_page_model(
        &repo_context,
        conversation.detail,
        files,
        false,
        None,
        &request,
    );
    let html = match (PrChangesContentTemplate {
        model: model.clone(),
    })
    .render()
    {
        Ok(html) => html,
        Err(error) => {
            return stream_render_error(error.to_string(), Some(CHANGES_CONTENT_SELECTOR));
        }
    };

    sse_patch(CHANGES_CONTENT_SELECTOR, &html)
}

fn stream_error_response(error: GhError, content_selector: Option<&str>) -> Response {
    let status = error.status_code();
    let message = escape_html(&error.message());
    let remediation = escape_html(error.remediation());

    let mut body = patch_event(
        STREAM_STATUS_SELECTOR,
        &format!(
            "<cp-alert tone=\"error\">Refresh failed ({status}): {message}</cp-alert>"
        ),
    );

    if let Some(selector) = content_selector {
        body.push_str(&patch_event(
            selector,
            &format!(
                "<cp-card class=\"error-panel\"><p class=\"eyebrow\">Status {status}</p><p>{message}</p><p class=\"meta\">How to fix: {remediation}</p></cp-card>"
            ),
        ));
    }

    sse_response(body)
}

fn stream_render_error(details: String, content_selector: Option<&str>) -> Response {
    let message = escape_html(&format!("failed rendering stream template: {details}"));
    let mut body = patch_event(
        STREAM_STATUS_SELECTOR,
        &format!("<cp-alert tone=\"error\">{message}</cp-alert>"),
    );

    if let Some(selector) = content_selector {
        body.push_str(&patch_event(
            selector,
            &format!(
                "<cp-card class=\"error-panel\"><p class=\"eyebrow\">Update Failed</p><p>{message}</p></cp-card>"
            ),
        ));
    }

    sse_response(body)
}

fn sse_response(body: String) -> Response {
    Response::ok()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("X-Accel-Buffering", "no")
        .body(body)
}

fn patch_event(selector: &str, html: &str) -> String {
    PatchElements::new(html)
        .selector(selector)
        .mode(ElementPatchMode::Inner)
        .into_datastar_event()
        .to_string()
}

fn clear_stream_status_event() -> String {
    patch_event(STREAM_STATUS_SELECTOR, "")
}

fn sse_noop() -> Response {
    sse_response(clear_stream_status_event())
}

fn sse_patch(selector: &str, html: &str) -> Response {
    let mut body = clear_stream_status_event();
    body.push_str(&patch_event(selector, html));
    sse_response(body)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

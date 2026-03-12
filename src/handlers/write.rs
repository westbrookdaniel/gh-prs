use crate::gh::client::{MergeMethod, PullRequestStateTransition, ReviewEvent};
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::format::render_gh_error;
use crate::handlers::forms::{
    CommentForm, MergeForm, ReviewForm, ReviewersForm, StateForm, parse_form,
};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};

#[tracing::instrument(
    name = "handler.submit_comment",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn submit_comment(request: Request) -> Response {
    let result = submit_comment_inner(&request).await;
    match result {
        Ok(location) => redirect(&location),
        Err(err) => render_gh_error(err),
    }
}

async fn submit_comment_inner(request: &Request) -> crate::gh::GhResult<String> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
    let number = parse_pr_number(request)?;
    let form =
        parse_form::<CommentForm>(request).map_err(|err| crate::gh::GhError::InvalidInput {
            field: "comment".to_string(),
            details: format!("unable to parse comment form: {err}"),
        })?;

    state
        .gh
        .submit_comment(&repo_name, number, &form.body)
        .await?;
    Ok(repo_pr_location(
        &repo_name,
        number,
        request.query.as_deref(),
    ))
}

#[tracing::instrument(
    name = "handler.submit_review",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn submit_review(request: Request) -> Response {
    let result = submit_review_inner(&request).await;
    match result {
        Ok(location) => redirect(&location),
        Err(err) => render_gh_error(err),
    }
}

async fn submit_review_inner(request: &Request) -> crate::gh::GhResult<String> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
    let number = parse_pr_number(request)?;
    let form =
        parse_form::<ReviewForm>(request).map_err(|err| crate::gh::GhError::InvalidInput {
            field: "review".to_string(),
            details: format!("unable to parse review form: {err}"),
        })?;
    let event = ReviewEvent::parse(&form.event)?;

    state
        .gh
        .submit_review(&repo_name, number, event, &form.body)
        .await?;

    Ok(repo_pr_location(
        &repo_name,
        number,
        request.query.as_deref(),
    ))
}

#[tracing::instrument(
    name = "handler.update_reviewers",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn update_reviewers(request: Request) -> Response {
    let result = update_reviewers_inner(&request).await;
    match result {
        Ok(location) => redirect(&location),
        Err(err) => render_gh_error(err),
    }
}

async fn update_reviewers_inner(request: &Request) -> crate::gh::GhResult<String> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
    let number = parse_pr_number(request)?;
    let form =
        parse_form::<ReviewersForm>(request).map_err(|err| crate::gh::GhError::InvalidInput {
            field: "reviewers".to_string(),
            details: format!("unable to parse reviewers form: {err}"),
        })?;

    let reviewers = form
        .reviewers
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty())
                .map(str::to_string)
                .collect::<Vec<String>>()
        })
        .collect::<Vec<String>>();

    state
        .gh
        .update_reviewers(&repo_name, number, reviewers)
        .await?;

    Ok(repo_pr_location(
        &repo_name,
        number,
        request.query.as_deref(),
    ))
}

#[tracing::instrument(
    name = "handler.merge_pull_request",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn merge_pull_request(request: Request) -> Response {
    let result = merge_pull_request_inner(&request).await;
    match result {
        Ok(location) => redirect(&location),
        Err(err) => render_gh_error(err),
    }
}

async fn merge_pull_request_inner(request: &Request) -> crate::gh::GhResult<String> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
    let number = parse_pr_number(request)?;
    let form =
        parse_form::<MergeForm>(request).map_err(|err| crate::gh::GhError::InvalidInput {
            field: "merge".to_string(),
            details: format!("unable to parse merge form: {err}"),
        })?;
    let method = MergeMethod::parse(&form.method)?;
    let delete_branch = form.delete_branch.as_deref() == Some("on");

    state
        .gh
        .merge_pull_request(&repo_name, number, method, delete_branch)
        .await?;

    Ok(repo_pr_location(
        &repo_name,
        number,
        request.query.as_deref(),
    ))
}

#[tracing::instrument(
    name = "handler.update_pull_request_state",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn update_pull_request_state(request: Request) -> Response {
    let result = update_pull_request_state_inner(&request).await;
    match result {
        Ok(location) => redirect(&location),
        Err(err) => render_gh_error(err),
    }
}

async fn update_pull_request_state_inner(request: &Request) -> crate::gh::GhResult<String> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
    let number = parse_pr_number(request)?;
    let form =
        parse_form::<StateForm>(request).map_err(|err| crate::gh::GhError::InvalidInput {
            field: "state".to_string(),
            details: format!("unable to parse state form: {err}"),
        })?;
    let transition = PullRequestStateTransition::parse(&form.state)?;

    state
        .gh
        .update_pull_request_state(&repo_name, number, transition)
        .await?;

    Ok(repo_pr_location(
        &repo_name,
        number,
        request.query.as_deref(),
    ))
}

fn redirect(location: &str) -> Response {
    Response::new(303, "See Other")
        .header("Location", location)
        .text_body("See Other")
}

fn repo_pr_location(repo: &str, number: u64, query: Option<&str>) -> String {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}")
    } else {
        format!("/prs/{number}")
    };

    if let Some(query) = query
        && !query.is_empty()
    {
        return format!("{base}?{query}");
    }

    base
}

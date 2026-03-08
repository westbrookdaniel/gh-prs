use crate::gh::client::{MergeMethod, PullRequestStateTransition, ReviewEvent};
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::flash::{redirect_to_repo_pr, redirect_with_flash};
use crate::handlers::forms::{
    CommentForm, MergeForm, ReviewForm, ReviewersForm, StateForm, parse_form,
};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::FlashMessageView;

#[tracing::instrument(
    name = "handler.submit_comment",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn submit_comment(request: Request) -> Response {
    let state = app_state_snapshot();

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
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(format!("Unable to parse comment form: {err}")),
                request.query.as_deref(),
            );
        }
    };

    let result = state
        .gh
        .submit_comment(&repo_name, number, &form.body)
        .await;

    match result {
        Ok(()) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::success("Comment posted."),
            request.query.as_deref(),
        ),
        Err(err) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::error(err.message()),
            request.query.as_deref(),
        ),
    }
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
    let state = app_state_snapshot();

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
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(format!("Unable to parse review form: {err}")),
                request.query.as_deref(),
            );
        }
    };

    let event = match ReviewEvent::parse(&form.event) {
        Ok(event) => event,
        Err(err) => {
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(err.message()),
                request.query.as_deref(),
            );
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
            redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::success(event_text),
                request.query.as_deref(),
            )
        }
        Err(err) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::error(err.message()),
            request.query.as_deref(),
        ),
    }
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
    let state = app_state_snapshot();

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

    let form = match parse_form::<ReviewersForm>(&request) {
        Ok(form) => form,
        Err(err) => {
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(format!("Unable to parse reviewers form: {err}")),
                request.query.as_deref(),
            );
        }
    };

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

    let result = state
        .gh
        .update_reviewers(&repo_name, number, reviewers)
        .await;

    match result {
        Ok(()) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::success("Reviewers updated."),
            request.query.as_deref(),
        ),
        Err(err) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::error(err.message()),
            request.query.as_deref(),
        ),
    }
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
    let state = app_state_snapshot();

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

    let form = match parse_form::<MergeForm>(&request) {
        Ok(form) => form,
        Err(err) => {
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(format!("Unable to parse merge form: {err}")),
                request.query.as_deref(),
            );
        }
    };

    let method = match MergeMethod::parse(&form.method) {
        Ok(method) => method,
        Err(err) => {
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(err.message()),
                request.query.as_deref(),
            );
        }
    };

    let delete_branch = form.delete_branch.as_deref() == Some("on");
    let result = state
        .gh
        .merge_pull_request(&repo_name, number, method, delete_branch)
        .await;

    match result {
        Ok(()) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::success("Merge requested."),
            request.query.as_deref(),
        ),
        Err(err) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::error(err.message()),
            request.query.as_deref(),
        ),
    }
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
    let state = app_state_snapshot();

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

    let form = match parse_form::<StateForm>(&request) {
        Ok(form) => form,
        Err(err) => {
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(format!("Unable to parse state form: {err}")),
                request.query.as_deref(),
            );
        }
    };

    let transition = match PullRequestStateTransition::parse(&form.state) {
        Ok(transition) => transition,
        Err(err) => {
            return redirect_to_repo_pr(
                &repo_name,
                number,
                FlashMessageView::error(err.message()),
                request.query.as_deref(),
            );
        }
    };

    let result = state
        .gh
        .update_pull_request_state(&repo_name, number, transition)
        .await;

    match result {
        Ok(()) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::success("Pull request state updated."),
            request.query.as_deref(),
        ),
        Err(err) => redirect_to_repo_pr(
            &repo_name,
            number,
            FlashMessageView::error(err.message()),
            request.query.as_deref(),
        ),
    }
}

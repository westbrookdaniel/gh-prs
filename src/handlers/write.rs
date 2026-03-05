use crate::gh::client::ReviewEvent;
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::flash::{redirect_to_repo_pr, redirect_with_flash};
use crate::handlers::forms::{CommentForm, ReviewForm, parse_form};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::FlashMessageView;

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

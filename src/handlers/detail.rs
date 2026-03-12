use crate::gh::models::{
    IssueComment, PullRequestReview, PullRequestReviewComment, RepoContext, StatusChecksSummary,
};
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::PageLoadMode;
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::PrDetailTemplate;
use crate::views::helpers::{
    build_detail_tabs, build_reviewer_statuses, default_list_back_href, markdown_to_html,
    merge_conversation_feed, repo_action_path, review_state_tone, with_query,
};
use crate::views::types::{
    IssueCommentView, PrDetailPageModel, PullRequestReviewView, ReviewCommentView, checks_view,
    merge_button_view, pr_header_view,
};

#[tracing::instrument(
    name = "handler.pull_request_detail",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn pull_request_detail(request: Request) -> Response {
    match build_detail_page_model(&request).await {
        Ok(model) => {
            let template = PrDetailTemplate { model };
            render_template(200, "OK", &template)
        }
        Err(err) => render_gh_error(err),
    }
}

async fn build_detail_page_model(request: &Request) -> crate::gh::GhResult<PrDetailPageModel> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
    let repo_context = RepoContext {
        name_with_owner: repo_name.clone(),
        url: format!("https://github.com/{repo_name}"),
        viewer_permission: "UNKNOWN".to_string(),
        default_branch: "main".to_string(),
    };
    let number = parse_pr_number(request)?;
    let query = crate::search::SearchArgs::from_request(request).to_query_string();
    let load_mode = PageLoadMode::from_request(request);

    let (conversation, needs_refresh) = if load_mode.bypass_cache() {
        (
            Some(
                state
                    .gh
                    .refresh_pull_request_conversation(&repo_name, number)
                    .await?,
            ),
            false,
        )
    } else {
        match state
            .gh
            .cached_pull_request_conversation(&repo_name, number)
            .await?
        {
            Some(cached) => (Some(cached.value), cached.is_stale),
            None => (None, true),
        }
    };

    let is_loading = conversation.is_none();
    let (header, reviewer_statuses, reviewer_options, checks, body_html, conversation_feed) =
        if let Some(conversation) = conversation.as_ref() {
            let requested_reviewers = conversation.detail.requested_reviewers.clone();
            let reviewer_statuses = build_reviewer_statuses(
                &requested_reviewers,
                &conversation.detail.latest_reviewer_decisions,
                &conversation.reviews,
            );

            let mut reviewer_options = requested_reviewers;
            for reviewer in &reviewer_statuses {
                reviewer_options.push(reviewer.reviewer.clone());
            }
            reviewer_options.sort();
            reviewer_options.dedup();

            (
                Some(pr_header_view(&repo_context, &conversation.detail)),
                reviewer_statuses,
                reviewer_options,
                checks_view(conversation.detail.checks.clone()),
                markdown_to_html(&conversation.detail.body),
                merge_conversation_feed(
                    map_issue_comments(conversation.issue_comments.clone()),
                    map_reviews(conversation.reviews.clone()),
                    map_review_comments(conversation.review_comments.clone()),
                ),
            )
        } else {
            (
                None,
                Vec::new(),
                Vec::new(),
                checks_view(StatusChecksSummary::default()),
                String::new(),
                Vec::new(),
            )
        };

    let merge_button = conversation
        .as_ref()
        .map(|conversation| merge_button_view(Some(&conversation.detail)))
        .unwrap_or(None);

    Ok(PrDetailPageModel {
        page_title: format!("PR #{number}"),
        refresh_path: with_query(request.path.clone(), request.query.as_deref()),
        needs_refresh: !load_mode.bypass_cache() && needs_refresh,
        is_loading,
        header,
        reviewer_statuses,
        reviewer_options,
        checks,
        body_html,
        conversation_feed,
        comment_post_path: repo_action_path(
            &repo_context.name_with_owner,
            number,
            "comment",
            query.as_deref(),
        ),
        review_post_path: repo_action_path(
            &repo_context.name_with_owner,
            number,
            "review",
            query.as_deref(),
        ),
        reviewers_post_path: repo_action_path(
            &repo_context.name_with_owner,
            number,
            "reviewers",
            query.as_deref(),
        ),
        merge_post_path: repo_action_path(
            &repo_context.name_with_owner,
            number,
            "merge",
            query.as_deref(),
        ),
        state_post_path: repo_action_path(
            &repo_context.name_with_owner,
            number,
            "state",
            query.as_deref(),
        ),
        back_to_list_href: default_list_back_href(query.as_deref()),
        tabs: build_detail_tabs(&repo_context, number, query.as_deref(), false),
        merge_button,
    })
}

fn map_issue_comments(values: Vec<IssueComment>) -> Vec<IssueCommentView> {
    values
        .into_iter()
        .map(|value| IssueCommentView {
            author: value.author,
            body_html: markdown_to_html(&value.body),
            created_at: value.created_at,
            updated_at: value.updated_at,
            url: value.url,
        })
        .collect()
}

fn map_reviews(values: Vec<PullRequestReview>) -> Vec<PullRequestReviewView> {
    values
        .into_iter()
        .map(|value| {
            let state = value.state;
            PullRequestReviewView {
                author: value.author,
                tone: review_state_tone(&state),
                state,
                body_html: markdown_to_html(&value.body),
                submitted_at: value.submitted_at,
                url: value.url,
            }
        })
        .collect()
}

fn map_review_comments(values: Vec<PullRequestReviewComment>) -> Vec<ReviewCommentView> {
    values
        .into_iter()
        .map(|value| {
            let line_label = match (value.line, value.original_line) {
                (Some(line), Some(original)) => format!("line {} (original {})", line, original),
                (Some(line), None) => format!("line {}", line),
                (None, Some(original)) => format!("original line {}", original),
                (None, None) => "line unavailable".to_string(),
            };

            ReviewCommentView {
                author: value.author,
                body_html: markdown_to_html(&value.body),
                path: value.path,
                line_label,
                created_at: value.created_at,
                updated_at: value.updated_at,
                url: value.url,
            }
        })
        .collect()
}

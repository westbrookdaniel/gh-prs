use crate::gh::models::{
    IssueComment, PullRequestConversation, PullRequestDetail, PullRequestFile, PullRequestReview,
    PullRequestReviewComment, PullRequestSearchItem, RepoContext,
};
use crate::http::Request;
use crate::search::SearchArgs;
use crate::views::helpers::{
    build_detail_tabs, build_list_tabs, build_reviewer_statuses, default_list_back_href,
    diff_files_view, markdown_to_html, merge_conversation_feed, repo_action_path,
    review_state_tone, sort_controls, with_query,
};
use crate::views::types::{
    checks_view, pr_header_view, ErrorPageModel, FlashMessageView, IssueCommentView, Loadable,
    PrChangesPageModel, PrDetailPageModel, PrListPageModel, PullRequestReviewView,
    ReviewCommentView,
};

pub struct ListPageModelInput<'a> {
    pub query: &'a SearchArgs,
    pub repo_options: Loadable<Vec<String>>,
    pub results: Loadable<Vec<PullRequestSearchItem>>,
    pub needs_refresh: bool,
    pub flash: Option<FlashMessageView>,
    pub request: &'a Request,
}

pub fn list_page_model(input: ListPageModelInput<'_>) -> PrListPageModel {
    let ListPageModelInput {
        query,
        repo_options,
        results,
        needs_refresh,
        flash,
        request,
    } = input;

    PrListPageModel {
        page_title: "Pull Requests Across Your Repos".to_string(),
        refresh_path: with_query(request.path.clone(), request.query.as_deref()),
        needs_refresh,
        query: query.clone(),
        repo_options,
        results,
        sort_controls: sort_controls(query),
        flash,
        tabs: build_list_tabs(query),
    }
}

pub fn detail_page_model(
    repo: &RepoContext,
    number: u64,
    conversation: Loadable<PullRequestConversation>,
    needs_refresh: bool,
    flash: Option<FlashMessageView>,
    request: &Request,
) -> PrDetailPageModel {
    let query = SearchArgs::from_request(request).to_query_string();

    let (header, reviewer_statuses, reviewer_options, checks, body_html, conversation_feed) =
        if let Some(conversation_value) = conversation.value.as_ref() {
            let requested_reviewers = conversation_value.detail.requested_reviewers.clone();
            let reviewer_statuses = build_reviewer_statuses(
                &requested_reviewers,
                &conversation_value.detail.latest_reviewer_decisions,
                &conversation_value.reviews,
            );

            let mut reviewer_options = requested_reviewers;
            for status in &reviewer_statuses {
                reviewer_options.push(status.reviewer.clone());
            }
            reviewer_options.sort();
            reviewer_options.dedup();

            let conversation_feed = merge_conversation_feed(
                map_issue_comments(conversation_value.issue_comments.clone()),
                map_reviews(conversation_value.reviews.clone()),
                map_review_comments(conversation_value.review_comments.clone()),
            );

            (
                Some(pr_header_view(repo, &conversation_value.detail)),
                reviewer_statuses,
                reviewer_options,
                checks_view(conversation_value.detail.checks.clone()),
                markdown_to_html(&conversation_value.detail.body),
                conversation_feed,
            )
        } else {
            (
                None,
                Vec::new(),
                Vec::new(),
                checks_view(crate::gh::models::StatusChecksSummary::default()),
                String::new(),
                Vec::new(),
            )
        };

    PrDetailPageModel {
        page_title: format!("PR #{number}"),
        refresh_path: with_query(request.path.clone(), request.query.as_deref()),
        needs_refresh,
        repo: repo.clone(),
        conversation,
        header,
        reviewer_statuses,
        reviewer_options,
        checks,
        body_html,
        conversation_feed,
        comment_post_path: repo_action_path(
            &repo.name_with_owner,
            number,
            "comment",
            query.as_deref(),
        ),
        review_post_path: repo_action_path(
            &repo.name_with_owner,
            number,
            "review",
            query.as_deref(),
        ),
        reviewers_post_path: repo_action_path(
            &repo.name_with_owner,
            number,
            "reviewers",
            query.as_deref(),
        ),
        merge_post_path: repo_action_path(&repo.name_with_owner, number, "merge", query.as_deref()),
        state_post_path: repo_action_path(&repo.name_with_owner, number, "state", query.as_deref()),
        flash,
        back_to_list_href: default_list_back_href(query.as_deref()),
        tabs: build_detail_tabs(repo, number, query.as_deref(), false),
    }
}

pub fn changes_page_model(
    repo: &RepoContext,
    number: u64,
    detail: Loadable<PullRequestDetail>,
    files: Loadable<Vec<PullRequestFile>>,
    needs_refresh: bool,
    flash: Option<FlashMessageView>,
    request: &Request,
) -> PrChangesPageModel {
    let query = SearchArgs::from_request(request).to_query_string();
    let (tree_items, rendered_files) = files
        .value
        .clone()
        .map(diff_files_view)
        .unwrap_or_else(|| (Vec::new(), Vec::new()));
    let header = detail
        .value
        .as_ref()
        .map(|detail| pr_header_view(repo, detail));

    PrChangesPageModel {
        page_title: format!("PR #{number} Changes"),
        refresh_path: with_query(request.path.clone(), request.query.as_deref()),
        needs_refresh,
        repo: repo.clone(),
        detail,
        header,
        files,
        rendered_files,
        tree_items,
        flash,
        back_to_list_href: default_list_back_href(query.as_deref()),
        state_post_path: repo_action_path(&repo.name_with_owner, number, "state", query.as_deref()),
        tabs: build_detail_tabs(repo, number, query.as_deref(), true),
    }
}

pub fn error_page_model(error: &crate::gh::GhError) -> ErrorPageModel {
    ErrorPageModel {
        page_title: error.title().to_string(),
        heading: error.title().to_string(),
        status_code: error.status_code(),
        message: error.message(),
        remediation: error.remediation().to_string(),
        details: error.details(),
    }
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
            let tone = review_state_tone(&state);

            PullRequestReviewView {
                author: value.author,
                state,
                tone,
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

use crate::gh::models::{
    DEFAULT_SEARCH_LIMIT, IssueComment, PreflightDiagnostics, PullRequestConversation,
    PullRequestFile, PullRequestReview, PullRequestReviewComment, PullRequestSearchItem,
    RepoContext,
};
use crate::search::SearchArgs;
use crate::views::helpers::{
    author_avatar_url, author_initial, avatar_style_from_author, build_detail_tabs,
    build_list_tabs, build_reviewer_statuses, default_list_back_href, detail_path_from_repo,
    diff_files_view, format_timestamp, markdown_to_html, merge_state_explainer, merge_state_tone,
    merge_state_tooltip, pr_state_tone, pr_state_tooltip, repo_action_path, review_decision_tone,
    review_decision_tooltip, review_state_tone, sort_controls, state_label,
};
use crate::views::types::{
    DetailHeaderView, ErrorPageModel, FilterFormView, IssueCommentView, PrChangesPageModel,
    PrDetailPageModel, PrListPageModel, PrListRowView, PullRequestReviewView, RepoOptionView,
    ReviewCommentView, checks_view, diagnostics_label, source_label,
};

pub fn list_page_model(
    repo: Option<&RepoContext>,
    diagnostics: Option<&PreflightDiagnostics>,
    query: &SearchArgs,
    available_repos: Vec<String>,
    items: Vec<PullRequestSearchItem>,
    flash: Option<crate::views::types::FlashMessageView>,
    _request: &crate::http::Request,
) -> PrListPageModel {
    let original_query = query.to_query_string();
    let rows: Vec<PrListRowView> = items
        .into_iter()
        .map(|item| PrListRowView {
            repo_name_with_owner: item.repository_name_with_owner.clone(),
            number: item.number,
            detail_path: detail_path_from_repo(
                &item.repository_name_with_owner,
                item.number,
                original_query.as_deref(),
            ),
            title: item.title,
            state_label: state_label(item.state.clone(), item.is_draft),
            state_tone: pr_state_tone(&item.state, item.is_draft),
            state_tooltip: pr_state_tooltip(&item.state, item.is_draft),
            author: item.author.clone(),
            author_avatar_url: author_avatar_url(&item.author, &item.author_avatar_url),
            author_avatar_style: avatar_style_from_author(&item.author),
            author_initial: author_initial(&item.author),
            updated_at: format_timestamp(&item.updated_at),
            comment_count: item.comment_count,
        })
        .collect();

    let (gh_version, authenticated_hosts) = diagnostics_label(diagnostics);

    PrListPageModel {
        page_title: "Pull Requests Across Your Repos".to_string(),
        source_label: source_label(repo),
        gh_version,
        authenticated_hosts,
        row_count: rows.len(),
        repo_options: available_repos
            .iter()
            .map(|value| RepoOptionView {
                value: value.clone(),
                selected: query.repos.contains(value),
            })
            .collect(),
        filters: FilterFormView {
            repos: query.repos.clone(),
            status: query.status.as_query_value().to_string(),
            title: query.title.clone().unwrap_or_default(),
            author: query.author.clone().unwrap_or_default(),
            sort: query.sort.as_query_value().to_string(),
            order: query.order.as_query_value().to_string(),
        },
        sort_controls: sort_controls(query),
        has_results_limit_warning: query.limit >= DEFAULT_SEARCH_LIMIT
            && rows.len() >= DEFAULT_SEARCH_LIMIT,
        tabs: build_list_tabs(query),
        rows,
        flash,
    }
}

pub fn detail_page_model(
    repo: &RepoContext,
    conversation: PullRequestConversation,
    flash: Option<crate::views::types::FlashMessageView>,
    request: &crate::http::Request,
) -> PrDetailPageModel {
    let query = SearchArgs::from_request(request).to_query_string();
    let PullRequestConversation {
        detail,
        issue_comments,
        reviews,
        review_comments,
    } = conversation;

    let header = detail_header(&detail);
    let requested_reviewers = if detail.requested_reviewers.is_empty() {
        Vec::new()
    } else {
        detail.requested_reviewers.clone()
    };
    let reviewer_statuses = build_reviewer_statuses(
        &requested_reviewers,
        &detail.latest_reviewer_decisions,
        &reviews,
    );

    PrDetailPageModel {
        page_title: format!("PR #{}", detail.number),
        repo_name: repo.name_with_owner.clone(),
        repo_url: repo.url.clone(),
        tabs: build_detail_tabs(repo, detail.number, query.as_deref(), false),
        back_to_list_href: default_list_back_href(query.as_deref()),
        header,
        reviewer_statuses,
        checks: checks_view(detail.checks),
        body_html: markdown_to_html(&detail.body),
        issue_comments: map_issue_comments(issue_comments),
        reviews: map_reviews(reviews),
        review_comments: map_review_comments(review_comments),
        comment_post_path: repo_action_path(
            &repo.name_with_owner,
            detail.number,
            "comment",
            query.as_deref(),
        ),
        review_post_path: repo_action_path(
            &repo.name_with_owner,
            detail.number,
            "review",
            query.as_deref(),
        ),
        flash,
    }
}

pub fn changes_page_model(
    repo: &RepoContext,
    detail: crate::gh::models::PullRequestDetail,
    files: Vec<PullRequestFile>,
    flash: Option<crate::views::types::FlashMessageView>,
    request: &crate::http::Request,
) -> PrChangesPageModel {
    let query = SearchArgs::from_request(request).to_query_string();
    let header = detail_header(&detail);
    let (tree_items, files) = diff_files_view(files);

    PrChangesPageModel {
        page_title: format!("PR #{} Changes", detail.number),
        repo_name: repo.name_with_owner.clone(),
        repo_url: repo.url.clone(),
        tabs: build_detail_tabs(repo, detail.number, query.as_deref(), true),
        back_to_list_href: default_list_back_href(query.as_deref()),
        header,
        tree_items,
        files,
        flash,
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

fn detail_header(detail: &crate::gh::models::PullRequestDetail) -> DetailHeaderView {
    DetailHeaderView {
        number: detail.number,
        title: detail.title.clone(),
        state_label: detail.state.clone(),
        state_tone: pr_state_tone(&detail.state, detail.is_draft),
        state_tooltip: pr_state_tooltip(&detail.state, detail.is_draft),
        is_draft: detail.is_draft,
        draft_label: if detail.is_draft {
            "DRAFT".to_string()
        } else {
            "READY".to_string()
        },
        draft_tooltip: "Draft pull request; not ready to merge".to_string(),
        author: detail.author.clone(),
        created_at: format_timestamp(&detail.created_at),
        updated_at: format_timestamp(&detail.updated_at),
        url: detail.url.clone(),
        base_ref_name: detail.base_ref_name.clone(),
        head_ref_name: detail.head_ref_name.clone(),
        merge_state_status: detail.merge_state_status.clone(),
        merge_state_tone: merge_state_tone(&detail.merge_state_status, &detail.mergeable),
        merge_state_tooltip: merge_state_tooltip(&detail.merge_state_status, &detail.mergeable),
        merge_state_explainer: merge_state_explainer(&detail.merge_state_status),
        mergeable: detail.mergeable.clone(),
        review_decision: detail
            .review_decision
            .clone()
            .unwrap_or_else(|| "NONE".to_string()),
        review_decision_tone: review_decision_tone(
            detail.review_decision.as_deref().unwrap_or("NONE"),
        ),
        review_decision_tooltip: review_decision_tooltip(
            detail.review_decision.as_deref().unwrap_or("NONE"),
        ),
        commit_count: detail.commit_count,
        file_count: detail.file_count,
    }
}

fn map_issue_comments(values: Vec<IssueComment>) -> Vec<IssueCommentView> {
    values
        .into_iter()
        .map(|value| IssueCommentView {
            author: value.author,
            body_html: markdown_to_html(&value.body),
            created_at: format_timestamp(&value.created_at),
            updated_at: format_timestamp(&value.updated_at),
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
                submitted_at: format_timestamp(&value.submitted_at),
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
                created_at: format_timestamp(&value.created_at),
                updated_at: format_timestamp(&value.updated_at),
                url: value.url,
            }
        })
        .collect()
}

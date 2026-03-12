use crate::gh::client::GhClient;
use crate::gh::models::{
    IssueComment, PullRequestConversation, PullRequestReview, PullRequestReviewComment,
    RepoContext, ReviewerDecision, StatusChecksSummary,
};
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::PageLoadMode;
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::PrDetailTemplate;
use crate::views::helpers::{format_timestamp, markdown_to_html};
use crate::views::paths::{detail_path, list_path, repo_action_path, with_query};
use crate::views::types::{
    ConversationFeedItemView, IssueCommentView, PrDetailPageModel, PullRequestReviewView,
    ReviewCommentView, ReviewerStatusView, checks_view, merge_button_view, pr_header_view,
};
use chrono::DateTime;
use std::collections::BTreeMap;

#[tracing::instrument(
    name = "handler.pull_request_detail",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn pull_request_detail(request: Request) -> Response {
    match render_detail_page(&request).await {
        Ok(response) => response,
        Err(err) => render_gh_error(err),
    }
}

async fn render_detail_page(request: &Request) -> crate::gh::GhResult<Response> {
    let context = DetailPageContext::from_request(request)?;
    let data = if context.bypass_cache {
        load_detail_fresh(&context).await?
    } else {
        load_detail_cached(&context).await?
    };

    let template = PrDetailTemplate {
        model: build_detail_page_model(&context, data),
    };

    Ok(render_template(200, "OK", &template))
}

struct DetailPageContext {
    gh: GhClient,
    repo: RepoContext,
    number: u64,
    query_string: Option<String>,
    refresh_path: String,
    bypass_cache: bool,
}

impl DetailPageContext {
    fn from_request(request: &Request) -> crate::gh::GhResult<Self> {
        let state = app_state_snapshot();
        state.startup_ready()?;

        let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
        let number = parse_pr_number(request)?;
        let query_string = crate::search::SearchArgs::from_request(request).to_query_string();

        Ok(Self {
            gh: state.gh,
            repo: RepoContext {
                name_with_owner: repo_name.clone(),
                url: format!("https://github.com/{repo_name}"),
                viewer_permission: "UNKNOWN".to_string(),
                default_branch: "main".to_string(),
            },
            number,
            query_string,
            refresh_path: with_query(request.path.clone(), request.query.as_deref()),
            bypass_cache: PageLoadMode::from_request(request).bypass_cache(),
        })
    }
}

struct DetailData {
    conversation: Option<PullRequestConversation>,
    needs_refresh: bool,
}

async fn load_detail_fresh(context: &DetailPageContext) -> crate::gh::GhResult<DetailData> {
    Ok(DetailData {
        conversation: Some(
            context
                .gh
                .refresh_pull_request_conversation(&context.repo.name_with_owner, context.number)
                .await?,
        ),
        needs_refresh: false,
    })
}

async fn load_detail_cached(context: &DetailPageContext) -> crate::gh::GhResult<DetailData> {
    let cached = context
        .gh
        .cached_pull_request_conversation(&context.repo.name_with_owner, context.number)
        .await?;

    Ok(match cached {
        Some(cached) => DetailData {
            conversation: Some(cached.value),
            needs_refresh: cached.is_stale,
        },
        None => DetailData {
            conversation: None,
            needs_refresh: true,
        },
    })
}

fn build_detail_page_model(context: &DetailPageContext, data: DetailData) -> PrDetailPageModel {
    let is_loading = data.conversation.is_none();
    let model_data = data
        .conversation
        .as_ref()
        .map(build_conversation_model_data)
        .unwrap_or_else(empty_conversation_model_data);

    PrDetailPageModel {
        page_title: format!("PR #{}", context.number),
        refresh_path: context.refresh_path.clone(),
        needs_refresh: !context.bypass_cache && data.needs_refresh,
        is_loading,
        header: data
            .conversation
            .as_ref()
            .map(|conversation| pr_header_view(&context.repo, &conversation.detail)),
        reviewer_statuses: model_data.reviewer_statuses,
        reviewer_options: model_data.reviewer_options,
        checks: model_data.checks,
        body_html: model_data.body_html,
        conversation_feed: model_data.conversation_feed,
        comment_post_path: repo_action_path(
            &context.repo.name_with_owner,
            context.number,
            "comment",
            context.query_string.as_deref(),
        ),
        review_post_path: repo_action_path(
            &context.repo.name_with_owner,
            context.number,
            "review",
            context.query_string.as_deref(),
        ),
        reviewers_post_path: repo_action_path(
            &context.repo.name_with_owner,
            context.number,
            "reviewers",
            context.query_string.as_deref(),
        ),
        merge_post_path: repo_action_path(
            &context.repo.name_with_owner,
            context.number,
            "merge",
            context.query_string.as_deref(),
        ),
        state_post_path: repo_action_path(
            &context.repo.name_with_owner,
            context.number,
            "state",
            context.query_string.as_deref(),
        ),
        back_to_list_href: list_path(context.query_string.as_deref()),
        tabs: detail_tabs(
            &context.repo.name_with_owner,
            context.number,
            context.query_string.as_deref(),
            false,
        ),
        merge_button: data
            .conversation
            .as_ref()
            .and_then(|conversation| merge_button_view(Some(&conversation.detail))),
    }
}

struct DetailModelData {
    reviewer_statuses: Vec<ReviewerStatusView>,
    reviewer_options: Vec<String>,
    checks: crate::views::types::ChecksSummaryView,
    body_html: String,
    conversation_feed: Vec<ConversationFeedItemView>,
}

fn build_conversation_model_data(conversation: &PullRequestConversation) -> DetailModelData {
    let reviewer_statuses = reviewer_statuses(
        &conversation.detail.requested_reviewers,
        &conversation.detail.latest_reviewer_decisions,
        &conversation.reviews,
    );

    let mut reviewer_options = conversation.detail.requested_reviewers.clone();
    reviewer_options.extend(
        reviewer_statuses
            .iter()
            .map(|reviewer| reviewer.reviewer.clone()),
    );
    reviewer_options.sort();
    reviewer_options.dedup();

    DetailModelData {
        reviewer_statuses,
        reviewer_options,
        checks: checks_view(conversation.detail.checks.clone()),
        body_html: markdown_to_html(&conversation.detail.body),
        conversation_feed: conversation_feed(conversation),
    }
}

fn empty_conversation_model_data() -> DetailModelData {
    DetailModelData {
        reviewer_statuses: Vec::new(),
        reviewer_options: Vec::new(),
        checks: checks_view(StatusChecksSummary::default()),
        body_html: String::new(),
        conversation_feed: Vec::new(),
    }
}

fn reviewer_statuses(
    requested_reviewers: &[String],
    latest_reviewer_decisions: &[ReviewerDecision],
    reviews: &[PullRequestReview],
) -> Vec<ReviewerStatusView> {
    let mut by_reviewer = BTreeMap::new();

    for reviewer in requested_reviewers {
        if reviewer == "none" {
            continue;
        }

        by_reviewer.insert(
            reviewer.clone(),
            ReviewerStatusView {
                reviewer: reviewer.clone(),
                state: "REVIEW_REQUIRED".to_string(),
                tone: review_decision_tone("REVIEW_REQUIRED"),
                state_tooltip: review_state_tooltip("REVIEW_REQUIRED"),
                submitted_at: "Pending".to_string(),
            },
        );
    }

    for decision in latest_reviewer_decisions {
        by_reviewer.insert(
            decision.reviewer.clone(),
            ReviewerStatusView {
                reviewer: decision.reviewer.clone(),
                state: decision.state.clone(),
                tone: review_decision_tone(&decision.state),
                state_tooltip: review_decision_tooltip(&decision.state),
                submitted_at: format_timestamp(&decision.submitted_at),
            },
        );
    }

    for review in reviews {
        by_reviewer
            .entry(review.author.clone())
            .or_insert_with(|| ReviewerStatusView {
                reviewer: review.author.clone(),
                state: review.state.clone(),
                tone: review_state_tone(&review.state),
                state_tooltip: review_state_tooltip(&review.state),
                submitted_at: format_timestamp(&review.submitted_at),
            });
    }

    by_reviewer.into_values().collect()
}

fn conversation_feed(conversation: &PullRequestConversation) -> Vec<ConversationFeedItemView> {
    let mut feed = Vec::new();

    for comment in map_issue_comments(&conversation.issue_comments) {
        feed.push((
            sort_key_timestamp(&comment.created_at),
            ConversationFeedItemView {
                author: comment.author,
                kind_label: "Comment".to_string(),
                context_label: String::new(),
                body_html: comment.body_html,
                timestamp: format_timestamp(&comment.created_at),
                url: comment.url,
            },
        ));
    }

    for review in map_reviews(&conversation.reviews) {
        feed.push((
            sort_key_timestamp(&review.submitted_at),
            ConversationFeedItemView {
                author: review.author,
                kind_label: format!("Review · {}", review.state),
                context_label: String::new(),
                body_html: review.body_html,
                timestamp: format_timestamp(&review.submitted_at),
                url: review.url,
            },
        ));
    }

    for comment in map_review_comments(&conversation.review_comments) {
        feed.push((
            sort_key_timestamp(&comment.created_at),
            ConversationFeedItemView {
                author: comment.author,
                kind_label: "Review Comment".to_string(),
                context_label: format!("{} · {}", comment.path, comment.line_label),
                body_html: comment.body_html,
                timestamp: format_timestamp(&comment.created_at),
                url: comment.url,
            },
        ));
    }

    feed.sort_by(|left, right| left.0.cmp(&right.0));
    feed.into_iter().map(|(_, item)| item).collect()
}

fn map_issue_comments(values: &[IssueComment]) -> Vec<IssueCommentView> {
    values
        .iter()
        .map(|value| IssueCommentView {
            author: value.author.clone(),
            body_html: markdown_to_html(&value.body),
            created_at: value.created_at.clone(),
            updated_at: value.updated_at.clone(),
            url: value.url.clone(),
        })
        .collect()
}

fn map_reviews(values: &[PullRequestReview]) -> Vec<PullRequestReviewView> {
    values
        .iter()
        .map(|value| PullRequestReviewView {
            author: value.author.clone(),
            tone: review_state_tone(&value.state),
            state: value.state.clone(),
            body_html: markdown_to_html(&value.body),
            submitted_at: value.submitted_at.clone(),
            url: value.url.clone(),
        })
        .collect()
}

fn map_review_comments(values: &[PullRequestReviewComment]) -> Vec<ReviewCommentView> {
    values
        .iter()
        .map(|value| ReviewCommentView {
            author: value.author.clone(),
            body_html: markdown_to_html(&value.body),
            path: value.path.clone(),
            line_label: review_comment_line_label(value),
            created_at: value.created_at.clone(),
            updated_at: value.updated_at.clone(),
            url: value.url.clone(),
        })
        .collect()
}

fn review_comment_line_label(value: &PullRequestReviewComment) -> String {
    match (value.line, value.original_line) {
        (Some(line), Some(original)) => format!("line {} (original {})", line, original),
        (Some(line), None) => format!("line {}", line),
        (None, Some(original)) => format!("original line {}", original),
        (None, None) => "line unavailable".to_string(),
    }
}

fn detail_tabs(
    repo: &str,
    number: u64,
    query: Option<&str>,
    is_changes: bool,
) -> Vec<crate::views::types::DetailTabView> {
    vec![
        crate::views::types::DetailTabView {
            label: "Conversation".to_string(),
            href: detail_path(repo, number, query),
            selected: !is_changes,
        },
        crate::views::types::DetailTabView {
            label: "Changes".to_string(),
            href: crate::views::paths::changes_path(repo, number, query),
            selected: is_changes,
        },
    ]
}

fn review_decision_tone(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "state-approved".to_string(),
        "CHANGES_REQUESTED" => "state-warning".to_string(),
        "REVIEW_REQUIRED" => "state-open".to_string(),
        _ => "state-neutral".to_string(),
    }
}

fn review_decision_tooltip(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "Required reviews approved".to_string(),
        "CHANGES_REQUESTED" => "Changes requested by at least one reviewer".to_string(),
        "REVIEW_REQUIRED" => "A review is still required".to_string(),
        "NONE" | "" => "No review decision yet".to_string(),
        _ => "Review decision state".to_string(),
    }
}

fn review_state_tone(state: &str) -> String {
    match state.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "state-approved".to_string(),
        "CHANGES_REQUESTED" => "state-warning".to_string(),
        "COMMENTED" => "state-open".to_string(),
        _ => "state-neutral".to_string(),
    }
}

fn review_state_tooltip(state: &str) -> String {
    match state.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "Reviewer approved".to_string(),
        "CHANGES_REQUESTED" => "Reviewer requested changes".to_string(),
        "COMMENTED" => "Reviewer left comments".to_string(),
        "REVIEW_REQUIRED" => "Review requested".to_string(),
        _ => "Review state".to_string(),
    }
}

fn sort_key_timestamp(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.timestamp())
        .unwrap_or_default()
}

use askama::Template;
use std::cmp;

use crate::gh::models::{
    IssueComment, PreflightDiagnostics, PullRequestConversation, PullRequestDetail,
    PullRequestListItem, PullRequestReview, PullRequestReviewComment, RepoContext,
    ReviewerDecision, StatusChecksSummary,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashMessageView {
    pub kind: String,
    pub message: String,
}

impl FlashMessageView {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            kind: "success".to_string(),
            message: clamp_flash(message.into()),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            kind: "error".to_string(),
            message: clamp_flash(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrListRowView {
    pub number: u64,
    pub detail_path: String,
    pub title: String,
    pub state_label: String,
    pub author: String,
    pub review_decision: String,
    pub updated_at: String,
    pub comment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewDecisionView {
    pub reviewer: String,
    pub state: String,
    pub submitted_at: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueCommentView {
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReviewView {
    pub author: String,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCommentView {
    pub author: String,
    pub body: String,
    pub path: String,
    pub line_label: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksSummaryView {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub pending: usize,
    pub neutral: usize,
    pub headline: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailHeaderView {
    pub number: u64,
    pub title: String,
    pub state_label: String,
    pub draft_label: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub merge_state_status: String,
    pub mergeable: String,
    pub review_decision: String,
    pub commit_count: usize,
    pub file_count: usize,
}

#[derive(Template)]
#[template(path = "pages/pr_list.html")]
pub struct PrListTemplate {
    pub page_title: String,
    pub repo_name: String,
    pub repo_url: String,
    pub default_branch: String,
    pub gh_version: String,
    pub authenticated_hosts: String,
    pub rows: Vec<PrListRowView>,
    pub flash: Option<FlashMessageView>,
}

#[derive(Template)]
#[template(path = "pages/pr_detail.html")]
pub struct PrDetailTemplate {
    pub page_title: String,
    pub repo_name: String,
    pub repo_url: String,
    pub header: DetailHeaderView,
    pub requested_reviewers: Vec<String>,
    pub reviewer_decisions: Vec<ReviewDecisionView>,
    pub checks: ChecksSummaryView,
    pub body: String,
    pub issue_comments: Vec<IssueCommentView>,
    pub reviews: Vec<PullRequestReviewView>,
    pub review_comments: Vec<ReviewCommentView>,
    pub comment_post_path: String,
    pub review_post_path: String,
    pub flash: Option<FlashMessageView>,
}

#[derive(Template)]
#[template(path = "pages/error.html")]
pub struct ErrorTemplate {
    pub page_title: String,
    pub heading: String,
    pub status_code: u16,
    pub message: String,
    pub remediation: String,
    pub details: Option<String>,
}

impl PrListTemplate {
    pub fn from_pull_requests(
        repo: &RepoContext,
        diagnostics: Option<&PreflightDiagnostics>,
        items: Vec<PullRequestListItem>,
        flash: Option<FlashMessageView>,
    ) -> Self {
        let rows = items
            .into_iter()
            .map(|item| PrListRowView {
                number: item.number,
                detail_path: format!("/prs/{}", item.number),
                title: item.title,
                state_label: state_label(item.state, item.is_draft),
                author: item.author,
                review_decision: item.review_decision.unwrap_or_else(|| "NONE".to_string()),
                updated_at: item.updated_at,
                comment_count: item.comment_count,
            })
            .collect();

        let (gh_version, authenticated_hosts) = diagnostics
            .map(|value| {
                (
                    value.gh_version.clone(),
                    value.authenticated_hosts.join(", "),
                )
            })
            .unwrap_or_else(|| ("unknown".to_string(), "none".to_string()));

        Self {
            page_title: format!("{} pull requests", repo.name_with_owner),
            repo_name: repo.name_with_owner.clone(),
            repo_url: repo.url.clone(),
            default_branch: repo.default_branch.clone(),
            gh_version,
            authenticated_hosts,
            rows,
            flash,
        }
    }
}

impl PrDetailTemplate {
    pub fn from_conversation(
        repo: &RepoContext,
        conversation: PullRequestConversation,
        flash: Option<FlashMessageView>,
    ) -> Self {
        let PullRequestConversation {
            detail,
            issue_comments,
            reviews,
            review_comments,
        } = conversation;

        let header = header_view(detail.clone());
        let requested_reviewers = if detail.requested_reviewers.is_empty() {
            vec!["none".to_string()]
        } else {
            detail.requested_reviewers.clone()
        };

        let reviewer_decisions = reviewer_decision_views(detail.latest_reviewer_decisions);
        let checks = checks_view(detail.checks);

        Self {
            page_title: format!("PR #{}", detail.number),
            repo_name: repo.name_with_owner.clone(),
            repo_url: repo.url.clone(),
            header,
            requested_reviewers,
            reviewer_decisions,
            checks,
            body: detail.body,
            issue_comments: issue_comment_views(issue_comments),
            reviews: review_views(reviews),
            review_comments: review_comment_views(review_comments),
            comment_post_path: format!("/prs/{}/comment", detail.number),
            review_post_path: format!("/prs/{}/review", detail.number),
            flash,
        }
    }
}

impl ErrorTemplate {
    pub fn new(
        status_code: u16,
        heading: impl Into<String>,
        message: impl Into<String>,
        remediation: impl Into<String>,
        details: Option<String>,
    ) -> Self {
        let heading = heading.into();
        Self {
            page_title: heading.clone(),
            heading,
            status_code,
            message: message.into(),
            remediation: remediation.into(),
            details,
        }
    }
}

fn header_view(detail: PullRequestDetail) -> DetailHeaderView {
    DetailHeaderView {
        number: detail.number,
        title: detail.title,
        state_label: detail.state,
        draft_label: if detail.is_draft {
            "DRAFT".to_string()
        } else {
            "READY".to_string()
        },
        author: detail.author,
        created_at: detail.created_at,
        updated_at: detail.updated_at,
        url: detail.url,
        base_ref_name: detail.base_ref_name,
        head_ref_name: detail.head_ref_name,
        merge_state_status: detail.merge_state_status,
        mergeable: detail.mergeable,
        review_decision: detail.review_decision.unwrap_or_else(|| "NONE".to_string()),
        commit_count: detail.commit_count,
        file_count: detail.file_count,
    }
}

fn reviewer_decision_views(values: Vec<ReviewerDecision>) -> Vec<ReviewDecisionView> {
    if values.is_empty() {
        return vec![ReviewDecisionView {
            reviewer: "none".to_string(),
            state: "NONE".to_string(),
            submitted_at: "n/a".to_string(),
            body: String::new(),
        }];
    }

    values
        .into_iter()
        .map(|value| ReviewDecisionView {
            reviewer: value.reviewer,
            state: value.state,
            submitted_at: value.submitted_at,
            body: value.body,
        })
        .collect()
}

fn checks_view(summary: StatusChecksSummary) -> ChecksSummaryView {
    let headline = if summary.total == 0 {
        "No status checks".to_string()
    } else if summary.failed > 0 {
        format!("{} failing checks", summary.failed)
    } else if summary.pending > 0 {
        format!("{} checks pending", summary.pending)
    } else {
        "All checks passing".to_string()
    };

    ChecksSummaryView {
        total: summary.total,
        successful: summary.successful,
        failed: summary.failed,
        pending: summary.pending,
        neutral: summary.neutral,
        headline,
    }
}

fn issue_comment_views(values: Vec<IssueComment>) -> Vec<IssueCommentView> {
    values
        .into_iter()
        .map(|value| IssueCommentView {
            author: value.author,
            body: value.body,
            created_at: value.created_at,
            updated_at: value.updated_at,
            url: value.url,
        })
        .collect()
}

fn review_views(values: Vec<PullRequestReview>) -> Vec<PullRequestReviewView> {
    values
        .into_iter()
        .map(|value| PullRequestReviewView {
            author: value.author,
            state: value.state,
            body: value.body,
            submitted_at: value.submitted_at,
            url: value.url,
        })
        .collect()
}

fn review_comment_views(values: Vec<PullRequestReviewComment>) -> Vec<ReviewCommentView> {
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
                body: value.body,
                path: value.path,
                line_label,
                created_at: value.created_at,
                updated_at: value.updated_at,
                url: value.url,
            }
        })
        .collect()
}

fn state_label(state: String, is_draft: bool) -> String {
    if is_draft {
        format!("{} · DRAFT", state)
    } else {
        state
    }
}

fn clamp_flash(message: String) -> String {
    let max = cmp::min(message.len(), 240);
    message.chars().take(max).collect()
}

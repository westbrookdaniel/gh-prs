use crate::gh::models::{PreflightDiagnostics, RepoContext, ReviewerDecision, StatusChecksSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashMessageView {
    pub kind: String,
    pub message: String,
}

impl FlashMessageView {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            kind: "success".to_string(),
            message: super::helpers::clamp_flash(message.into()),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            kind: "error".to_string(),
            message: super::helpers::clamp_flash(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrListRowView {
    pub repo_name_with_owner: String,
    pub number: u64,
    pub detail_path: String,
    pub title: String,
    pub state_label: String,
    pub author: String,
    pub updated_at: String,
    pub comment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterFormView {
    pub org: String,
    pub repo: String,
    pub status: String,
    pub title: String,
    pub author: String,
    pub sort: String,
    pub order: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListTabView {
    pub label: String,
    pub href: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailTabView {
    pub label: String,
    pub href: String,
    pub selected: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLineView {
    pub kind_class: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFileView {
    pub id: String,
    pub filename: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub changes: usize,
    pub blob_url: String,
    pub previous_filename: Option<String>,
    pub is_collapsed: bool,
    pub has_patch: bool,
    pub lines: Vec<DiffLineView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffTreeItemView {
    pub id: String,
    pub filename: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrListPageModel {
    pub page_title: String,
    pub source_label: String,
    pub gh_version: String,
    pub authenticated_hosts: String,
    pub row_count: usize,
    pub rows: Vec<PrListRowView>,
    pub filters: FilterFormView,
    pub has_results_limit_warning: bool,
    pub flash: Option<FlashMessageView>,
    pub tabs: Vec<ListTabView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrDetailPageModel {
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
    pub back_to_list_href: String,
    pub tabs: Vec<DetailTabView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrChangesPageModel {
    pub page_title: String,
    pub repo_name: String,
    pub repo_url: String,
    pub header: DetailHeaderView,
    pub files: Vec<DiffFileView>,
    pub tree_items: Vec<DiffTreeItemView>,
    pub flash: Option<FlashMessageView>,
    pub back_to_list_href: String,
    pub tabs: Vec<DetailTabView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorPageModel {
    pub page_title: String,
    pub heading: String,
    pub status_code: u16,
    pub message: String,
    pub remediation: String,
    pub details: Option<String>,
}

pub fn source_label(repo: Option<&RepoContext>) -> String {
    repo.map(|ctx| {
        format!(
            "Scoped to your account; startup repo: {}",
            ctx.name_with_owner
        )
    })
    .unwrap_or_else(|| "Scoped to your account".to_string())
}

pub fn diagnostics_label(diagnostics: Option<&PreflightDiagnostics>) -> (String, String) {
    diagnostics
        .map(|value| {
            (
                value.gh_version.clone(),
                value.authenticated_hosts.join(", "),
            )
        })
        .unwrap_or_else(|| ("unknown".to_string(), "none".to_string()))
}

pub fn checks_view(summary: StatusChecksSummary) -> ChecksSummaryView {
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

pub fn reviewer_decisions_or_none(values: Vec<ReviewerDecision>) -> Vec<ReviewDecisionView> {
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

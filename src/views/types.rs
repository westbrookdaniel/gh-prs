use crate::gh::models::{PreflightDiagnostics, RepoContext, StatusCheckJob, StatusChecksSummary};

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
    pub state_tone: String,
    pub state_tooltip: String,
    pub author: String,
    pub author_avatar_url: String,
    pub author_avatar_style: String,
    pub author_initial: String,
    pub updated_at: String,
    pub comment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoOptionView {
    pub value: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterFormView {
    pub repos: Vec<String>,
    pub status: String,
    pub title: String,
    pub author: String,
    pub sort: String,
    pub order: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortControlView {
    pub label: String,
    pub href: String,
    pub selected: bool,
    pub direction: String,
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
pub struct ReviewerStatusView {
    pub reviewer: String,
    pub state: String,
    pub tone: String,
    pub state_tooltip: String,
    pub submitted_at: String,
    pub body_html: String,
    pub is_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueCommentView {
    pub author: String,
    pub body_html: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReviewView {
    pub author: String,
    pub state: String,
    pub tone: String,
    pub body_html: String,
    pub submitted_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCommentView {
    pub author: String,
    pub body_html: String,
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
    pub successful_pct: usize,
    pub failed_pct: usize,
    pub pending_pct: usize,
    pub neutral_pct: usize,
    pub headline: String,
    pub jobs: Vec<CheckJobView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckJobView {
    pub name: String,
    pub state: String,
    pub tone: String,
    pub icon_src: String,
    pub tooltip: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailHeaderView {
    pub number: u64,
    pub title: String,
    pub state_label: String,
    pub state_tone: String,
    pub state_tooltip: String,
    pub is_draft: bool,
    pub draft_label: String,
    pub draft_tooltip: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub merge_state_status: String,
    pub merge_state_tone: String,
    pub merge_state_tooltip: String,
    pub merge_state_explainer: Option<String>,
    pub mergeable: String,
    pub review_decision: String,
    pub review_decision_tone: String,
    pub review_decision_tooltip: String,
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
    pub repo_options: Vec<RepoOptionView>,
    pub filters: FilterFormView,
    pub sort_controls: Vec<SortControlView>,
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
    pub reviewer_statuses: Vec<ReviewerStatusView>,
    pub checks: ChecksSummaryView,
    pub body_html: String,
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
    let percentages = |count: usize| {
        if summary.total == 0 {
            0
        } else {
            ((count * 100) / summary.total).min(100)
        }
    };

    let successful_pct = percentages(summary.successful);
    let failed_pct = percentages(summary.failed);
    let pending_pct = percentages(summary.pending);
    let neutral_pct = percentages(summary.neutral);

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
        successful_pct,
        failed_pct,
        pending_pct,
        neutral_pct,
        headline,
        jobs: check_jobs_view(summary.jobs),
    }
}

fn check_jobs_view(jobs: Vec<StatusCheckJob>) -> Vec<CheckJobView> {
    jobs.into_iter()
        .map(|job| {
            let (tone, icon_href) = match job.state.as_str() {
                "SUCCESS" => (
                    "state-approved".to_string(),
                    "/assets/icons/check-circle.svg".to_string(),
                ),
                "FAILED" => (
                    "state-conflict".to_string(),
                    "/assets/icons/x-circle.svg".to_string(),
                ),
                "PENDING" => (
                    "state-open".to_string(),
                    "/assets/icons/clock.svg".to_string(),
                ),
                _ => (
                    "state-neutral".to_string(),
                    "/assets/icons/minus-circle.svg".to_string(),
                ),
            };

            CheckJobView {
                name: job.name,
                state: job.state,
                tone: tone.clone(),
                icon_src: icon_href,
                tooltip: match tone.as_str() {
                    "state-approved" => "Check passed".to_string(),
                    "state-conflict" => "Check failed".to_string(),
                    "state-open" => "Check pending".to_string(),
                    _ => "Check neutral".to_string(),
                },
            }
        })
        .collect()
}

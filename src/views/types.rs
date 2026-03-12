use crate::gh::models::{PullRequestDetail, PullRequestFile, StatusCheckJob, StatusChecksSummary};
use crate::views::helpers::format_timestamp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoOptionView {
    pub name: String,
    pub selected: bool,
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
pub struct ListResultRowView {
    pub repository_name_with_owner: String,
    pub state_label: String,
    pub detail_path: String,
    pub number: u64,
    pub title: String,
    pub author: String,
    pub comment_count: usize,
    pub updated_at: String,
    pub updated_at_display: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerStatusView {
    pub reviewer: String,
    pub state: String,
    pub tone: String,
    pub state_tooltip: String,
    pub submitted_at: String,
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
pub struct ConversationFeedItemView {
    pub author: String,
    pub kind_label: String,
    pub context_label: String,
    pub body_html: String,
    pub timestamp: String,
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
    pub headline_tone: String,
    pub headline_icon_src: String,
    pub headline_tooltip: String,
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
pub struct PrHeaderViewModel {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub repo_name_with_owner: String,
    pub repo_url: String,
    pub state_label: String,
    pub review_decision: String,
    pub merge_state_status: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub mergeable: String,
    pub is_draft: bool,
    pub can_close: bool,
    pub can_reopen: bool,
    pub can_mark_ready: bool,
    pub merge_state_explainer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeButtonView {
    pub label: String,
    pub reason: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrListPageModel {
    pub page_title: String,
    pub refresh_path: String,
    pub needs_refresh: bool,
    pub repo_options: Option<Vec<RepoOptionView>>,
    pub results: Option<Vec<ListResultRowView>>,
    pub sort_controls: Vec<SortControlView>,
    pub tabs: Vec<ListTabView>,
    pub title_value: String,
    pub author_value: String,
    pub status_value: String,
    pub sort_value: String,
    pub order_value: String,
    pub row_count: usize,
    pub has_results_limit_warning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrDetailPageModel {
    pub page_title: String,
    pub refresh_path: String,
    pub needs_refresh: bool,
    pub is_loading: bool,
    pub header: Option<PrHeaderViewModel>,
    pub reviewer_statuses: Vec<ReviewerStatusView>,
    pub reviewer_options: Vec<String>,
    pub checks: ChecksSummaryView,
    pub body_html: String,
    pub conversation_feed: Vec<ConversationFeedItemView>,
    pub comment_post_path: String,
    pub review_post_path: String,
    pub reviewers_post_path: String,
    pub merge_post_path: String,
    pub state_post_path: String,
    pub back_to_list_href: String,
    pub tabs: Vec<DetailTabView>,
    pub merge_button: Option<MergeButtonView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrChangesPageModel {
    pub page_title: String,
    pub refresh_path: String,
    pub needs_refresh: bool,
    pub files_loading: bool,
    pub header: Option<PrHeaderViewModel>,
    pub rendered_files: Vec<DiffFileView>,
    pub tree_items: Vec<DiffTreeItemView>,
    pub back_to_list_href: String,
    pub state_post_path: String,
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

    let (headline_tone, headline_icon_src, headline_tooltip) = if summary.failed > 0 {
        (
            "state-warning".to_string(),
            "/assets/icons/x-circle.svg".to_string(),
            "Checks are failing".to_string(),
        )
    } else if summary.pending > 0 {
        (
            "state-open".to_string(),
            "/assets/icons/clock.svg".to_string(),
            "Checks are still running".to_string(),
        )
    } else if summary.total == 0 {
        (
            "state-neutral".to_string(),
            "/assets/icons/minus-circle.svg".to_string(),
            "No checks reported".to_string(),
        )
    } else {
        (
            "state-approved".to_string(),
            "/assets/icons/check-circle.svg".to_string(),
            "All checks are passing".to_string(),
        )
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
        headline_tone,
        headline_icon_src,
        headline_tooltip,
        jobs: check_jobs_view(summary.jobs),
    }
}

pub fn pr_header_view(
    repo: &crate::gh::models::RepoContext,
    detail: &PullRequestDetail,
) -> PrHeaderViewModel {
    PrHeaderViewModel {
        number: detail.number,
        title: detail.title.clone(),
        url: detail.url.clone(),
        repo_name_with_owner: repo.name_with_owner.clone(),
        repo_url: repo.url.clone(),
        state_label: state_label(detail.state.clone(), detail.is_draft),
        review_decision: detail
            .review_decision
            .clone()
            .unwrap_or_else(|| "NONE".to_string()),
        merge_state_status: detail.merge_state_status.clone(),
        author: detail.author.clone(),
        created_at: format_timestamp(&detail.created_at),
        updated_at: format_timestamp(&detail.updated_at),
        base_ref_name: detail.base_ref_name.clone(),
        head_ref_name: detail.head_ref_name.clone(),
        mergeable: detail.mergeable.clone(),
        is_draft: detail.is_draft,
        can_close: detail.state.eq_ignore_ascii_case("OPEN"),
        can_reopen: detail.state.eq_ignore_ascii_case("CLOSED"),
        can_mark_ready: detail.is_draft,
        merge_state_explainer: merge_state_explainer(&detail.merge_state_status),
    }
}

pub fn merge_button_view(detail: Option<&PullRequestDetail>) -> Option<MergeButtonView> {
    let detail = detail?;
    let has_failing_checks = detail.checks.failed > 0;
    let has_pending_checks = detail.checks.pending > 0;
    let mergeable_clean = detail.mergeable.eq_ignore_ascii_case("MERGEABLE");
    let is_reopenable = detail.state.eq_ignore_ascii_case("CLOSED");

    let (label, reason, disabled) = if is_reopenable {
        (
            "Cannot Merge".to_string(),
            "PR is not open".to_string(),
            true,
        )
    } else if !mergeable_clean {
        (
            "Blocked".to_string(),
            "Merge conflicts or branch issues detected".to_string(),
            true,
        )
    } else if has_failing_checks {
        (
            "Merge Risk".to_string(),
            "One or more checks are failing".to_string(),
            false,
        )
    } else if has_pending_checks {
        (
            "Merge Pending".to_string(),
            "Checks are still running".to_string(),
            false,
        )
    } else {
        ("Merge PR".to_string(), "Ready to merge".to_string(), false)
    };

    Some(MergeButtonView {
        label,
        reason,
        disabled,
    })
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

fn check_jobs_view(jobs: Vec<StatusCheckJob>) -> Vec<CheckJobView> {
    jobs.into_iter()
        .map(|job| {
            let (tone, icon_href) = match job.state.as_str() {
                "SUCCESS" => (
                    "state-approved".to_string(),
                    "/assets/icons/check-circle.svg".to_string(),
                ),
                "FAILED" => (
                    "state-warning".to_string(),
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
                    "state-warning" => "Check failed".to_string(),
                    "state-open" => "Check pending".to_string(),
                    _ => "Check neutral".to_string(),
                },
            }
        })
        .collect()
}

pub fn diff_files_view(files: Vec<PullRequestFile>) -> (Vec<DiffTreeItemView>, Vec<DiffFileView>) {
    super::helpers::diff_files_view(files)
}

fn state_label(state: String, is_draft: bool) -> String {
    if is_draft {
        format!("{} · DRAFT", state)
    } else {
        state
    }
}

fn merge_state_explainer(merge_state_status: &str) -> Option<String> {
    let status = merge_state_status.trim().to_ascii_uppercase();
    if status == "BEHIND" {
        return Some(
            "Behind means this branch is behind the base branch and may require an update before merge."
                .to_string(),
        );
    }

    None
}

use crate::gh::models::{
    PullRequestConversation, PullRequestDetail, PullRequestFile, PullRequestSearchItem,
    StatusCheckJob, StatusChecksSummary,
};
use crate::search::SearchArgs;

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
pub struct Loadable<T> {
    pub value: Option<T>,
    pub is_stale: bool,
}

impl<T> Loadable<T> {
    pub fn missing() -> Self {
        Self {
            value: None,
            is_stale: false,
        }
    }

    pub fn ready(value: T, is_stale: bool) -> Self {
        Self {
            value: Some(value),
            is_stale,
        }
    }

    pub fn needs_refresh(&self) -> bool {
        self.is_stale || self.value.is_none()
    }
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
pub struct PrListPageModel {
    pub page_title: String,
    pub refresh_path: String,
    pub needs_refresh: bool,
    pub query: SearchArgs,
    pub repo_options: Loadable<Vec<String>>,
    pub results: Loadable<Vec<PullRequestSearchItem>>,
    pub sort_controls: Vec<SortControlView>,
    pub flash: Option<FlashMessageView>,
    pub tabs: Vec<ListTabView>,
}

impl PrListPageModel {
    pub fn row_count(&self) -> usize {
        self.results.value.as_ref().map_or(0, Vec::len)
    }

    pub fn has_results_limit_warning(&self) -> bool {
        self.query.limit >= crate::gh::models::DEFAULT_SEARCH_LIMIT
            && self.row_count() >= crate::gh::models::DEFAULT_SEARCH_LIMIT
    }

    pub fn is_repo_selected(&self, repo: &str) -> bool {
        self.query.repos.iter().any(|value| value == repo)
    }

    pub fn status_value(&self) -> &str {
        self.query.status.as_query_value()
    }

    pub fn sort_value(&self) -> &str {
        self.query.sort.as_query_value()
    }

    pub fn order_value(&self) -> &str {
        self.query.order.as_query_value()
    }

    pub fn title_value(&self) -> &str {
        self.query.title.as_deref().unwrap_or_default()
    }

    pub fn author_value(&self) -> &str {
        self.query.author.as_deref().unwrap_or_default()
    }

    pub fn detail_path(&self, item: &PullRequestSearchItem) -> String {
        super::helpers::detail_path_from_repo(
            &item.repository_name_with_owner,
            item.number,
            self.query.to_query_string().as_deref(),
        )
    }

    pub fn state_label(&self, item: &PullRequestSearchItem) -> String {
        super::helpers::state_label(item.state.clone(), item.is_draft)
    }

    pub fn formatted_updated_at(&self, item: &PullRequestSearchItem) -> String {
        super::helpers::format_timestamp(&item.updated_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrDetailPageModel {
    pub page_title: String,
    pub refresh_path: String,
    pub needs_refresh: bool,
    pub repo: crate::gh::models::RepoContext,
    pub conversation: Loadable<PullRequestConversation>,
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
    pub flash: Option<FlashMessageView>,
    pub back_to_list_href: String,
    pub tabs: Vec<DetailTabView>,
}

impl PrDetailPageModel {
    pub fn merge_button_label(&self, header: &PrHeaderViewModel) -> String {
        let (_, label, _, _) = self.merge_button(header);
        label
    }

    pub fn merge_button_reason(&self, header: &PrHeaderViewModel) -> String {
        let (_, _, reason, _) = self.merge_button(header);
        reason
    }

    pub fn merge_button_disabled(&self, header: &PrHeaderViewModel) -> bool {
        let (_, _, _, disabled) = self.merge_button(header);
        disabled
    }

    fn merge_button(&self, header: &PrHeaderViewModel) -> (String, String, String, bool) {
        let conversation = match self.conversation.value.as_ref() {
            Some(conversation) => conversation,
            None => {
                return (
                    "secondary".to_string(),
                    "Loading".to_string(),
                    "Pull request details are still loading".to_string(),
                    true,
                )
            }
        };

        let has_failing_checks = conversation.detail.checks.failed > 0;
        let has_pending_checks = conversation.detail.checks.pending > 0;
        let mergeable_clean = header.mergeable.eq_ignore_ascii_case("MERGEABLE");

        if header.can_reopen {
            (
                "secondary".to_string(),
                "Cannot Merge".to_string(),
                "PR is not open".to_string(),
                true,
            )
        } else if !mergeable_clean {
            (
                "danger".to_string(),
                "Blocked".to_string(),
                "Merge conflicts or branch issues detected".to_string(),
                true,
            )
        } else if has_failing_checks {
            (
                "warning".to_string(),
                "Merge Risk".to_string(),
                "One or more checks are failing".to_string(),
                false,
            )
        } else if has_pending_checks {
            (
                "warning".to_string(),
                "Merge Pending".to_string(),
                "Checks are still running".to_string(),
                false,
            )
        } else {
            (
                "approve".to_string(),
                "Merge PR".to_string(),
                "Ready to merge".to_string(),
                false,
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrChangesPageModel {
    pub page_title: String,
    pub refresh_path: String,
    pub needs_refresh: bool,
    pub repo: crate::gh::models::RepoContext,
    pub detail: Loadable<PullRequestDetail>,
    pub header: Option<PrHeaderViewModel>,
    pub files: Loadable<Vec<PullRequestFile>>,
    pub rendered_files: Vec<DiffFileView>,
    pub tree_items: Vec<DiffTreeItemView>,
    pub flash: Option<FlashMessageView>,
    pub back_to_list_href: String,
    pub state_post_path: String,
    pub tabs: Vec<DetailTabView>,
}

impl PrChangesPageModel {}

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
        state_label: super::helpers::state_label(detail.state.clone(), detail.is_draft),
        review_decision: detail
            .review_decision
            .clone()
            .unwrap_or_else(|| "NONE".to_string()),
        merge_state_status: detail.merge_state_status.clone(),
        author: detail.author.clone(),
        created_at: super::helpers::format_timestamp(&detail.created_at),
        updated_at: super::helpers::format_timestamp(&detail.updated_at),
        base_ref_name: detail.base_ref_name.clone(),
        head_ref_name: detail.head_ref_name.clone(),
        mergeable: detail.mergeable.clone(),
        is_draft: detail.is_draft,
        can_close: detail.state.eq_ignore_ascii_case("OPEN"),
        can_reopen: detail.state.eq_ignore_ascii_case("CLOSED"),
        can_mark_ready: detail.is_draft,
        merge_state_explainer: super::helpers::merge_state_explainer(&detail.merge_state_status),
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

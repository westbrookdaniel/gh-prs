use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

pub const DEFAULT_SEARCH_LIMIT: usize = 100;
pub const MAX_SEARCH_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoContext {
    pub name_with_owner: String,
    pub url: String,
    pub viewer_permission: String,
    pub default_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightDiagnostics {
    pub gh_version: String,
    pub authenticated_hosts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestListItem {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub review_decision: Option<String>,
    pub requested_reviewers: Vec<String>,
    pub comment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestSearchItem {
    pub repository_name_with_owner: String,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub is_draft: bool,
    pub author: String,
    pub author_avatar_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub comment_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestStatus {
    All,
    Open,
    Closed,
    Merged,
}

impl PullRequestStatus {
    pub fn as_query_value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Merged => "merged",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "all" => Some(Self::All),
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            "merged" => Some(Self::Merged),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestSort {
    Updated,
    Created,
    Comments,
}

impl PullRequestSort {
    pub fn as_query_value(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::Created => "created",
            Self::Comments => "comments",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "updated" => Some(Self::Updated),
            "created" => Some(Self::Created),
            "comments" => Some(Self::Comments),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestOrder {
    Asc,
    Desc,
}

impl PullRequestOrder {
    pub fn as_query_value(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "asc" => Some(Self::Asc),
            "desc" => Some(Self::Desc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerDecision {
    pub reviewer: String,
    pub state: String,
    pub submitted_at: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusCheckJob {
    pub name: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusChecksSummary {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub pending: usize,
    pub neutral: usize,
    pub jobs: Vec<StatusCheckJob>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestDetail {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub is_draft: bool,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub merge_state_status: String,
    pub mergeable: String,
    pub review_decision: Option<String>,
    pub requested_reviewers: Vec<String>,
    pub latest_reviewer_decisions: Vec<ReviewerDecision>,
    pub checks: StatusChecksSummary,
    pub commit_count: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueComment {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReview {
    pub id: u64,
    pub author: String,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestReviewComment {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub path: String,
    pub line: Option<u64>,
    pub original_line: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestFile {
    pub filename: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub changes: usize,
    pub previous_filename: Option<String>,
    pub patch: Option<String>,
    pub blob_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestConversation {
    pub detail: PullRequestDetail,
    pub issue_comments: Vec<IssueComment>,
    pub reviews: Vec<PullRequestReview>,
    pub review_comments: Vec<PullRequestReviewComment>,
}

#[derive(Debug, Deserialize)]
struct RepoContextRaw {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
    url: String,
    #[serde(rename = "viewerPermission")]
    viewer_permission: Option<String>,
    #[serde(rename = "defaultBranchRef")]
    default_branch_ref: Option<RepoBranchRaw>,
}

#[derive(Debug, Deserialize)]
struct RepoBranchRaw {
    name: String,
}

#[derive(Debug, Deserialize)]
struct AuthStatusRaw {
    #[serde(default)]
    hosts: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct UserRaw {
    login: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PullRequestListItemRaw {
    number: u64,
    title: String,
    state: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    author: Option<UserRaw>,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    url: String,
    #[serde(rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(rename = "reviewRequests", default)]
    review_requests: Option<Value>,
    #[serde(default)]
    comments: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct PullRequestDetailRaw {
    number: u64,
    title: String,
    #[serde(default)]
    body: String,
    state: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    author: Option<UserRaw>,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "updatedAt")]
    updated_at: String,
    url: String,
    #[serde(rename = "baseRefName")]
    base_ref_name: String,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(rename = "mergeStateStatus")]
    merge_state_status: Option<String>,
    mergeable: Option<String>,
    #[serde(rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(rename = "reviewRequests", default)]
    review_requests: Option<Value>,
    #[serde(rename = "latestReviews", default)]
    latest_reviews: Vec<LatestReviewRaw>,
    #[serde(rename = "statusCheckRollup", default)]
    status_check_rollup: Option<Value>,
    #[serde(default)]
    commits: Option<Value>,
    #[serde(default)]
    files: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct LatestReviewRaw {
    #[serde(default)]
    state: String,
    author: Option<UserRaw>,
    #[serde(rename = "submittedAt", default)]
    submitted_at: String,
    #[serde(default)]
    body: String,
}

#[derive(Debug, Deserialize)]
struct IssueCommentRaw {
    id: u64,
    user: Option<UserRaw>,
    #[serde(default)]
    body: String,
    #[serde(rename = "created_at", default)]
    created_at: String,
    #[serde(rename = "updated_at", default)]
    updated_at: String,
    #[serde(rename = "html_url", default)]
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct PullRequestReviewRaw {
    id: u64,
    user: Option<UserRaw>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: String,
    #[serde(rename = "submitted_at", default)]
    submitted_at: String,
    #[serde(rename = "html_url", default)]
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct PullRequestReviewCommentRaw {
    id: u64,
    user: Option<UserRaw>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    path: String,
    line: Option<u64>,
    #[serde(rename = "original_line")]
    original_line: Option<u64>,
    #[serde(rename = "created_at", default)]
    created_at: String,
    #[serde(rename = "updated_at", default)]
    updated_at: String,
    #[serde(rename = "html_url", default)]
    html_url: String,
}

#[derive(Debug, Deserialize)]
struct PullRequestFileRaw {
    #[serde(default)]
    filename: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    additions: usize,
    #[serde(default)]
    deletions: usize,
    #[serde(default)]
    changes: usize,
    #[serde(rename = "previous_filename")]
    previous_filename: Option<String>,
    patch: Option<String>,
    #[serde(rename = "blob_url", default)]
    blob_url: String,
}

pub fn parse_repo_context(json: &str) -> Result<RepoContext, String> {
    let raw: RepoContextRaw = serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(RepoContext {
        name_with_owner: raw.name_with_owner,
        url: raw.url,
        viewer_permission: raw
            .viewer_permission
            .unwrap_or_else(|| "UNKNOWN".to_string()),
        default_branch: raw
            .default_branch_ref
            .map(|branch| branch.name)
            .unwrap_or_else(|| "main".to_string()),
    })
}

pub fn parse_preflight_auth(json: &str) -> Result<Vec<String>, String> {
    let raw: AuthStatusRaw = serde_json::from_str(json).map_err(|err| err.to_string())?;
    let mut hosts: Vec<String> = raw
        .hosts
        .iter()
        .filter_map(|(host, value)| host_is_authenticated(value).then_some(host.clone()))
        .collect();
    hosts.sort();
    Ok(hosts)
}

fn host_is_authenticated(value: &Value) -> bool {
    if value.is_null() {
        return false;
    }

    if let Some(array) = value.as_array() {
        return array.iter().any(host_is_authenticated);
    }

    if value
        .get("active")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }

    let state = value
        .get("state")
        .or_else(|| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if state.contains("logged out") {
        return false;
    }

    state.contains("success") || state.contains("logged in")
}

pub fn parse_pull_request_list(json: &str) -> Result<Vec<PullRequestListItem>, String> {
    let raw_items: Vec<PullRequestListItemRaw> =
        serde_json::from_str(json).map_err(|err| err.to_string())?;
    let mut items = Vec::with_capacity(raw_items.len());
    for raw in raw_items {
        items.push(PullRequestListItem {
            number: raw.number,
            title: raw.title,
            state: raw.state,
            is_draft: raw.is_draft,
            author: extract_user(raw.author),
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            url: raw.url,
            review_decision: raw.review_decision,
            requested_reviewers: extract_requested_reviewers(raw.review_requests),
            comment_count: extract_count_field(raw.comments),
        });
    }
    Ok(items)
}

pub fn parse_pull_request_detail(json: &str) -> Result<PullRequestDetail, String> {
    let raw: PullRequestDetailRaw = serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(PullRequestDetail {
        number: raw.number,
        title: raw.title,
        body: raw.body,
        state: raw.state,
        is_draft: raw.is_draft,
        author: extract_user(raw.author),
        created_at: raw.created_at,
        updated_at: raw.updated_at,
        url: raw.url,
        base_ref_name: raw.base_ref_name,
        head_ref_name: raw.head_ref_name,
        merge_state_status: raw
            .merge_state_status
            .unwrap_or_else(|| "UNKNOWN".to_string()),
        mergeable: raw.mergeable.unwrap_or_else(|| "UNKNOWN".to_string()),
        review_decision: raw.review_decision,
        requested_reviewers: extract_requested_reviewers(raw.review_requests),
        latest_reviewer_decisions: extract_latest_reviewer_decisions(raw.latest_reviews),
        checks: summarize_status_checks(raw.status_check_rollup),
        commit_count: extract_count_field(raw.commits),
        file_count: extract_count_field(raw.files),
    })
}

pub fn parse_issue_comments(json: &str) -> Result<Vec<IssueComment>, String> {
    let raw_items: Vec<IssueCommentRaw> =
        serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(raw_items
        .into_iter()
        .map(|raw| IssueComment {
            id: raw.id,
            author: extract_user(raw.user),
            body: raw.body,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            url: raw.html_url,
        })
        .collect())
}

pub fn parse_pull_request_reviews(json: &str) -> Result<Vec<PullRequestReview>, String> {
    let raw_items: Vec<PullRequestReviewRaw> =
        serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(raw_items
        .into_iter()
        .map(|raw| PullRequestReview {
            id: raw.id,
            author: extract_user(raw.user),
            state: normalize_review_state(&raw.state),
            body: raw.body,
            submitted_at: raw.submitted_at,
            url: raw.html_url,
        })
        .collect())
}

pub fn parse_pull_request_review_comments(
    json: &str,
) -> Result<Vec<PullRequestReviewComment>, String> {
    let raw_items: Vec<PullRequestReviewCommentRaw> =
        serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(raw_items
        .into_iter()
        .map(|raw| PullRequestReviewComment {
            id: raw.id,
            author: extract_user(raw.user),
            body: raw.body,
            path: raw.path,
            line: raw.line,
            original_line: raw.original_line,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
            url: raw.html_url,
        })
        .collect())
}

pub fn parse_pull_request_files(json: &str) -> Result<Vec<PullRequestFile>, String> {
    let raw_items: Vec<PullRequestFileRaw> =
        serde_json::from_str(json).map_err(|err| err.to_string())?;
    Ok(raw_items
        .into_iter()
        .map(|raw| PullRequestFile {
            filename: raw.filename,
            status: raw.status.to_ascii_uppercase(),
            additions: raw.additions,
            deletions: raw.deletions,
            changes: raw.changes,
            previous_filename: raw.previous_filename,
            patch: raw.patch,
            blob_url: raw.blob_url,
        })
        .collect())
}

fn extract_user(user: Option<UserRaw>) -> String {
    user.and_then(|candidate| candidate.login.or(candidate.name))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn extract_requested_reviewers(value: Option<Value>) -> Vec<String> {
    let mut reviewers = Vec::new();
    let Some(value) = value else {
        return reviewers;
    };

    for item in collect_nodes(&value) {
        if let Some(requested_reviewer) = item.get("requestedReviewer") {
            if let Some(name) = reviewer_name(requested_reviewer) {
                reviewers.push(name);
                continue;
            }
        }

        if let Some(name) = reviewer_name(item) {
            reviewers.push(name);
        }
    }

    reviewers.sort();
    reviewers.dedup();
    reviewers
}

fn reviewer_name(value: &Value) -> Option<String> {
    value
        .get("login")
        .and_then(Value::as_str)
        .or_else(|| value.get("name").and_then(Value::as_str))
        .or_else(|| value.get("slug").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn extract_count_field(value: Option<Value>) -> usize {
    let Some(value) = value else {
        return 0;
    };

    if let Some(number) = value.as_u64() {
        return number as usize;
    }

    if let Some(total_count) = value.get("totalCount").and_then(Value::as_u64) {
        return total_count as usize;
    }

    if let Some(nodes) = value.get("nodes").and_then(Value::as_array) {
        return nodes.len();
    }

    if let Some(values) = value.as_array() {
        return values.len();
    }

    0
}

fn extract_latest_reviewer_decisions(raw: Vec<LatestReviewRaw>) -> Vec<ReviewerDecision> {
    let mut latest_by_reviewer: BTreeMap<String, ReviewerDecision> = BTreeMap::new();

    for entry in raw {
        let reviewer = extract_user(entry.author);
        let candidate = ReviewerDecision {
            reviewer: reviewer.clone(),
            state: normalize_review_state(&entry.state),
            submitted_at: entry.submitted_at,
            body: entry.body,
        };

        match latest_by_reviewer.get(&reviewer) {
            Some(existing) if existing.submitted_at > candidate.submitted_at => {}
            _ => {
                latest_by_reviewer.insert(reviewer, candidate);
            }
        }
    }

    latest_by_reviewer.into_values().collect()
}

fn normalize_review_state(value: &str) -> String {
    if value.trim().is_empty() {
        "UNKNOWN".to_string()
    } else {
        value.trim().to_ascii_uppercase()
    }
}

fn summarize_status_checks(value: Option<Value>) -> StatusChecksSummary {
    let mut summary = StatusChecksSummary::default();
    let Some(value) = value else {
        return summary;
    };

    for node in collect_nodes(&value) {
        let state = classify_status_check(node);
        let fallback_name = format!("Check {}", summary.total + 1);
        let name = extract_check_name(node).unwrap_or(fallback_name);

        summary.total += 1;

        summary.jobs.push(StatusCheckJob {
            name,
            state: state.to_string(),
        });

        match state {
            "SUCCESS" => summary.successful += 1,
            "FAILED" => summary.failed += 1,
            "PENDING" => summary.pending += 1,
            _ => summary.neutral += 1,
        }
    }

    summary
}

fn classify_status_check(node: &Value) -> &'static str {
    let status = node
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();
    let conclusion = node
        .get("conclusion")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();
    let state = node
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase();

    if !status.is_empty() && status != "COMPLETED" {
        return "PENDING";
    }

    if !state.is_empty() {
        return match state.as_str() {
            "SUCCESS" => "SUCCESS",
            "FAILURE" | "ERROR" => "FAILED",
            "PENDING" | "EXPECTED" => "PENDING",
            _ => "NEUTRAL",
        };
    }

    if !conclusion.is_empty() {
        return match conclusion.as_str() {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => "SUCCESS",
            "FAILURE" | "STARTUP_FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" => {
                "FAILED"
            }
            _ => "NEUTRAL",
        };
    }

    "NEUTRAL"
}

fn extract_check_name(node: &Value) -> Option<String> {
    let candidates = ["name", "context", "displayName", "title"];
    for key in candidates {
        let value = node
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if value.is_some() {
            return value;
        }
    }

    None
}

fn collect_nodes(value: &Value) -> Vec<&Value> {
    if let Some(values) = value.as_array() {
        return values.iter().collect();
    }

    if let Some(nodes) = value.get("nodes").and_then(Value::as_array) {
        return nodes.iter().collect();
    }

    if let Some(contexts) = value.get("contexts") {
        if let Some(values) = contexts.as_array() {
            return values.iter().collect();
        }
        if let Some(nodes) = contexts.get("nodes").and_then(Value::as_array) {
            return nodes.iter().collect();
        }
    }

    if value.is_object() {
        return vec![value];
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_issue_comments, parse_preflight_auth, parse_pull_request_detail,
        parse_pull_request_files, parse_pull_request_list, parse_pull_request_review_comments,
        parse_pull_request_reviews, parse_repo_context,
    };

    #[test]
    fn parses_repo_context_with_branch() {
        let json = r#"{
            "nameWithOwner":"acme/widgets",
            "url":"https://github.com/acme/widgets",
            "viewerPermission":"WRITE",
            "defaultBranchRef":{"name":"main"}
        }"#;

        let repo = parse_repo_context(json).expect("repo should parse");
        assert_eq!(repo.name_with_owner, "acme/widgets");
        assert_eq!(repo.default_branch, "main");
    }

    #[test]
    fn parses_authenticated_hosts() {
        let json = r#"{
            "hosts": {
                "github.com": {"status": "logged in"},
                "ghe.local": {"status": "logged out"}
            }
        }"#;

        let hosts = parse_preflight_auth(json).expect("auth should parse");
        assert_eq!(hosts, vec!["github.com".to_string()]);
    }

    #[test]
    fn parses_authenticated_hosts_from_cli_array_shape() {
        let json = r#"{
            "hosts": {
                "github.com": [
                    {
                        "state": "success",
                        "active": true,
                        "login": "octocat"
                    }
                ]
            }
        }"#;

        let hosts = parse_preflight_auth(json).expect("auth should parse");
        assert_eq!(hosts, vec!["github.com".to_string()]);
    }

    #[test]
    fn parses_pull_request_list_with_review_requests() {
        let json = r#"[
          {
            "number": 12,
            "title": "Improve parser",
            "state": "OPEN",
            "isDraft": false,
            "author": {"login": "alice"},
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-02T00:00:00Z",
            "url": "https://example/pr/12",
            "reviewDecision": "REVIEW_REQUIRED",
            "reviewRequests": [
              {"requestedReviewer": {"login": "bob"}},
              {"requestedReviewer": {"name": "backend-team"}}
            ],
            "comments": {"totalCount": 3}
          }
        ]"#;

        let items = parse_pull_request_list(json).expect("list should parse");
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.number, 12);
        assert_eq!(item.requested_reviewers, vec!["backend-team", "bob"]);
        assert_eq!(item.comment_count, 3);
    }

    #[test]
    fn parses_pull_request_detail_and_status_rollup() {
        let json = r#"{
          "number": 20,
          "title": "Ship MVP",
          "body": "details",
          "state": "OPEN",
          "isDraft": false,
          "author": {"login": "alice"},
          "createdAt": "2026-01-01T00:00:00Z",
          "updatedAt": "2026-01-02T00:00:00Z",
          "url": "https://example/pr/20",
          "baseRefName": "main",
          "headRefName": "feature",
          "mergeStateStatus": "CLEAN",
          "mergeable": "MERGEABLE",
          "reviewDecision": "REVIEW_REQUIRED",
          "reviewRequests": [{"requestedReviewer": {"login": "bob"}}],
          "latestReviews": [
            {
              "state": "APPROVED",
              "author": {"login": "bob"},
              "submittedAt": "2026-01-03T00:00:00Z",
              "body": "looks good"
            }
          ],
          "statusCheckRollup": {
            "contexts": {
              "nodes": [
                {"__typename":"CheckRun","name":"lint","status":"COMPLETED","conclusion":"SUCCESS"},
                {"__typename":"StatusContext","context":"tests","state":"PENDING"},
                {"__typename":"StatusContext","context":"build","state":"FAILURE"}
              ]
            }
          },
          "commits": {"totalCount": 5},
          "files": {"totalCount": 2}
        }"#;

        let detail = parse_pull_request_detail(json).expect("detail should parse");
        assert_eq!(detail.number, 20);
        assert_eq!(detail.requested_reviewers, vec!["bob"]);
        assert_eq!(detail.latest_reviewer_decisions.len(), 1);
        assert_eq!(detail.checks.total, 3);
        assert_eq!(detail.checks.successful, 1);
        assert_eq!(detail.checks.pending, 1);
        assert_eq!(detail.checks.failed, 1);
        assert_eq!(detail.checks.jobs.len(), 3);
        assert_eq!(detail.checks.jobs[0].name, "lint");
        assert_eq!(detail.checks.jobs[1].name, "tests");
        assert_eq!(detail.checks.jobs[2].name, "build");
        assert_eq!(detail.commit_count, 5);
        assert_eq!(detail.file_count, 2);
    }

    #[test]
    fn parses_rest_conversation_payloads() {
        let issue_comments = r#"[
          {
            "id": 1,
            "user": {"login": "alice"},
            "body": "issue comment",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "html_url": "https://example/comments/1"
          }
        ]"#;
        let reviews = r#"[
          {
            "id": 7,
            "user": {"login": "bob"},
            "state": "APPROVED",
            "body": "done",
            "submitted_at": "2026-01-02T00:00:00Z",
            "html_url": "https://example/reviews/7"
          }
        ]"#;
        let review_comments = r#"[
          {
            "id": 10,
            "user": {"login": "carol"},
            "body": "nit",
            "path": "src/main.rs",
            "line": 14,
            "original_line": 13,
            "created_at": "2026-01-03T00:00:00Z",
            "updated_at": "2026-01-03T00:00:00Z",
            "html_url": "https://example/review-comments/10"
          }
        ]"#;

        let parsed_issue_comments =
            parse_issue_comments(issue_comments).expect("issue comments should parse");
        let parsed_reviews = parse_pull_request_reviews(reviews).expect("reviews should parse");
        let parsed_review_comments = parse_pull_request_review_comments(review_comments)
            .expect("review comments should parse");

        assert_eq!(parsed_issue_comments.len(), 1);
        assert_eq!(parsed_reviews.len(), 1);
        assert_eq!(parsed_review_comments.len(), 1);
        assert_eq!(parsed_review_comments[0].path, "src/main.rs");
    }

    #[test]
    fn parses_pull_request_files_payload() {
        let json = r#"[
          {
            "filename": "src/main.rs",
            "status": "modified",
            "additions": 10,
            "deletions": 2,
            "changes": 12,
            "previous_filename": "src/lib.rs",
            "patch": "@@ -1,2 +1,3 @@",
            "blob_url": "https://example/blob"
          }
        ]"#;

        let files = parse_pull_request_files(json).expect("files should parse");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "src/main.rs");
        assert_eq!(files[0].status, "MODIFIED");
        assert_eq!(files[0].additions, 10);
        assert_eq!(files[0].deletions, 2);
        assert_eq!(files[0].patch.as_deref(), Some("@@ -1,2 +1,3 @@"));
    }
}

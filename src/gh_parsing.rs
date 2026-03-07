use crate::gh::models::{
    IssueComment, PullRequestDetail, PullRequestFile, PullRequestReview, PullRequestReviewComment,
    RepoContext, ReviewerDecision, StatusCheckJob, StatusChecksSummary,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

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
    let raw: RepoContextRaw = serde_json::from_str(json).map_err(|error| error.to_string())?;
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
    let raw: AuthStatusRaw = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let mut hosts: Vec<String> = raw
        .hosts
        .iter()
        .filter_map(|(host, value)| host_is_authenticated(value).then_some(host.clone()))
        .collect();
    hosts.sort();
    Ok(hosts)
}

pub fn parse_pull_request_detail(json: &str) -> Result<PullRequestDetail, String> {
    let raw: PullRequestDetailRaw =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
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
        serde_json::from_str(json).map_err(|error| error.to_string())?;
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
        serde_json::from_str(json).map_err(|error| error.to_string())?;
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
        serde_json::from_str(json).map_err(|error| error.to_string())?;
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
        serde_json::from_str(json).map_err(|error| error.to_string())?;
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
        if let Some(requested_reviewer) = item.get("requestedReviewer")
            && let Some(name) = reviewer_name(requested_reviewer)
        {
            reviewers.push(name);
            continue;
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
    for key in ["name", "context", "displayName", "title"] {
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
        parse_pull_request_files, parse_pull_request_review_comments, parse_pull_request_reviews,
        parse_repo_context,
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
    }
}

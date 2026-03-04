use crate::gh::models::{
    PullRequestFile, PullRequestOrder, PullRequestSearchQuery, PullRequestSort, PullRequestStatus,
    RepoContext,
};
use crate::http::Request;
use crate::views::types::{
    DetailTabView, DiffFileView, DiffLineView, DiffTreeItemView, ListTabView,
};
use std::cmp;

pub fn clamp_flash(message: String) -> String {
    let max = cmp::min(message.len(), 240);
    message.chars().take(max).collect()
}

pub fn normalize_text(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Some(value.chars().take(120).collect())
}

pub fn normalize_simple(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Some(value.to_string());
    }

    None
}

pub fn normalize_login(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('@');
    normalize_simple(value)
}

pub fn normalize_repo(value: &str, org: Option<&str>) -> Option<String> {
    let value = value.trim();
    if let Some((owner, name)) = value.split_once('/') {
        return normalize_repo_parts(owner, name);
    }

    let owner = org?;
    normalize_repo_parts(owner, value)
}

pub fn normalize_repo_parts(owner: &str, name: &str) -> Option<String> {
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }

    if !owner
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return None;
    }

    Some(format!("{owner}/{name}"))
}

pub fn state_label(state: String, is_draft: bool) -> String {
    if is_draft {
        format!("{} · DRAFT", state)
    } else {
        state
    }
}

pub fn detail_path_from_repo(repo: &str, number: u64, query: Option<&str>) -> String {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}")
    } else {
        format!("/prs/{number}")
    };

    with_query(base, query)
}

pub fn changes_path_from_repo(repo: &str, number: u64, query: Option<&str>) -> String {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}/changes")
    } else {
        format!("/prs/{number}/changes")
    };

    with_query(base, query)
}

pub fn repo_action_path(repo: &str, number: u64, action: &str, query: Option<&str>) -> String {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}/{action}")
    } else {
        format!("/prs/{number}/{action}")
    };

    with_query(base, query)
}

pub fn with_query(mut path: String, query: Option<&str>) -> String {
    if let Some(query) = query {
        if !query.is_empty() {
            path.push('?');
            path.push_str(query);
        }
    }
    path
}

pub fn query_from_request(request: &Request) -> Option<String> {
    let org = request.query_param("org").and_then(normalize_simple);
    let repo = request
        .query_param("repo")
        .and_then(|value| normalize_repo(value, org.as_deref()));
    let title = request.query_param("title").and_then(normalize_text);
    let author = request.query_param("author").and_then(normalize_login);

    let status = request
        .query_param("status")
        .and_then(PullRequestStatus::parse)
        .unwrap_or(PullRequestStatus::All);
    let sort = request
        .query_param("sort")
        .and_then(PullRequestSort::parse)
        .unwrap_or(PullRequestSort::Updated);
    let order = request
        .query_param("order")
        .and_then(PullRequestOrder::parse)
        .unwrap_or(PullRequestOrder::Desc);

    let view = request
        .query_param("view")
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "all")
        .map(str::to_string);

    let mut encoded: Vec<(String, String)> = Vec::new();
    if let Some(org) = org {
        encoded.push(("org".to_string(), org));
    }
    if let Some(repo) = repo {
        encoded.push(("repo".to_string(), repo));
    }
    if status != PullRequestStatus::All {
        encoded.push(("status".to_string(), status.as_query_value().to_string()));
    }
    if let Some(title) = title {
        encoded.push(("title".to_string(), title));
    }
    if let Some(author) = author {
        encoded.push(("author".to_string(), author));
    }
    if sort != PullRequestSort::Updated {
        encoded.push(("sort".to_string(), sort.as_query_value().to_string()));
    }
    if order != PullRequestOrder::Desc {
        encoded.push(("order".to_string(), order.as_query_value().to_string()));
    }
    if let Some(view) = view {
        encoded.push(("view".to_string(), view));
    }

    if encoded.is_empty() {
        return None;
    }

    serde_urlencoded::to_string(encoded).ok()
}

pub fn build_list_tabs(query: &PullRequestSearchQuery) -> Vec<ListTabView> {
    let open_query = query_for_status(query, "open");
    let merged_query = query_for_status(query, "merged");
    let closed_query = query_for_status(query, "closed");
    let all_query = query_for_status(query, "all");

    vec![
        ListTabView {
            label: "Open".to_string(),
            href: with_query(
                "/prs".to_string(),
                (!open_query.is_empty()).then_some(open_query.as_str()),
            ),
            selected: query.status.as_query_value() == "open",
        },
        ListTabView {
            label: "Merged".to_string(),
            href: with_query(
                "/prs".to_string(),
                (!merged_query.is_empty()).then_some(merged_query.as_str()),
            ),
            selected: query.status.as_query_value() == "merged",
        },
        ListTabView {
            label: "Closed".to_string(),
            href: with_query(
                "/prs".to_string(),
                (!closed_query.is_empty()).then_some(closed_query.as_str()),
            ),
            selected: query.status.as_query_value() == "closed",
        },
        ListTabView {
            label: "All".to_string(),
            href: with_query(
                "/prs".to_string(),
                (!all_query.is_empty()).then_some(all_query.as_str()),
            ),
            selected: query.status.as_query_value() == "all",
        },
    ]
}

fn query_for_status(query: &PullRequestSearchQuery, status: &str) -> String {
    let status = PullRequestStatus::parse(status).unwrap_or(PullRequestStatus::All);
    let mut encoded = Vec::new();

    if status != PullRequestStatus::All {
        encoded.push(("status".to_string(), status.as_query_value().to_string()));
    }

    if let Some(org) = &query.org {
        encoded.push(("org".to_string(), org.clone()));
    }

    if let Some(repo) = &query.repo {
        encoded.push(("repo".to_string(), repo.clone()));
    }

    if let Some(title) = &query.title {
        encoded.push(("title".to_string(), title.clone()));
    }

    if let Some(author) = &query.author {
        encoded.push(("author".to_string(), author.clone()));
    }

    if query.sort != PullRequestSort::Updated {
        encoded.push(("sort".to_string(), query.sort.as_query_value().to_string()));
    }

    if query.order != PullRequestOrder::Desc {
        encoded.push((
            "order".to_string(),
            query.order.as_query_value().to_string(),
        ));
    }

    serde_urlencoded::to_string(encoded).unwrap_or_default()
}

pub fn build_detail_tabs(
    repo: &RepoContext,
    number: u64,
    query: Option<&str>,
    is_changes: bool,
) -> Vec<DetailTabView> {
    vec![
        DetailTabView {
            label: "Conversation".to_string(),
            href: detail_path_from_repo(&repo.name_with_owner, number, query),
            selected: !is_changes,
        },
        DetailTabView {
            label: "Changes".to_string(),
            href: changes_path_from_repo(&repo.name_with_owner, number, query),
            selected: is_changes,
        },
    ]
}

pub fn default_list_back_href(query: Option<&str>) -> String {
    with_query("/prs".to_string(), query)
}

pub fn diff_files_view(files: Vec<PullRequestFile>) -> (Vec<DiffTreeItemView>, Vec<DiffFileView>) {
    let tree_items = files
        .iter()
        .enumerate()
        .map(|(index, file)| DiffTreeItemView {
            id: format!("file-{index}"),
            filename: file.filename.clone(),
            additions: file.additions,
            deletions: file.deletions,
        })
        .collect::<Vec<DiffTreeItemView>>();

    let rendered = files
        .into_iter()
        .enumerate()
        .map(|(index, file)| {
            let lines = parse_patch_lines(file.patch.as_deref());
            DiffFileView {
                id: format!("file-{index}"),
                filename: file.filename,
                status: file.status,
                additions: file.additions,
                deletions: file.deletions,
                changes: file.changes,
                blob_url: file.blob_url,
                previous_filename: file.previous_filename,
                is_collapsed: index > 0 && lines.len() > 80,
                has_patch: !lines.is_empty(),
                lines,
            }
        })
        .collect::<Vec<DiffFileView>>();

    (tree_items, rendered)
}

fn parse_patch_lines(patch: Option<&str>) -> Vec<DiffLineView> {
    let Some(patch) = patch else {
        return Vec::new();
    };

    patch
        .lines()
        .map(|line| DiffLineView {
            kind_class: if line.starts_with("@@") {
                "hunk".to_string()
            } else if line.starts_with('+') {
                "add".to_string()
            } else if line.starts_with('-') {
                "del".to_string()
            } else {
                "ctx".to_string()
            },
            text: line.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        build_list_tabs, diff_files_view, normalize_repo, parse_patch_lines, query_for_status,
        query_from_request,
    };
    use crate::gh::models::{PullRequestFile, PullRequestSearchQuery};
    use crate::http::Request;

    #[test]
    fn query_for_status_preserves_filters() {
        let query = PullRequestSearchQuery {
            org: Some("acme".to_string()),
            repo: Some("acme/widgets".to_string()),
            title: Some("auth".to_string()),
            ..PullRequestSearchQuery::default()
        };

        let encoded = query_for_status(&query, "open");
        assert!(encoded.contains("status=open"));
        assert!(encoded.contains("org=acme"));
        assert!(encoded.contains("repo=acme%2Fwidgets"));
    }

    #[test]
    fn query_for_all_status_omits_default_fields() {
        let encoded = query_for_status(&PullRequestSearchQuery::default(), "all");
        assert_eq!(encoded, "");
    }

    #[test]
    fn normalize_repo_supports_owner_fallback() {
        assert_eq!(
            normalize_repo("widgets", Some("acme")),
            Some("acme/widgets".to_string())
        );
        assert!(normalize_repo("bad/repo/extra", None).is_none());
    }

    #[test]
    fn list_tabs_include_all_states() {
        let tabs = build_list_tabs(&PullRequestSearchQuery::default());
        assert_eq!(tabs.len(), 4);
        assert_eq!(tabs[3].href, "/prs");
    }

    #[test]
    fn canonical_query_strips_empty_and_default_values() {
        let request = Request::from_bytes(
            b"GET /prs?org=example&repo=&status=all&title=&author=&sort=updated&order=desc&view=all HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .expect("request should parse");

        let query = query_from_request(&request);
        assert_eq!(query, Some("org=example".to_string()));
    }

    #[test]
    fn diff_parser_marks_line_kinds() {
        let lines = parse_patch_lines(Some("@@ -1 +1 @@\n-a\n+b\n context"));
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn diff_view_collapses_large_nonfirst_file() {
        let files = vec![
            PullRequestFile {
                filename: "a.rs".to_string(),
                status: "MODIFIED".to_string(),
                additions: 1,
                deletions: 1,
                changes: 2,
                previous_filename: None,
                patch: Some("+a".to_string()),
                blob_url: String::new(),
            },
            PullRequestFile {
                filename: "b.rs".to_string(),
                status: "MODIFIED".to_string(),
                additions: 100,
                deletions: 0,
                changes: 100,
                previous_filename: None,
                patch: Some((0..90).map(|_| "+x\n").collect()),
                blob_url: String::new(),
            },
        ];

        let (_, rendered) = diff_files_view(files);
        assert!(!rendered[0].is_collapsed);
        assert!(rendered[1].is_collapsed);
    }
}

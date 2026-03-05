use crate::gh::models::{PullRequestFile, PullRequestStatus, RepoContext};
use crate::search::SearchArgs;
use crate::views::types::{
    DetailTabView, DiffFileView, DiffLineView, DiffTreeItemView, ListTabView,
};
pub fn clamp_flash(message: String) -> String {
    message.chars().take(240).collect()
}

pub fn state_label(state: String, is_draft: bool) -> String {
    if is_draft {
        format!("{} · DRAFT", state)
    } else {
        state
    }
}

pub fn pr_state_tone(state: &str, is_draft: bool) -> String {
    if is_draft {
        return "state-draft".to_string();
    }

    match state.trim().to_ascii_uppercase().as_str() {
        "OPEN" => "state-open".to_string(),
        "MERGED" => "state-merged".to_string(),
        "CLOSED" => "state-closed".to_string(),
        _ => "state-neutral".to_string(),
    }
}

pub fn review_decision_tone(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "state-approved".to_string(),
        "CHANGES_REQUESTED" => "state-conflict".to_string(),
        "REVIEW_REQUIRED" => "state-open".to_string(),
        "NONE" | "" => "state-neutral".to_string(),
        _ => "state-neutral".to_string(),
    }
}

pub fn merge_state_tone(merge_state_status: &str, mergeable: &str) -> String {
    let merge_state = merge_state_status.trim().to_ascii_uppercase();
    let mergeable = mergeable.trim().to_ascii_uppercase();

    if merge_state.contains("CONFLICT") || mergeable.contains("CONFLICT") {
        return "state-conflict".to_string();
    }

    if merge_state == "CLEAN" || mergeable == "MERGEABLE" {
        return "state-merge".to_string();
    }

    "state-neutral".to_string()
}

pub fn review_state_tone(state: &str) -> String {
    match state.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "state-approved".to_string(),
        "CHANGES_REQUESTED" => "state-conflict".to_string(),
        "COMMENTED" => "state-open".to_string(),
        _ => "state-neutral".to_string(),
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

pub fn build_list_tabs(query: &SearchArgs) -> Vec<ListTabView> {
    let open_query = query
        .with_status(PullRequestStatus::Open)
        .to_query_string()
        .unwrap_or_default();
    let merged_query = query
        .with_status(PullRequestStatus::Merged)
        .to_query_string()
        .unwrap_or_default();
    let closed_query = query
        .with_status(PullRequestStatus::Closed)
        .to_query_string()
        .unwrap_or_default();
    let all_query = query
        .with_status(PullRequestStatus::All)
        .to_query_string()
        .unwrap_or_default();

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
    use super::{build_list_tabs, diff_files_view, parse_patch_lines};
    use crate::gh::models::PullRequestFile;
    use crate::gh::models::PullRequestStatus;
    use crate::search::SearchArgs;

    #[test]
    fn query_for_status_preserves_filters() {
        let query = SearchArgs {
            org: Some("acme".to_string()),
            repo: Some("acme/widgets".to_string()),
            title: Some("auth".to_string()),
            ..SearchArgs::default()
        };

        let encoded = query
            .with_status(PullRequestStatus::Open)
            .to_query_string()
            .expect("query string");
        assert!(encoded.contains("status=open"));
        assert!(encoded.contains("org=acme"));
        assert!(encoded.contains("repo=acme%2Fwidgets"));
    }

    #[test]
    fn query_for_all_status_omits_default_fields() {
        let encoded = SearchArgs::default()
            .with_status(PullRequestStatus::All)
            .to_query_string();
        assert!(encoded.is_none());
    }

    #[test]
    fn list_tabs_include_all_states() {
        let tabs = build_list_tabs(&SearchArgs::default());
        assert_eq!(tabs.len(), 4);
        assert_eq!(tabs[3].href, "/prs");
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

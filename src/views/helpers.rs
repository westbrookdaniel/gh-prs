use crate::gh::models::{
    PullRequestFile, PullRequestOrder, PullRequestSort, PullRequestStatus, RepoContext,
};
use crate::search::SearchArgs;
use crate::views::types::{
    DetailTabView, DiffFileView, DiffLineView, DiffTreeItemView, ListTabView, ReviewerStatusView,
    SortControlView,
};
use ammonia::Builder;
use chrono::{DateTime, Local};
use pulldown_cmark::{Options, Parser, html};
use std::collections::HashSet;

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

pub fn pr_state_tooltip(state: &str, is_draft: bool) -> String {
    if is_draft {
        return "Draft pull request; not ready to merge".to_string();
    }

    match state.trim().to_ascii_uppercase().as_str() {
        "OPEN" => "Open pull request".to_string(),
        "MERGED" => "Pull request merged".to_string(),
        "CLOSED" => "Pull request closed without merge".to_string(),
        value if value.contains("DIRTY") => {
            "Merge is blocked by conflicts with base branch".to_string()
        }
        _ => "Pull request state".to_string(),
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

pub fn review_decision_tooltip(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "Required reviews approved".to_string(),
        "CHANGES_REQUESTED" => "Changes requested by at least one reviewer".to_string(),
        "REVIEW_REQUIRED" => "A review is still required".to_string(),
        "NONE" | "" => "No review decision yet".to_string(),
        _ => "Review decision state".to_string(),
    }
}

pub fn merge_state_tone(merge_state_status: &str, mergeable: &str) -> String {
    let merge_state = merge_state_status.trim().to_ascii_uppercase();
    let mergeable = mergeable.trim().to_ascii_uppercase();

    if merge_state.contains("CONFLICT")
        || merge_state == "DIRTY"
        || merge_state == "BLOCKED"
        || mergeable.contains("CONFLICT")
    {
        return "state-conflict".to_string();
    }

    if merge_state == "CLEAN" || mergeable == "MERGEABLE" {
        return "state-merge".to_string();
    }

    "state-neutral".to_string()
}

pub fn merge_state_tooltip(merge_state_status: &str, mergeable: &str) -> String {
    let merge_state = merge_state_status.trim().to_ascii_uppercase();
    let mergeable = mergeable.trim().to_ascii_uppercase();

    if merge_state.contains("CONFLICT")
        || merge_state == "DIRTY"
        || merge_state == "BLOCKED"
        || mergeable.contains("CONFLICT")
    {
        return "Merge conflicts detected".to_string();
    }
    if merge_state == "CLEAN" || mergeable == "MERGEABLE" {
        return "Ready to merge".to_string();
    }
    if merge_state == "BEHIND" {
        return "Branch is behind base and may need update".to_string();
    }

    "Merge status".to_string()
}

pub fn review_state_tone(state: &str) -> String {
    match state.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "state-approved".to_string(),
        "CHANGES_REQUESTED" => "state-conflict".to_string(),
        "COMMENTED" => "state-open".to_string(),
        _ => "state-neutral".to_string(),
    }
}

pub fn review_state_tooltip(state: &str) -> String {
    match state.trim().to_ascii_uppercase().as_str() {
        "APPROVED" => "Reviewer approved".to_string(),
        "CHANGES_REQUESTED" => "Reviewer requested changes".to_string(),
        "COMMENTED" => "Reviewer left comments".to_string(),
        "REVIEW_REQUIRED" => "Review requested".to_string(),
        _ => "Review state".to_string(),
    }
}

pub fn format_timestamp(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "N/A".to_string();
    }

    if let Ok(parsed) = DateTime::parse_from_rfc3339(trimmed) {
        return parsed
            .with_timezone(&Local)
            .format("%b %-d, %Y at %-I:%M %p")
            .to_string();
    }

    trimmed.to_string()
}

pub fn markdown_to_html(input: &str) -> String {
    if input.trim().is_empty() {
        return String::new();
    }

    let mut rendered = String::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(input, options);
    html::push_html(&mut rendered, parser);

    let mut tags = HashSet::new();
    for tag in [
        "a",
        "p",
        "br",
        "blockquote",
        "pre",
        "code",
        "em",
        "strong",
        "ul",
        "ol",
        "li",
        "hr",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "table",
        "thead",
        "tbody",
        "tr",
        "th",
        "td",
        "span",
    ] {
        tags.insert(tag);
    }

    let mut attrs = HashSet::new();
    attrs.insert("href");
    attrs.insert("title");

    Builder::default()
        .tags(tags)
        .url_schemes(["http", "https", "mailto"].into())
        .generic_attributes(attrs)
        .clean(&rendered)
        .to_string()
}

pub fn author_avatar_url(author: &str, explicit: &str) -> String {
    let explicit = explicit.trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }

    let slug = author.trim().trim_start_matches('@');
    if slug.is_empty() || slug.eq_ignore_ascii_case("unknown") {
        return String::new();
    }

    format!("https://github.com/{slug}.png?size=80")
}

pub fn author_initial(author: &str) -> String {
    author
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

pub fn avatar_style_from_author(author: &str) -> String {
    let mut hash = 0u32;
    for byte in author.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as u32);
    }

    let hue = (hash % 360) as i32;
    let saturation = 38 + ((hash / 360) % 22) as i32;
    let lightness = 40 + ((hash / 8192) % 18) as i32;

    format!(
        "--avatar-bg: hsl({hue}deg {saturation}% {lightness}%); --avatar-fg: hsl({hue}deg 28% 95%);"
    )
}

pub fn sort_controls(query: &SearchArgs) -> Vec<SortControlView> {
    let specs = [
        (PullRequestSort::Updated, "Updated"),
        (PullRequestSort::Created, "Created"),
        (PullRequestSort::Comments, "Comments"),
    ];

    specs
        .into_iter()
        .map(|(sort, label)| {
            let selected = query.sort == sort;
            let target_order = if selected && query.order == PullRequestOrder::Desc {
                PullRequestOrder::Asc
            } else {
                PullRequestOrder::Desc
            };
            let target = query.with_sort_order(sort, target_order);
            let direction = if selected {
                if query.order == PullRequestOrder::Desc {
                    "down"
                } else {
                    "up"
                }
            } else {
                "none"
            };

            SortControlView {
                label: label.to_string(),
                href: with_query("/prs".to_string(), target.to_query_string().as_deref()),
                selected,
                direction: direction.to_string(),
            }
        })
        .collect()
}

pub fn merge_state_explainer(merge_state_status: &str) -> Option<String> {
    let status = merge_state_status.trim().to_ascii_uppercase();
    if status == "BEHIND" {
        return Some("Behind means this branch is behind the base branch and may require an update before merge.".to_string());
    }

    None
}

pub fn build_reviewer_statuses(
    requested_reviewers: &[String],
    latest_reviewer_decisions: &[crate::gh::models::ReviewerDecision],
    reviews: &[crate::gh::models::PullRequestReview],
) -> Vec<ReviewerStatusView> {
    let mut by_reviewer = std::collections::BTreeMap::<String, ReviewerStatusView>::new();

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
                body_html: String::new(),
                is_requested: true,
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
                body_html: markdown_to_html(&decision.body),
                is_requested: requested_reviewers.contains(&decision.reviewer),
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
                body_html: markdown_to_html(&review.body),
                is_requested: requested_reviewers.contains(&review.author),
            });
    }

    by_reviewer.into_values().collect()
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
    use super::{
        build_list_tabs, diff_files_view, format_timestamp, markdown_to_html, parse_patch_lines,
    };
    use crate::gh::models::PullRequestFile;
    use crate::gh::models::PullRequestStatus;
    use crate::search::SearchArgs;

    #[test]
    fn query_for_status_preserves_filters() {
        let query = SearchArgs {
            repos: vec!["acme/widgets".to_string()],
            repo: Some("acme/widgets".to_string()),
            title: Some("auth".to_string()),
            ..SearchArgs::default()
        };

        let encoded = query
            .with_status(PullRequestStatus::Open)
            .to_query_string()
            .expect("query string");
        assert!(encoded.contains("status=open"));
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

    #[test]
    fn formats_timestamp_to_human_readable() {
        let formatted = format_timestamp("2026-01-01T14:08:00Z");
        assert!(formatted.contains("2026"));
        assert!(!formatted.contains("UTC"));
    }

    #[test]
    fn markdown_rendering_strips_unsafe_html() {
        let html = markdown_to_html("hello <script>alert(1)</script>");
        assert!(html.contains("hello"));
        assert!(!html.contains("script"));
    }
}

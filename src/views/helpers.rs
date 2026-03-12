use crate::gh::models::PullRequestFile;
use crate::views::types::{DiffFileView, DiffLineView, DiffTreeItemView};
use ammonia::Builder;
use chrono::{DateTime, Local};
use pulldown_cmark::{Options, Parser, html};
use std::collections::HashSet;

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
            text: highlight_diff_line(line),
        })
        .collect()
}

fn highlight_diff_line(line: &str) -> String {
    if let Some(comment_index) = line.find("//") {
        let (before, comment) = line.split_at(comment_index);
        let before_highlighted = highlight_diff_line(before);
        let comment_escaped = comment
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        return format!(
            "{}<span class=\"tok-comment\">{}</span>",
            before_highlighted, comment_escaped
        );
    }

    let escaped = line
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");

    let mut out = escaped;

    for keyword in [
        "fn", "let", "const", "if", "else", "match", "for", "while", "return", "pub", "impl",
        "struct", "enum", "use", "mod", "class", "function", "var",
    ] {
        let replacement = format!("<span class=\"tok-keyword\">{keyword}</span>");
        out = out.replace(&format!(" {keyword} "), &format!(" {replacement} "));
        out = out.replace(&format!(" {keyword}("), &format!(" {replacement}("));
    }

    out = highlight_quoted(&out, '"', "tok-string");
    out = highlight_quoted(&out, '\'', "tok-string");
    out = highlight_numbers(&out);
    out
}

fn highlight_numbers(input: &str) -> String {
    let mut output = String::new();
    let mut number = String::new();

    for ch in input.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }

        if !number.is_empty() {
            output.push_str(&format!("<span class=\"tok-number\">{}</span>", number));
            number.clear();
        }
        output.push(ch);
    }

    if !number.is_empty() {
        output.push_str(&format!("<span class=\"tok-number\">{}</span>", number));
    }

    output
}

fn highlight_quoted(input: &str, quote: char, class_name: &str) -> String {
    let mut output = String::new();
    let mut in_string = false;
    let mut buffer = String::new();

    for ch in input.chars() {
        if ch == quote {
            if in_string {
                buffer.push(ch);
                output.push_str(&format!("<span class=\"{class_name}\">{}</span>", buffer));
                buffer.clear();
                in_string = false;
            } else {
                in_string = true;
                buffer.push(ch);
            }
            continue;
        }

        if in_string {
            buffer.push(ch);
        } else {
            output.push(ch);
        }
    }

    if !buffer.is_empty() {
        output.push_str(&buffer);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{diff_files_view, format_timestamp, markdown_to_html, parse_patch_lines};
    use crate::gh::models::PullRequestFile;

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

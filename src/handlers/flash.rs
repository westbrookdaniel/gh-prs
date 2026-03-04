use crate::http::{Request, Response};
use crate::views::FlashMessageView;

const FLASH_MAX_LEN: usize = 220;

pub fn flash_from_query(request: &Request) -> Option<FlashMessageView> {
    let kind = request.query_param("flash")?;
    let message = request.query_param("message")?.trim();
    if message.is_empty() {
        return None;
    }

    let message = message.chars().take(FLASH_MAX_LEN).collect::<String>();

    match kind {
        "success" => Some(FlashMessageView::success(message)),
        "error" => Some(FlashMessageView::error(message)),
        _ => None,
    }
}

pub fn redirect_with_flash(location: &str, flash: FlashMessageView) -> Response {
    let query = serde_urlencoded::to_string([
        ("flash", flash.kind.as_str()),
        ("message", flash.message.as_str()),
    ])
    .unwrap_or_default();

    let destination = if query.is_empty() {
        location.to_string()
    } else {
        format!("{location}?{query}")
    };

    Response::new(303, "See Other")
        .header("Location", destination)
        .text_body("See Other")
}

pub fn redirect_to_repo_pr(
    repo: &str,
    number: u64,
    flash: FlashMessageView,
    query: Option<&str>,
) -> Response {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}")
    } else {
        format!("/prs/{number}")
    };

    let location = if let Some(query) = query {
        if query.is_empty() {
            base
        } else {
            format!("{base}?{query}")
        }
    } else {
        base
    };

    redirect_with_flash(&location, flash)
}

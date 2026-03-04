use askama::Template;

use crate::gh::GhError;
use crate::http::Response;
use crate::views::{ErrorTemplate, error_page_model};

pub fn render_gh_error(error: GhError) -> Response {
    let status = error.status_code();
    let reason = reason_phrase(status);
    let model = error_page_model(&error);
    let template = ErrorTemplate { model };
    render_template(status, reason, &template)
}

pub fn render_template(status: u16, reason: &'static str, template: &impl Template) -> Response {
    match template.render() {
        Ok(html) => Response::new(status, reason)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(html),
        Err(err) => {
            Response::internal_server_error().text_body(format!("failed to render template: {err}"))
        }
    }
}

pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

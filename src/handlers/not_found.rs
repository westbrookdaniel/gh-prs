use crate::handlers::format::render_template;
use crate::http::{Request, Response};
use crate::views::{NotFoundTemplate, not_found_page_model};

pub async fn not_found(_request: Request) -> Response {
    let template = NotFoundTemplate {
        model: not_found_page_model(),
    };
    render_template(404, "Not Found", &template)
}

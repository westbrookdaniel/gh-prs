use crate::handlers::flash::flash_from_query;
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::state::SharedState;
use crate::http::{Request, Response};
use crate::views::{PrListTemplate, list_page_model, parse_search_query};

pub async fn list_pull_requests(request: Request, state: SharedState) -> Response {
    let flash = flash_from_query(&request);

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let query = parse_search_query(&request);

    match state.gh.search_pull_requests(&query).await {
        Ok(items) => {
            let model = list_page_model(
                state.startup_repo.as_ref(),
                state.diagnostics.as_ref(),
                &query,
                items,
                flash,
                &request,
            );
            let template = PrListTemplate { model };
            render_template(200, "OK", &template)
        }
        Err(err) => render_gh_error(err),
    }
}

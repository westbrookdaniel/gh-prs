use crate::handlers::flash::flash_from_query;
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::{PrListTemplate, SearchArgs, list_page_model};

pub async fn list_pull_requests(request: Request) -> Response {
    let flash = flash_from_query(&request);
    let state = app_state_snapshot();

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let query = SearchArgs::from_request(&request);

    let available_repos = state.gh.accessible_repositories().await.unwrap_or_default();

    match state.gh.search_pull_requests(&query).await {
        Ok(items) => {
            let model = list_page_model(
                state.startup_repo.as_ref(),
                state.diagnostics.as_ref(),
                &query,
                available_repos,
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

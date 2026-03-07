use crate::handlers::flash::flash_from_query;
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::{PageLoadMode, loadable_from_cached};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::{ListPageModelInput, PrListTemplate, SearchArgs, list_page_model};

pub async fn list_pull_requests(request: Request) -> Response {
    let flash = flash_from_query(&request);
    let state = app_state_snapshot();

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let query = SearchArgs::from_request(&request);
    let load_mode = PageLoadMode::from_request(&request);

    let repo_options = if load_mode.bypass_cache() {
        match state.gh.refresh_accessible_repositories().await {
            Ok(value) => crate::views::types::Loadable::ready(value, false),
            Err(err) => return render_gh_error(err),
        }
    } else {
        match state.gh.cached_accessible_repositories().await {
            Ok(value) => loadable_from_cached(value),
            Err(err) => return render_gh_error(err),
        }
    };

    let results = if load_mode.bypass_cache() {
        match state.gh.refresh_search_pull_requests(&query).await {
            Ok(value) => crate::views::types::Loadable::ready(value, false),
            Err(err) => return render_gh_error(err),
        }
    } else {
        match state.gh.cached_search_pull_requests(&query).await {
            Ok(value) => loadable_from_cached(value),
            Err(err) => return render_gh_error(err),
        }
    };
    let needs_refresh = !load_mode.bypass_cache()
        && (repo_options.needs_refresh() || results.needs_refresh());

    let model = list_page_model(ListPageModelInput {
        query: &query,
        repo_options,
        results,
        needs_refresh,
        flash,
        request: &request,
    });
    let template = PrListTemplate { model };
    render_template(200, "OK", &template)
}

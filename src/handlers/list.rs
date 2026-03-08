use crate::handlers::flash::flash_from_query;
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::{PageLoadMode, loadable_from_cached};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::{ListPageModelInput, PrListTemplate, SearchArgs, list_page_model};
use futures_lite::future;

#[tracing::instrument(
    name = "handler.list_pull_requests",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn list_pull_requests(request: Request) -> Response {
    let flash = flash_from_query(&request);
    let state = app_state_snapshot();

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let query = SearchArgs::from_request(&request);
    let load_mode = PageLoadMode::from_request(&request);

    let repo_options_future = {
        let gh = state.gh.clone();
        async move {
            match gh.cached_accessible_repositories().await? {
                Some(value) => Ok(loadable_from_cached(Some(value))),
                None => gh
                    .refresh_accessible_repositories()
                    .await
                    .map(|value| crate::views::types::Loadable::ready(value, false)),
            }
        }
    };

    let results_future = {
        let gh = state.gh.clone();
        let query = query.clone();
        async move {
            if load_mode.bypass_cache() {
                gh.refresh_search_pull_requests(&query)
                    .await
                    .map(|value| crate::views::types::Loadable::ready(value, false))
            } else {
                gh.cached_search_pull_requests(&query)
                    .await
                    .map(loadable_from_cached)
            }
        }
    };

    let (repo_options, results) = future::zip(repo_options_future, results_future).await;
    let repo_options = match repo_options {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };
    let results = match results {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };
    let needs_refresh =
        !load_mode.bypass_cache() && (repo_options.needs_refresh() || results.needs_refresh());

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

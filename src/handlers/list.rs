use crate::handlers::flash::flash_from_query;
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::{ListPageModelInput, PrListTemplate, SearchArgs, list_page_model};
use smol::spawn;

pub async fn list_pull_requests(request: Request) -> Response {
    let flash = flash_from_query(&request);
    let state = app_state_snapshot();

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let query = SearchArgs::from_request(&request);

    let available_repos = match state.gh.cached_accessible_repositories().await {
        Ok(Some(cached)) => cached.value,
        Ok(None) => {
            let gh = state.gh.clone();
            spawn(async move {
                let _ = gh.refresh_accessible_repositories().await;
            })
            .detach();
            Vec::new()
        }
        Err(err) => return render_gh_error(err),
    };

    let cached_items = match state.gh.cached_search_pull_requests(&query).await {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };

    let (items, is_loading) = match cached_items {
        Some(cached) => (cached.value, false),
        None => (Vec::new(), true),
    };

    let model = list_page_model(ListPageModelInput {
        repo: state.startup_repo.as_ref(),
        diagnostics: state.diagnostics.as_ref(),
        query: &query,
        available_repos,
        items,
        is_loading,
        flash,
        request: &request,
    });
    let template = PrListTemplate { model };
    render_template(200, "OK", &template)
}

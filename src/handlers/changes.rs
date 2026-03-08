use crate::gh::models::RepoContext;
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::flash::flash_from_query;
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::{PageLoadMode, loadable_from_cached};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::{PrChangesTemplate, changes_page_model};
use futures_lite::future;

#[tracing::instrument(
    name = "handler.pull_request_changes",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn pull_request_changes(request: Request) -> Response {
    let flash = flash_from_query(&request);
    let state = app_state_snapshot();

    if let Err(err) = state.startup_ready() {
        return render_gh_error(err);
    }

    let repo_name = match repo_from_request(&request, state.startup_repo.as_ref()) {
        Ok(repo) => repo,
        Err(err) => return render_gh_error(err),
    };

    let repo_context = RepoContext {
        name_with_owner: repo_name.clone(),
        url: format!("https://github.com/{repo_name}"),
        viewer_permission: "UNKNOWN".to_string(),
        default_branch: "main".to_string(),
    };

    let number = match parse_pr_number(&request) {
        Ok(number) => number,
        Err(err) => return render_gh_error(err),
    };

    let load_mode = PageLoadMode::from_request(&request);

    let detail_future = {
        let gh = state.gh.clone();
        let repo_name = repo_name.clone();
        async move {
            if load_mode.bypass_cache() {
                gh.refresh_pull_request_conversation(&repo_name, number)
                    .await
                    .map(|value| crate::views::types::Loadable::ready(value.detail, false))
            } else {
                gh.cached_pull_request_conversation(&repo_name, number)
                    .await
                    .map(|value| match value {
                        Some(cached) => crate::views::types::Loadable::ready(
                            cached.value.detail,
                            cached.is_stale,
                        ),
                        None => crate::views::types::Loadable::missing(),
                    })
            }
        }
    };
    let files_future = {
        let gh = state.gh.clone();
        let repo_name = repo_name.clone();
        async move {
            if load_mode.bypass_cache() {
                gh.refresh_pull_request_files(&repo_name, number)
                    .await
                    .map(|value| crate::views::types::Loadable::ready(value, false))
            } else {
                gh.cached_pull_request_files(&repo_name, number)
                    .await
                    .map(loadable_from_cached)
            }
        }
    };

    let (detail, files) = future::zip(detail_future, files_future).await;
    let detail = match detail {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };
    let files = match files {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };
    let needs_refresh =
        !load_mode.bypass_cache() && (detail.needs_refresh() || files.needs_refresh());

    let model = changes_page_model(
        &repo_context,
        number,
        detail,
        files,
        needs_refresh,
        flash,
        &request,
    );
    let template = PrChangesTemplate { model };
    render_template(200, "OK", &template)
}

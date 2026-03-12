use crate::gh::models::RepoContext;
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::PageLoadMode;
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::PrChangesTemplate;
use crate::views::helpers::{
    build_detail_tabs, default_list_back_href, repo_action_path, with_query,
};
use crate::views::types::{PrChangesPageModel, diff_files_view, pr_header_view};

#[tracing::instrument(
    name = "handler.pull_request_changes",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn pull_request_changes(request: Request) -> Response {
    match build_changes_page_model(&request).await {
        Ok(model) => {
            let template = PrChangesTemplate { model };
            render_template(200, "OK", &template)
        }
        Err(err) => render_gh_error(err),
    }
}

async fn build_changes_page_model(request: &Request) -> crate::gh::GhResult<PrChangesPageModel> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
    let repo_context = RepoContext {
        name_with_owner: repo_name.clone(),
        url: format!("https://github.com/{repo_name}"),
        viewer_permission: "UNKNOWN".to_string(),
        default_branch: "main".to_string(),
    };
    let number = parse_pr_number(request)?;
    let query = crate::search::SearchArgs::from_request(request).to_query_string();
    let load_mode = PageLoadMode::from_request(request);

    let (detail, detail_needs_refresh) = if load_mode.bypass_cache() {
        (
            Some(
                state
                    .gh
                    .refresh_pull_request_conversation(&repo_name, number)
                    .await?
                    .detail,
            ),
            false,
        )
    } else {
        match state
            .gh
            .cached_pull_request_conversation(&repo_name, number)
            .await?
        {
            Some(cached) => (Some(cached.value.detail), cached.is_stale),
            None => (None, true),
        }
    };

    let (files, files_needs_refresh) = if load_mode.bypass_cache() {
        (
            Some(
                state
                    .gh
                    .refresh_pull_request_files(&repo_name, number)
                    .await?,
            ),
            false,
        )
    } else {
        match state
            .gh
            .cached_pull_request_files(&repo_name, number)
            .await?
        {
            Some(cached) => (Some(cached.value), cached.is_stale),
            None => (None, true),
        }
    };

    let files_loading = files.is_none();
    let (tree_items, rendered_files) = files.map(diff_files_view).unwrap_or_default();
    let header = detail
        .as_ref()
        .map(|detail| pr_header_view(&repo_context, detail));

    Ok(PrChangesPageModel {
        page_title: format!("PR #{number} Changes"),
        refresh_path: with_query(request.path.clone(), request.query.as_deref()),
        needs_refresh: !load_mode.bypass_cache() && (detail_needs_refresh || files_needs_refresh),
        files_loading,
        header,
        rendered_files,
        tree_items,
        back_to_list_href: default_list_back_href(query.as_deref()),
        state_post_path: repo_action_path(
            &repo_context.name_with_owner,
            number,
            "state",
            query.as_deref(),
        ),
        tabs: build_detail_tabs(&repo_context, number, query.as_deref(), true),
    })
}

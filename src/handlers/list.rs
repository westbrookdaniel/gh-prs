use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::PageLoadMode;
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::helpers::{
    build_list_tabs, detail_path_from_repo, format_timestamp, sort_controls, state_label,
    with_query,
};
use crate::views::types::{ListResultRowView, PrListPageModel, RepoOptionView};
use crate::views::{PrListTemplate, SearchArgs};

#[tracing::instrument(
    name = "handler.list_pull_requests",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn list_pull_requests(request: Request) -> Response {
    match build_list_page_model(&request).await {
        Ok(model) => {
            let template = PrListTemplate { model };
            render_template(200, "OK", &template)
        }
        Err(err) => render_gh_error(err),
    }
}

async fn build_list_page_model(request: &Request) -> crate::gh::GhResult<PrListPageModel> {
    let state = app_state_snapshot();
    state.startup_ready()?;

    let query = SearchArgs::from_request(request);
    let load_mode = PageLoadMode::from_request(request);

    let cached_repo_options = state.gh.cached_accessible_repositories().await?;
    let (repo_options, repo_options_needs_refresh) = match cached_repo_options {
        Some(cached) => (
            Some(
                cached
                    .value
                    .into_iter()
                    .map(|name| RepoOptionView {
                        selected: query.repos.iter().any(|repo| repo == &name),
                        name,
                    })
                    .collect(),
            ),
            cached.is_stale,
        ),
        None => (
            Some(
                state
                    .gh
                    .refresh_accessible_repositories()
                    .await?
                    .into_iter()
                    .map(|name| RepoOptionView {
                        selected: query.repos.iter().any(|repo| repo == &name),
                        name,
                    })
                    .collect(),
            ),
            false,
        ),
    };

    let (results, results_needs_refresh) = if load_mode.bypass_cache() {
        (
            Some(
                state
                    .gh
                    .refresh_search_pull_requests(&query)
                    .await?
                    .into_iter()
                    .map(|item| ListResultRowView {
                        repository_name_with_owner: item.repository_name_with_owner.clone(),
                        state_label: state_label(item.state, item.is_draft),
                        detail_path: detail_path_from_repo(
                            &item.repository_name_with_owner,
                            item.number,
                            query.to_query_string().as_deref(),
                        ),
                        number: item.number,
                        title: item.title,
                        author: item.author,
                        comment_count: item.comment_count,
                        updated_at_display: format_timestamp(&item.updated_at),
                        updated_at: item.updated_at,
                    })
                    .collect(),
            ),
            false,
        )
    } else {
        match state.gh.cached_search_pull_requests(&query).await? {
            Some(cached) => (
                Some(
                    cached
                        .value
                        .into_iter()
                        .map(|item| ListResultRowView {
                            repository_name_with_owner: item.repository_name_with_owner.clone(),
                            state_label: state_label(item.state, item.is_draft),
                            detail_path: detail_path_from_repo(
                                &item.repository_name_with_owner,
                                item.number,
                                query.to_query_string().as_deref(),
                            ),
                            number: item.number,
                            title: item.title,
                            author: item.author,
                            comment_count: item.comment_count,
                            updated_at_display: format_timestamp(&item.updated_at),
                            updated_at: item.updated_at,
                        })
                        .collect(),
                ),
                cached.is_stale,
            ),
            None => (None, true),
        }
    };

    let row_count = results.as_ref().map_or(0, Vec::len);
    let has_results_limit_warning = query.limit >= crate::gh::models::DEFAULT_SEARCH_LIMIT
        && row_count >= crate::gh::models::DEFAULT_SEARCH_LIMIT;

    Ok(PrListPageModel {
        page_title: "Pull Requests Across Your Repos".to_string(),
        refresh_path: with_query(request.path.clone(), request.query.as_deref()),
        needs_refresh: !load_mode.bypass_cache()
            && (repo_options_needs_refresh || results_needs_refresh),
        repo_options,
        results,
        sort_controls: sort_controls(&query),
        tabs: build_list_tabs(&query),
        title_value: query.title.clone().unwrap_or_default(),
        author_value: query.author.clone().unwrap_or_default(),
        status_value: query.status.as_query_value().to_string(),
        sort_value: query.sort.as_query_value().to_string(),
        order_value: query.order.as_query_value().to_string(),
        row_count,
        has_results_limit_warning,
    })
}

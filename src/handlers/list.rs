use crate::gh::client::GhClient;
use crate::gh::models::{
    DEFAULT_SEARCH_LIMIT, PullRequestOrder, PullRequestSearchItem, PullRequestSort,
    PullRequestStatus,
};
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::PageLoadMode;
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::search::SearchArgs;
use crate::views::PrListTemplate;
use crate::views::helpers::format_timestamp;
use crate::views::paths::{detail_path, list_path, with_query};
use crate::views::types::{
    ListResultRowView, ListTabView, PrListPageModel, RepoOptionView, SortControlView,
};

#[tracing::instrument(
    name = "handler.list_pull_requests",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn list_pull_requests(request: Request) -> Response {
    match render_list_page(&request).await {
        Ok(response) => response,
        Err(err) => render_gh_error(err),
    }
}

async fn render_list_page(request: &Request) -> crate::gh::GhResult<Response> {
    let context = ListPageContext::from_request(request)?;
    let data = if context.bypass_cache {
        load_list_fresh(&context).await?
    } else {
        load_list_cached(&context).await?
    };

    let template = PrListTemplate {
        model: build_list_page_model(&context, data),
    };

    Ok(render_template(200, "OK", &template))
}

struct ListPageContext {
    gh: GhClient,
    query: SearchArgs,
    query_string: Option<String>,
    refresh_path: String,
    bypass_cache: bool,
}

impl ListPageContext {
    fn from_request(request: &Request) -> crate::gh::GhResult<Self> {
        let state = app_state_snapshot();
        state.startup_ready()?;

        let query = SearchArgs::from_request(request);
        let query_string = query.to_query_string();

        Ok(Self {
            gh: state.gh,
            refresh_path: with_query(request.path.clone(), request.query.as_deref()),
            bypass_cache: PageLoadMode::from_request(request).bypass_cache(),
            query,
            query_string,
        })
    }
}

struct ListData {
    repo_options: Option<Vec<String>>,
    repo_options_stale: bool,
    results: Option<Vec<PullRequestSearchItem>>,
    results_stale: bool,
}

async fn load_list_fresh(context: &ListPageContext) -> crate::gh::GhResult<ListData> {
    let repo_options = match context.gh.cached_accessible_repositories().await? {
        Some(cached) => cached.value,
        None => context.gh.refresh_accessible_repositories().await?,
    };

    Ok(ListData {
        repo_options: Some(repo_options),
        repo_options_stale: false,
        results: Some(
            context
                .gh
                .refresh_search_pull_requests(&context.query)
                .await?,
        ),
        results_stale: false,
    })
}

async fn load_list_cached(context: &ListPageContext) -> crate::gh::GhResult<ListData> {
    let cached_repo_options = context.gh.cached_accessible_repositories().await?;
    let cached_results = context
        .gh
        .cached_search_pull_requests(&context.query)
        .await?;

    let (repo_options, repo_options_stale) = match cached_repo_options {
        Some(cached) => (Some(cached.value), cached.is_stale),
        None => (None, true),
    };
    let (results, results_stale) = match cached_results {
        Some(cached) => (Some(cached.value), cached.is_stale),
        None => (None, true),
    };

    Ok(ListData {
        repo_options,
        repo_options_stale,
        results,
        results_stale,
    })
}

fn build_list_page_model(context: &ListPageContext, data: ListData) -> PrListPageModel {
    let results = data.results.map(|items| map_list_results(&items, context));
    let row_count = results.as_ref().map_or(0, Vec::len);

    PrListPageModel {
        page_title: "Pull Requests Across Your Repos".to_string(),
        refresh_path: context.refresh_path.clone(),
        needs_refresh: !context.bypass_cache && (data.repo_options_stale || data.results_stale),
        repo_options: data
            .repo_options
            .map(|repos| map_repo_options(&repos, &context.query)),
        results,
        sort_controls: list_sort_controls(&context.query),
        tabs: list_tabs(&context.query),
        title_value: context.query.title.clone().unwrap_or_default(),
        author_value: context.query.author.clone().unwrap_or_default(),
        status_value: context.query.status.as_query_value().to_string(),
        sort_value: context.query.sort.as_query_value().to_string(),
        order_value: context.query.order.as_query_value().to_string(),
        row_count,
        has_results_limit_warning: context.query.limit >= DEFAULT_SEARCH_LIMIT
            && row_count >= DEFAULT_SEARCH_LIMIT,
    }
}

fn map_repo_options(repos: &[String], query: &SearchArgs) -> Vec<RepoOptionView> {
    repos
        .iter()
        .map(|repo| RepoOptionView {
            name: repo.clone(),
            selected: query.repos.iter().any(|selected| selected == repo),
        })
        .collect()
}

fn map_list_results(
    items: &[PullRequestSearchItem],
    context: &ListPageContext,
) -> Vec<ListResultRowView> {
    items
        .iter()
        .map(|item| ListResultRowView {
            repository_name_with_owner: item.repository_name_with_owner.clone(),
            state_label: list_state_label(&item.state, item.is_draft),
            detail_path: detail_path(
                &item.repository_name_with_owner,
                item.number,
                context.query_string.as_deref(),
            ),
            number: item.number,
            title: item.title.clone(),
            author: item.author.clone(),
            comment_count: item.comment_count,
            updated_at: item.updated_at.clone(),
            updated_at_display: format_timestamp(&item.updated_at),
        })
        .collect()
}

fn list_sort_controls(query: &SearchArgs) -> Vec<SortControlView> {
    let specs = [
        (PullRequestSort::Updated, "Updated"),
        (PullRequestSort::Created, "Created"),
        (PullRequestSort::Comments, "Comments"),
    ];

    specs
        .into_iter()
        .map(|(sort, label)| {
            let selected = query.sort == sort;
            let order = if selected && query.order == PullRequestOrder::Desc {
                PullRequestOrder::Asc
            } else {
                PullRequestOrder::Desc
            };
            let direction = if selected {
                if query.order == PullRequestOrder::Desc {
                    "down"
                } else {
                    "up"
                }
            } else {
                "none"
            };

            SortControlView {
                label: label.to_string(),
                href: list_path(
                    query
                        .with_sort_order(sort, order)
                        .to_query_string()
                        .as_deref(),
                ),
                selected,
                direction: direction.to_string(),
            }
        })
        .collect()
}

fn list_tabs(query: &SearchArgs) -> Vec<ListTabView> {
    [
        (PullRequestStatus::Open, "Open"),
        (PullRequestStatus::Merged, "Merged"),
        (PullRequestStatus::Closed, "Closed"),
        (PullRequestStatus::All, "All"),
    ]
    .into_iter()
    .map(|(status, label)| ListTabView {
        label: label.to_string(),
        href: list_path(query.with_status(status).to_query_string().as_deref()),
        selected: query.status == status,
    })
    .collect()
}

fn list_state_label(state: &str, is_draft: bool) -> String {
    if is_draft {
        format!("{} · DRAFT", state)
    } else {
        state.to_string()
    }
}

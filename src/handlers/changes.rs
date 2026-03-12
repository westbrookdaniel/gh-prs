use crate::gh::client::GhClient;
use crate::gh::models::{PullRequestDetail, PullRequestFile, RepoContext};
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::load::PageLoadMode;
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::PrChangesTemplate;
use crate::views::paths::{changes_path, list_path, repo_action_path, with_query};
use crate::views::types::{DetailTabView, PrChangesPageModel, diff_files_view, pr_header_view};

#[tracing::instrument(
    name = "handler.pull_request_changes",
    skip(request),
    fields(
        http.request.method = %request.method,
        http.route = request.matched_route().unwrap_or(request.path.as_str())
    )
)]
pub async fn pull_request_changes(request: Request) -> Response {
    match render_changes_page(&request).await {
        Ok(response) => response,
        Err(err) => render_gh_error(err),
    }
}

async fn render_changes_page(request: &Request) -> crate::gh::GhResult<Response> {
    let context = ChangesPageContext::from_request(request)?;
    let data = if context.bypass_cache {
        load_changes_fresh(&context).await?
    } else {
        load_changes_cached(&context).await?
    };

    let template = PrChangesTemplate {
        model: build_changes_page_model(&context, data),
    };

    Ok(render_template(200, "OK", &template))
}

struct ChangesPageContext {
    gh: GhClient,
    repo: RepoContext,
    number: u64,
    query_string: Option<String>,
    refresh_path: String,
    bypass_cache: bool,
}

impl ChangesPageContext {
    fn from_request(request: &Request) -> crate::gh::GhResult<Self> {
        let state = app_state_snapshot();
        state.startup_ready()?;

        let repo_name = repo_from_request(request, state.startup_repo.as_ref())?;
        let number = parse_pr_number(request)?;

        Ok(Self {
            gh: state.gh,
            repo: RepoContext {
                name_with_owner: repo_name.clone(),
                url: format!("https://github.com/{repo_name}"),
                viewer_permission: "UNKNOWN".to_string(),
                default_branch: "main".to_string(),
            },
            number,
            query_string: crate::search::SearchArgs::from_request(request).to_query_string(),
            refresh_path: with_query(request.path.clone(), request.query.as_deref()),
            bypass_cache: PageLoadMode::from_request(request).bypass_cache(),
        })
    }
}

struct ChangesData {
    detail: Option<PullRequestDetail>,
    detail_stale: bool,
    files: Option<Vec<PullRequestFile>>,
    files_stale: bool,
}

async fn load_changes_fresh(context: &ChangesPageContext) -> crate::gh::GhResult<ChangesData> {
    Ok(ChangesData {
        detail: Some(
            context
                .gh
                .refresh_pull_request_conversation(&context.repo.name_with_owner, context.number)
                .await?
                .detail,
        ),
        detail_stale: false,
        files: Some(
            context
                .gh
                .refresh_pull_request_files(&context.repo.name_with_owner, context.number)
                .await?,
        ),
        files_stale: false,
    })
}

async fn load_changes_cached(context: &ChangesPageContext) -> crate::gh::GhResult<ChangesData> {
    let cached_detail = context
        .gh
        .cached_pull_request_conversation(&context.repo.name_with_owner, context.number)
        .await?;
    let cached_files = context
        .gh
        .cached_pull_request_files(&context.repo.name_with_owner, context.number)
        .await?;

    let (detail, detail_stale) = match cached_detail {
        Some(cached) => (Some(cached.value.detail), cached.is_stale),
        None => (None, true),
    };
    let (files, files_stale) = match cached_files {
        Some(cached) => (Some(cached.value), cached.is_stale),
        None => (None, true),
    };

    Ok(ChangesData {
        detail,
        detail_stale,
        files,
        files_stale,
    })
}

fn build_changes_page_model(context: &ChangesPageContext, data: ChangesData) -> PrChangesPageModel {
    let files_loading = data.files.is_none();
    let (tree_items, rendered_files) = data.files.map(diff_files_view).unwrap_or_default();

    PrChangesPageModel {
        page_title: format!("PR #{} Changes", context.number),
        refresh_path: context.refresh_path.clone(),
        needs_refresh: !context.bypass_cache && (data.detail_stale || data.files_stale),
        files_loading,
        header: data
            .detail
            .as_ref()
            .map(|detail| pr_header_view(&context.repo, detail)),
        rendered_files,
        tree_items,
        back_to_list_href: list_path(context.query_string.as_deref()),
        state_post_path: repo_action_path(
            &context.repo.name_with_owner,
            context.number,
            "state",
            context.query_string.as_deref(),
        ),
        tabs: detail_tabs(
            &context.repo.name_with_owner,
            context.number,
            context.query_string.as_deref(),
        ),
    }
}

fn detail_tabs(repo: &str, number: u64, query: Option<&str>) -> Vec<DetailTabView> {
    vec![
        DetailTabView {
            label: "Conversation".to_string(),
            href: crate::views::paths::detail_path(repo, number, query),
            selected: false,
        },
        DetailTabView {
            label: "Changes".to_string(),
            href: changes_path(repo, number, query),
            selected: true,
        },
    ]
}

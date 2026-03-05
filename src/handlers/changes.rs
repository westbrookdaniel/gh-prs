use crate::gh::models::RepoContext;
use crate::handlers::context::{parse_pr_number, repo_from_request};
use crate::handlers::flash::flash_from_query;
use crate::handlers::format::{render_gh_error, render_template};
use crate::handlers::state::app_state_snapshot;
use crate::http::{Request, Response};
use crate::views::{PrChangesTemplate, changes_page_model};

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

    let conversation = match state.gh.pull_request_conversation(&repo_name, number).await {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };

    let files = match state.gh.pull_request_files(&repo_name, number).await {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };

    let model = changes_page_model(&repo_context, conversation.detail, files, flash, &request);
    let template = PrChangesTemplate { model };
    render_template(200, "OK", &template)
}

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

    let maybe_conversation = match state
        .gh
        .cached_pull_request_conversation(&repo_name, number)
        .await
    {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };
    let maybe_files = match state.gh.cached_pull_request_files(&repo_name, number).await {
        Ok(value) => value,
        Err(err) => return render_gh_error(err),
    };

    let detail = maybe_conversation
        .map(|cached| cached.value.detail)
        .unwrap_or(crate::gh::models::PullRequestDetail {
            number,
            title: "Loading pull request changes...".to_string(),
            body: String::new(),
            state: "OPEN".to_string(),
            is_draft: false,
            author: "loading".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            url: format!("https://github.com/{repo_name}/pull/{number}"),
            base_ref_name: "main".to_string(),
            head_ref_name: "...".to_string(),
            merge_state_status: "UNKNOWN".to_string(),
            mergeable: "UNKNOWN".to_string(),
            review_decision: None,
            requested_reviewers: Vec::new(),
            latest_reviewer_decisions: Vec::new(),
            checks: crate::gh::models::StatusChecksSummary::default(),
            commit_count: 0,
            file_count: 0,
        });
    let files = maybe_files.map(|cached| cached.value).unwrap_or_default();
    let is_loading = files.is_empty();

    let model = changes_page_model(&repo_context, detail, files, is_loading, flash, &request);
    let template = PrChangesTemplate { model };
    render_template(200, "OK", &template)
}

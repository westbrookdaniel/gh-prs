use super::GhClient;
use crate::gh::commands::{MergeMethod, PullRequestStateTransition, ReviewEvent};
use crate::gh::models::{
    IssueComment, PullRequestDetail, PullRequestReview, PullRequestReviewComment,
};
use crate::gh::validation::{normalize_write_body, validate_pr_number, validate_repo_identifier};
use crate::gh::{CommandClass, GhError, GhResult};
use crate::gh_parsing::{
    parse_issue_comments, parse_pull_request_detail, parse_pull_request_review_comments,
    parse_pull_request_reviews,
};

impl GhClient {
    pub async fn submit_comment(&self, repo: &str, number: u64, body: &str) -> GhResult<()> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;
        let body = normalize_write_body(body)?;

        self.run_raw_command_with_stdin(
            CommandClass::SubmitComment,
            vec![
                "pr".to_string(),
                "comment".to_string(),
                number.to_string(),
                "-R".to_string(),
                repo.clone(),
                "--body-file".to_string(),
                "-".to_string(),
            ],
            body.into_bytes(),
            Some(repo),
            Some(number),
        )
        .await?;

        self.invalidate_pr_cache().await
    }

    pub async fn submit_review(
        &self,
        repo: &str,
        number: u64,
        event: ReviewEvent,
        body: &str,
    ) -> GhResult<()> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;
        let body = normalize_write_body(body)?;

        self.run_raw_command_with_stdin(
            CommandClass::SubmitReview,
            vec![
                "pr".to_string(),
                "review".to_string(),
                number.to_string(),
                "-R".to_string(),
                repo.clone(),
                event.gh_flag().to_string(),
                "--body-file".to_string(),
                "-".to_string(),
            ],
            body.into_bytes(),
            Some(repo),
            Some(number),
        )
        .await?;

        self.invalidate_pr_cache().await
    }

    pub async fn update_reviewers(
        &self,
        repo: &str,
        number: u64,
        reviewers: Vec<String>,
    ) -> GhResult<()> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;
        if reviewers.is_empty() {
            return Err(GhError::InvalidInput {
                field: "reviewers".to_string(),
                details: "provide at least one reviewer".to_string(),
            });
        }

        self.run_raw_command_with_context(
            CommandClass::UpdateReviewers,
            vec![
                "pr".to_string(),
                "edit".to_string(),
                number.to_string(),
                "-R".to_string(),
                repo.clone(),
                "--add-reviewer".to_string(),
                reviewers.join(","),
            ],
            Some(repo),
            Some(number),
        )
        .await?;

        self.invalidate_pr_cache().await
    }

    pub async fn merge_pull_request(
        &self,
        repo: &str,
        number: u64,
        method: MergeMethod,
        delete_branch: bool,
    ) -> GhResult<()> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;

        let mut args = vec![
            "pr".to_string(),
            "merge".to_string(),
            number.to_string(),
            "-R".to_string(),
            repo.clone(),
            method.gh_flag().to_string(),
            "--auto".to_string(),
        ];
        if delete_branch {
            args.push("--delete-branch".to_string());
        }

        self.run_raw_command_with_context(
            CommandClass::MergePullRequest,
            args,
            Some(repo),
            Some(number),
        )
        .await?;

        self.invalidate_pr_cache().await
    }

    pub async fn update_pull_request_state(
        &self,
        repo: &str,
        number: u64,
        transition: PullRequestStateTransition,
    ) -> GhResult<()> {
        let repo = validate_repo_identifier(repo)?;
        validate_pr_number(number)?;

        self.run_raw_command_with_context(
            CommandClass::UpdatePullRequestState,
            vec![
                "pr".to_string(),
                transition.as_gh_subcommand().to_string(),
                number.to_string(),
                "-R".to_string(),
                repo.clone(),
            ],
            Some(repo),
            Some(number),
        )
        .await?;

        self.invalidate_pr_cache().await
    }

    async fn invalidate_pr_cache(&self) -> GhResult<()> {
        self.cache
            .invalidate_prefix(&self.cache_key("pr|"))
            .await
            .map_err(|err| GhError::Internal(format!("failed invalidating cache: {err}")))?;
        Ok(())
    }

    pub(super) async fn fetch_pull_request_detail(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<PullRequestDetail> {
        let result = self
            .run_raw_command_with_context(
                CommandClass::PullRequestDetail,
                vec![
                    "pr".to_string(),
                    "view".to_string(),
                    number.to_string(),
                    "-R".to_string(),
                    repo.to_string(),
                    "--json".to_string(),
                    "number,title,body,state,isDraft,author,createdAt,updatedAt,url,baseRefName,headRefName,mergeStateStatus,mergeable,reviewDecision,reviewRequests,latestReviews,statusCheckRollup,commits,files,comments".to_string(),
                ],
                Some(repo.to_string()),
                Some(number),
            )
            .await?;

        parse_pull_request_detail(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::PullRequestDetail,
            details,
        })
    }

    pub(super) async fn fetch_issue_comments(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<IssueComment>> {
        let result = self
            .run_raw_command_with_context(
                CommandClass::IssueComments,
                vec![
                    "api".to_string(),
                    format!("repos/{repo}/issues/{number}/comments?per_page=100"),
                ],
                Some(repo.to_string()),
                Some(number),
            )
            .await?;

        parse_issue_comments(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::IssueComments,
            details,
        })
    }

    pub(super) async fn fetch_pull_request_reviews(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<PullRequestReview>> {
        let result = self
            .run_raw_command_with_context(
                CommandClass::PullRequestReviews,
                vec![
                    "api".to_string(),
                    format!("repos/{repo}/pulls/{number}/reviews?per_page=100"),
                ],
                Some(repo.to_string()),
                Some(number),
            )
            .await?;

        parse_pull_request_reviews(&result.stdout).map_err(|details| GhError::ParseFailure {
            class: CommandClass::PullRequestReviews,
            details,
        })
    }

    pub(super) async fn fetch_pull_request_review_comments(
        &self,
        repo: &str,
        number: u64,
    ) -> GhResult<Vec<PullRequestReviewComment>> {
        let result = self
            .run_raw_command_with_context(
                CommandClass::PullRequestReviewComments,
                vec![
                    "api".to_string(),
                    format!("repos/{repo}/pulls/{number}/comments?per_page=100"),
                ],
                Some(repo.to_string()),
                Some(number),
            )
            .await?;

        parse_pull_request_review_comments(&result.stdout).map_err(|details| {
            GhError::ParseFailure {
                class: CommandClass::PullRequestReviewComments,
                details,
            }
        })
    }
}

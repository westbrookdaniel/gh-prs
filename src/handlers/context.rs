use crate::gh::GhError;
use crate::gh::GhResult;
use crate::gh::models::RepoContext;
use crate::http::Request;

pub fn parse_pr_number(request: &Request) -> GhResult<u64> {
    let raw = request
        .param("number")
        .ok_or_else(|| GhError::InvalidInput {
            field: "number".to_string(),
            details: "missing route parameter".to_string(),
        })?;

    let number = raw.parse::<u64>().map_err(|_| GhError::InvalidInput {
        field: "number".to_string(),
        details: "must be a positive integer".to_string(),
    })?;

    if number == 0 {
        return Err(GhError::InvalidInput {
            field: "number".to_string(),
            details: "must be greater than zero".to_string(),
        });
    }

    Ok(number)
}

pub fn validate_repo_identifier(repo: &str) -> GhResult<String> {
    let repo = repo.trim();
    let (owner, name) = repo.split_once('/').ok_or_else(|| GhError::InvalidInput {
        field: "repo".to_string(),
        details: "expected OWNER/REPO".to_string(),
    })?;

    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return Err(GhError::InvalidInput {
            field: "repo".to_string(),
            details: "expected OWNER/REPO".to_string(),
        });
    }

    if !owner
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(GhError::InvalidInput {
            field: "repo".to_string(),
            details: "owner contains invalid characters".to_string(),
        });
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err(GhError::InvalidInput {
            field: "repo".to_string(),
            details: "repo contains invalid characters".to_string(),
        });
    }

    Ok(format!("{owner}/{name}"))
}

pub fn repo_from_request(
    request: &Request,
    fallback_repo: Option<&RepoContext>,
) -> GhResult<String> {
    if let Some(repo) = request.query_param("repo") {
        return validate_repo_identifier(repo);
    }

    let owner = request.param("owner");
    let name = request.param("repo");

    if let (Some(owner), Some(name)) = (owner, name) {
        return validate_repo_identifier(&format!("{owner}/{name}"));
    }

    if let Some(repo) = fallback_repo {
        return validate_repo_identifier(&repo.name_with_owner);
    }

    Err(GhError::InvalidInput {
        field: "repo".to_string(),
        details: "missing repo context; provide ?repo=OWNER/REPO".to_string(),
    })
}

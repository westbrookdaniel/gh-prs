use crate::gh::{GhError, GhResult};

pub const MAX_WRITE_BODY_BYTES: usize = 64 * 1024;

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

pub fn validate_pr_number(number: u64) -> GhResult<u64> {
    if number == 0 {
        return Err(GhError::InvalidInput {
            field: "number".to_string(),
            details: "must be greater than zero".to_string(),
        });
    }
    Ok(number)
}

pub fn normalize_write_body(body: &str) -> GhResult<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(GhError::InvalidInput {
            field: "body".to_string(),
            details: "body cannot be empty".to_string(),
        });
    }
    if body.len() > MAX_WRITE_BODY_BYTES {
        return Err(GhError::InvalidInput {
            field: "body".to_string(),
            details: format!("body must be <= {} bytes", MAX_WRITE_BODY_BYTES),
        });
    }
    Ok(body.to_string())
}

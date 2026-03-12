pub fn with_query(mut path: String, query: Option<&str>) -> String {
    if let Some(query) = query
        && !query.is_empty()
    {
        path.push('?');
        path.push_str(query);
    }
    path
}

pub fn detail_path(repo: &str, number: u64, query: Option<&str>) -> String {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}")
    } else {
        format!("/prs/{number}")
    };

    with_query(base, query)
}

pub fn changes_path(repo: &str, number: u64, query: Option<&str>) -> String {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}/changes")
    } else {
        format!("/prs/{number}/changes")
    };

    with_query(base, query)
}

pub fn repo_action_path(repo: &str, number: u64, action: &str, query: Option<&str>) -> String {
    let base = if let Some((owner, name)) = repo.split_once('/') {
        format!("/repos/{owner}/{name}/prs/{number}/{action}")
    } else {
        format!("/prs/{number}/{action}")
    };

    with_query(base, query)
}

pub fn list_path(query: Option<&str>) -> String {
    with_query("/prs".to_string(), query)
}

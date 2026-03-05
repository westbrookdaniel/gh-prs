use crate::gh::models::{
    DEFAULT_SEARCH_LIMIT, PullRequestOrder, PullRequestSort, PullRequestStatus,
};
use crate::http::Request;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchArgs {
    pub org: Option<String>,
    pub repo: Option<String>,
    pub status: PullRequestStatus,
    pub title: Option<String>,
    pub author: Option<String>,
    pub sort: PullRequestSort,
    pub order: PullRequestOrder,
    pub limit: usize,
    pub view: Option<String>,
}

impl Default for SearchArgs {
    fn default() -> Self {
        Self {
            org: None,
            repo: None,
            status: PullRequestStatus::All,
            title: None,
            author: None,
            sort: PullRequestSort::Updated,
            order: PullRequestOrder::Desc,
            limit: DEFAULT_SEARCH_LIMIT,
            view: None,
        }
    }
}

impl SearchArgs {
    pub fn from_request(request: &Request) -> Self {
        let mut args = Self::default();

        args.org = request.query_param("org").and_then(normalize_simple);
        args.repo = request
            .query_param("repo")
            .and_then(|value| normalize_repo(value, args.org.as_deref()));
        args.title = request.query_param("title").and_then(normalize_text);
        args.author = request.query_param("author").and_then(normalize_login);

        if let Some(status) = request
            .query_param("status")
            .and_then(PullRequestStatus::parse)
        {
            args.status = status;
        }

        if let Some(sort) = request.query_param("sort").and_then(PullRequestSort::parse) {
            args.sort = sort;
        }

        if let Some(order) = request
            .query_param("order")
            .and_then(PullRequestOrder::parse)
        {
            args.order = order;
        }

        if let Some(limit) = request
            .query_param("limit")
            .and_then(|value| value.parse::<usize>().ok())
        {
            args.limit = limit.clamp(1, DEFAULT_SEARCH_LIMIT);
        }

        args.view = request
            .query_param("view")
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "all")
            .map(str::to_string);

        args
    }

    pub fn to_query_string(&self) -> Option<String> {
        let mut encoded: Vec<(String, String)> = Vec::new();

        if let Some(org) = &self.org {
            encoded.push(("org".to_string(), org.clone()));
        }
        if let Some(repo) = &self.repo {
            encoded.push(("repo".to_string(), repo.clone()));
        }
        if self.status != PullRequestStatus::All {
            encoded.push((
                "status".to_string(),
                self.status.as_query_value().to_string(),
            ));
        }
        if let Some(title) = &self.title {
            encoded.push(("title".to_string(), title.clone()));
        }
        if let Some(author) = &self.author {
            encoded.push(("author".to_string(), author.clone()));
        }
        if self.sort != PullRequestSort::Updated {
            encoded.push(("sort".to_string(), self.sort.as_query_value().to_string()));
        }
        if self.order != PullRequestOrder::Desc {
            encoded.push(("order".to_string(), self.order.as_query_value().to_string()));
        }
        if self.limit != DEFAULT_SEARCH_LIMIT {
            encoded.push(("limit".to_string(), self.limit.to_string()));
        }
        if let Some(view) = &self.view {
            encoded.push(("view".to_string(), view.clone()));
        }

        if encoded.is_empty() {
            return None;
        }

        serde_urlencoded::to_string(encoded).ok()
    }

    pub fn with_status(&self, status: PullRequestStatus) -> Self {
        let mut cloned = self.clone();
        cloned.status = status;
        cloned
    }
}

fn normalize_text(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Some(value.chars().take(120).collect())
}

fn normalize_simple(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Some(value.to_string());
    }

    None
}

fn normalize_login(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches('@');
    normalize_simple(value)
}

fn normalize_repo(value: &str, org: Option<&str>) -> Option<String> {
    let value = value.trim();
    if let Some((owner, name)) = value.split_once('/') {
        return normalize_repo_parts(owner, name);
    }

    let owner = org?;
    normalize_repo_parts(owner, value)
}

fn normalize_repo_parts(owner: &str, name: &str) -> Option<String> {
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }

    if !owner
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return None;
    }

    Some(format!("{owner}/{name}"))
}

#[cfg(test)]
mod tests {
    use super::SearchArgs;
    use crate::gh::models::{
        DEFAULT_SEARCH_LIMIT, PullRequestOrder, PullRequestSort, PullRequestStatus,
    };
    use crate::http::Request;

    fn request(raw: &str) -> Request {
        Request::from_bytes(raw.as_bytes()).expect("request should parse")
    }

    #[test]
    fn parses_basic_query_filters() {
        let req = request(
            "GET /prs?org=westbrookdaniel&repo=blogs&status=merged&title=security&author=@alice&sort=comments&order=asc HTTP/1.1\r\nHost: localhost\r\n\r\n",
        );
        let query = SearchArgs::from_request(&req);

        assert_eq!(query.org.as_deref(), Some("westbrookdaniel"));
        assert_eq!(query.repo.as_deref(), Some("westbrookdaniel/blogs"));
        assert_eq!(query.status, PullRequestStatus::Merged);
        assert_eq!(query.title.as_deref(), Some("security"));
        assert_eq!(query.author.as_deref(), Some("alice"));
        assert_eq!(query.sort, PullRequestSort::Comments);
        assert_eq!(query.order, PullRequestOrder::Asc);
    }

    #[test]
    fn invalid_query_values_fall_back_to_defaults() {
        let req = request(
            "GET /prs?org=bad!org&repo=bad/repo/extra&status=oops&sort=nope&order=up HTTP/1.1\r\nHost: localhost\r\n\r\n",
        );
        let query = SearchArgs::from_request(&req);

        assert!(query.org.is_none());
        assert!(query.repo.is_none());
        assert_eq!(query.status, PullRequestStatus::All);
        assert_eq!(query.sort, PullRequestSort::Updated);
        assert_eq!(query.order, PullRequestOrder::Desc);
    }

    #[test]
    fn canonical_query_strips_empty_and_default_values() {
        let request = Request::from_bytes(
            b"GET /prs?org=example&repo=&status=all&title=&author=&sort=updated&order=desc&view=all&limit=100 HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .expect("request should parse");

        let query = SearchArgs::from_request(&request);
        assert_eq!(query.limit, DEFAULT_SEARCH_LIMIT);
        assert_eq!(query.to_query_string(), Some("org=example".to_string()));
    }
}

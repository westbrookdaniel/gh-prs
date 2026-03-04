pub mod builders;
pub mod helpers;
pub mod templates;
pub mod types;

pub use builders::{changes_page_model, detail_page_model, error_page_model, list_page_model};
pub use templates::{ErrorTemplate, PrChangesTemplate, PrDetailTemplate, PrListTemplate};
pub use types::FlashMessageView;

use crate::gh::models::{
    PullRequestOrder, PullRequestSearchQuery, PullRequestSort, PullRequestStatus,
};

pub fn parse_search_query(request: &crate::http::Request) -> PullRequestSearchQuery {
    let mut query = PullRequestSearchQuery::default();

    query.org = request
        .query_param("org")
        .and_then(helpers::normalize_simple);
    query.repo = request
        .query_param("repo")
        .and_then(|value| helpers::normalize_repo(value, query.org.as_deref()));
    query.title = request
        .query_param("title")
        .and_then(helpers::normalize_text);
    query.author = request
        .query_param("author")
        .and_then(helpers::normalize_login);

    if let Some(status) = request
        .query_param("status")
        .and_then(PullRequestStatus::parse)
    {
        query.status = status;
    }

    if let Some(sort) = request.query_param("sort").and_then(PullRequestSort::parse) {
        query.sort = sort;
    }

    if let Some(order) = request
        .query_param("order")
        .and_then(PullRequestOrder::parse)
    {
        query.order = order;
    }

    query
}

#[cfg(test)]
mod tests {
    use super::parse_search_query;
    use crate::gh::models::{PullRequestOrder, PullRequestSort, PullRequestStatus};
    use crate::http::Request;

    fn request(raw: &str) -> Request {
        Request::from_bytes(raw.as_bytes()).expect("request should parse")
    }

    #[test]
    fn parses_basic_query_filters() {
        let req = request(
            "GET /prs?org=westbrookdaniel&repo=blogs&status=merged&title=security&author=@alice&sort=comments&order=asc HTTP/1.1\r\nHost: localhost\r\n\r\n",
        );
        let query = parse_search_query(&req);

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
        let query = parse_search_query(&req);

        assert!(query.org.is_none());
        assert!(query.repo.is_none());
        assert_eq!(query.status, PullRequestStatus::All);
        assert_eq!(query.sort, PullRequestSort::Updated);
        assert_eq!(query.order, PullRequestOrder::Desc);
    }
}

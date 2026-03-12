use crate::http::Request;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLoadMode {
    CacheFirst,
    NoCache,
}

impl PageLoadMode {
    pub fn from_request(request: &Request) -> Self {
        if query_flag(request, "nocache") {
            Self::NoCache
        } else {
            Self::CacheFirst
        }
    }

    pub fn bypass_cache(self) -> bool {
        matches!(self, Self::NoCache)
    }
}

fn query_flag(request: &Request, name: &str) -> bool {
    request
        .query_param(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

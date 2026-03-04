use crate::http::{Request, Response};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type HandlerFuture = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;
pub type Handler = Arc<dyn Fn(Request) -> HandlerFuture + Send + Sync + 'static>;

#[derive(Default)]
pub struct Router {
    methods: HashMap<String, MethodRoutes>,
}

#[derive(Default)]
struct MethodRoutes {
    static_routes: HashMap<String, Handler>,
    param_routes: Vec<(RoutePattern, Handler)>,
    catch_all_routes: Vec<(RoutePattern, Handler)>,
}

#[derive(Clone)]
pub enum ResolveResult {
    Found {
        handler: Handler,
        params: HashMap<String, String>,
    },
    MethodNotAllowed,
    NotFound,
}

impl Router {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_route<F, Fut>(
        &mut self,
        method: impl Into<String>,
        pattern: &str,
        handler: F,
    ) -> Result<(), String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        let method = method.into().to_ascii_uppercase();
        let pattern = RoutePattern::parse(pattern)?;
        let handler: Handler = Arc::new(move |request| Box::pin(handler(request)));

        let routes = self.methods.entry(method).or_default();
        match pattern.kind {
            RouteKind::Static => {
                routes.static_routes.insert(pattern.original, handler);
            }
            RouteKind::Param => {
                routes.param_routes.push((pattern, handler));
            }
            RouteKind::CatchAll => {
                routes.catch_all_routes.push((pattern, handler));
            }
        }

        Ok(())
    }

    pub fn resolve(&self, method: &str, path: &str) -> ResolveResult {
        let method = method.to_ascii_uppercase();
        let normalized_path = normalize_path(path);

        if let Some(routes) = self.methods.get(&method) {
            if let Some(found) = Self::resolve_in_routes(routes, &normalized_path) {
                return found;
            }
        }

        if let Some(routes) = self.methods.get("ANY") {
            if let Some(found) = Self::resolve_in_routes(routes, &normalized_path) {
                return found;
            }
        }

        if self.matches_any_method(&normalized_path) {
            ResolveResult::MethodNotAllowed
        } else {
            ResolveResult::NotFound
        }
    }

    fn resolve_in_routes(routes: &MethodRoutes, path: &str) -> Option<ResolveResult> {
        if let Some(handler) = routes.static_routes.get(path) {
            return Some(ResolveResult::Found {
                handler: handler.clone(),
                params: HashMap::new(),
            });
        }

        for (pattern, handler) in &routes.param_routes {
            if let Some(params) = pattern.match_path(path) {
                return Some(ResolveResult::Found {
                    handler: handler.clone(),
                    params,
                });
            }
        }

        for (pattern, handler) in &routes.catch_all_routes {
            if let Some(params) = pattern.match_path(path) {
                return Some(ResolveResult::Found {
                    handler: handler.clone(),
                    params,
                });
            }
        }

        None
    }

    fn matches_any_method(&self, path: &str) -> bool {
        self.methods.values().any(|routes| {
            if routes.static_routes.contains_key(path) {
                return true;
            }
            routes
                .param_routes
                .iter()
                .any(|(pattern, _)| pattern.match_path(path).is_some())
                || routes
                    .catch_all_routes
                    .iter()
                    .any(|(pattern, _)| pattern.match_path(path).is_some())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteKind {
    Static,
    Param,
    CatchAll,
}

#[derive(Debug, Clone)]
struct RoutePattern {
    original: String,
    segments: Vec<Segment>,
    kind: RouteKind,
}

#[derive(Debug, Clone)]
enum Segment {
    Static(String),
    Param(String),
    CatchAll(String),
}

impl RoutePattern {
    fn parse(pattern: &str) -> Result<Self, String> {
        if !pattern.starts_with('/') {
            return Err(format!("route pattern must start with '/': {pattern}"));
        }

        let clean = pattern.trim_matches('/');
        if clean.is_empty() {
            return Ok(Self {
                original: "/".to_string(),
                segments: Vec::new(),
                kind: RouteKind::Static,
            });
        }

        let mut has_param = false;
        let mut has_catch_all = false;
        let mut segments = Vec::new();
        let parts: Vec<&str> = clean.split('/').collect();
        let total_segments = parts.len();
        for (index, part) in parts.into_iter().enumerate() {
            if part.is_empty() {
                return Err(format!(
                    "invalid route pattern with empty segment: {pattern}"
                ));
            }
            if let Some(name) = part.strip_prefix(':') {
                if name.is_empty() {
                    return Err(format!("empty param name in route: {pattern}"));
                }
                has_param = true;
                segments.push(Segment::Param(name.to_string()));
                continue;
            }

            if let Some(name) = part.strip_prefix('*') {
                if name.is_empty() {
                    return Err(format!("empty catch-all name in route: {pattern}"));
                }
                if index != total_segments - 1 {
                    return Err(format!("catch-all must be final segment: {pattern}"));
                }
                has_catch_all = true;
                segments.push(Segment::CatchAll(name.to_string()));
                continue;
            }

            segments.push(Segment::Static(part.to_string()));
        }

        let kind = if has_catch_all {
            RouteKind::CatchAll
        } else if has_param {
            RouteKind::Param
        } else {
            RouteKind::Static
        };

        Ok(Self {
            original: normalize_path(pattern),
            segments,
            kind,
        })
    }

    fn match_path(&self, path: &str) -> Option<HashMap<String, String>> {
        let parts: Vec<&str> = split_path(path);
        if self.segments.is_empty() {
            return if parts.is_empty() {
                Some(HashMap::new())
            } else {
                None
            };
        }

        let mut params = HashMap::new();
        let mut path_index = 0usize;
        for segment in &self.segments {
            match segment {
                Segment::Static(value) => {
                    if parts.get(path_index)? != &value.as_str() {
                        return None;
                    }
                    path_index += 1;
                }
                Segment::Param(name) => {
                    let value = parts.get(path_index)?;
                    params.insert(name.clone(), (*value).to_string());
                    path_index += 1;
                }
                Segment::CatchAll(name) => {
                    let rest = parts[path_index..].join("/");
                    params.insert(name.clone(), rest);
                    path_index = parts.len();
                    break;
                }
            }
        }

        if path_index == parts.len() {
            Some(params)
        } else {
            None
        }
    }
}

fn normalize_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }

    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn split_path(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|p| !p.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ResolveResult, Router};
    use crate::http::Response;
    use std::time::{Duration, Instant};

    async fn ok_handler(_request: crate::http::Request) -> Response {
        Response::ok()
    }

    #[test]
    fn resolves_static_then_param_then_catch_all() {
        let mut router = Router::new();
        router.add_route("GET", "/users/me", ok_handler).unwrap();
        router.add_route("GET", "/users/:id", ok_handler).unwrap();
        router.add_route("GET", "/users/*path", ok_handler).unwrap();

        assert!(matches!(
            router.resolve("GET", "/users/me"),
            ResolveResult::Found { params, .. } if params.is_empty()
        ));
        assert!(matches!(
            router.resolve("GET", "/users/42"),
            ResolveResult::Found { params, .. } if params.get("id").map(String::as_str) == Some("42")
        ));
        assert!(matches!(
            router.resolve("GET", "/users/a/b/c"),
            ResolveResult::Found { params, .. } if params.get("path").map(String::as_str) == Some("a/b/c")
        ));
    }

    #[test]
    fn returns_method_not_allowed_when_path_exists_for_other_method() {
        let mut router = Router::new();
        router.add_route("GET", "/users/:id", ok_handler).unwrap();

        assert!(matches!(
            router.resolve("POST", "/users/42"),
            ResolveResult::MethodNotAllowed
        ));
    }

    #[test]
    fn any_route_acts_as_catch_all_fallback() {
        let mut router = Router::new();
        router.add_route("ANY", "/*path", ok_handler).unwrap();

        assert!(matches!(
            router.resolve("GET", "/missing/page"),
            ResolveResult::Found { params, .. } if params.get("path").map(String::as_str) == Some("missing/page")
        ));
        assert!(matches!(
            router.resolve("POST", "/also/missing"),
            ResolveResult::Found { params, .. } if params.get("path").map(String::as_str) == Some("also/missing")
        ));
    }

    #[test]
    fn sanity_performance_route_resolution() {
        let mut router = Router::new();
        router.add_route("GET", "/", ok_handler).unwrap();
        router.add_route("GET", "/users/:id", ok_handler).unwrap();
        router
            .add_route("GET", "/assets/*path", ok_handler)
            .unwrap();

        let start = Instant::now();
        for _ in 0..100_000 {
            let _ = router.resolve("GET", "/users/12345");
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(5),
            "route resolution sanity check too slow: {elapsed:?}"
        );
    }
}

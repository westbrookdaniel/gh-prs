use crate::http::{Request, Response};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type HandlerFuture = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;
pub type Handler = Arc<dyn Fn(Request) -> HandlerFuture + Send + Sync + 'static>;

#[derive(Default)]
pub struct Router {
    methods: HashMap<String, MethodTree>,
}

#[derive(Default)]
struct MethodTree {
    root: Node,
}

#[derive(Default)]
struct Node {
    handler: Option<Handler>,
    static_children: HashMap<String, Node>,
    param_child: Option<ParamNode>,
    catch_all_child: Option<CatchAllNode>,
}

struct ParamNode {
    name: String,
    node: Box<Node>,
}

struct CatchAllNode {
    name: String,
    handler: Handler,
}

#[derive(Clone)]
pub enum ResolveResult {
    Found {
        handler: Handler,
        params: HashMap<String, String>,
    },
    MethodNotAllowed {
        allow: Vec<String>,
    },
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

        let tree = self.methods.entry(method).or_default();
        tree.insert_route(&pattern, handler)
    }

    pub fn resolve(&self, method: &str, path: &str) -> ResolveResult {
        let method = method.to_ascii_uppercase();
        let normalized_path = normalize_path(path);
        let parts = split_path(&normalized_path);

        if let Some(tree) = self.methods.get(&method) {
            if let Some(found) = tree.resolve(parts.as_slice()) {
                return found;
            }
        }

        if method != "ANY" {
            if let Some(tree) = self.methods.get("ANY") {
                if let Some(found) = tree.resolve(parts.as_slice()) {
                    return found;
                }
            }
        }

        let mut allow = self.allowed_methods_for_parts(parts.as_slice());
        allow.retain(|registered_method| registered_method.as_str() != method.as_str());

        if allow.is_empty() {
            ResolveResult::NotFound
        } else {
            ResolveResult::MethodNotAllowed { allow }
        }
    }

    pub fn allowed_methods(&self, path: &str) -> Vec<String> {
        let normalized_path = normalize_path(path);
        let parts = split_path(&normalized_path);

        self.allowed_methods_for_parts(parts.as_slice())
    }

    fn allowed_methods_for_parts(&self, parts: &[&str]) -> Vec<String> {
        let mut allow = Vec::new();

        for (method, tree) in &self.methods {
            if method == "ANY" {
                continue;
            }
            if tree.matches_path(parts) {
                allow.push(method.clone());
            }
        }

        if self
            .methods
            .get("ANY")
            .is_some_and(|tree| tree.matches_path(parts))
        {
            allow.extend(
                ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
                    .into_iter()
                    .map(str::to_string),
            );
        }

        allow.sort_unstable();
        allow.dedup();
        allow
    }
}

impl MethodTree {
    fn insert_route(&mut self, pattern: &RoutePattern, handler: Handler) -> Result<(), String> {
        let mut current = &mut self.root;
        for (index, segment) in pattern.segments.iter().enumerate() {
            match segment {
                Segment::Static(value) => {
                    current = current.static_children.entry(value.clone()).or_default();
                }
                Segment::Param(name) => {
                    if current.param_child.is_none() {
                        current.param_child = Some(ParamNode {
                            name: name.clone(),
                            node: Box::new(Node::default()),
                        });
                    }

                    let param = current
                        .param_child
                        .as_mut()
                        .expect("param child must exist");
                    if param.name != *name {
                        return Err(format!(
                            "ambiguous param route at segment {}: '{}' conflicts with '{}'",
                            index + 1,
                            name,
                            param.name
                        ));
                    }

                    current = param.node.as_mut();
                }
                Segment::CatchAll(name) => {
                    if index != pattern.segments.len() - 1 {
                        return Err(format!(
                            "catch-all must be the final segment: {}",
                            pattern.original
                        ));
                    }

                    current.catch_all_child = Some(CatchAllNode {
                        name: name.clone(),
                        handler: Arc::clone(&handler),
                    });
                    return Ok(());
                }
            }
        }

        current.handler = Some(handler);
        Ok(())
    }

    fn resolve(&self, parts: &[&str]) -> Option<ResolveResult> {
        let mut params = HashMap::new();
        resolve_node(&self.root, parts, 0, &mut params)
    }

    fn matches_path(&self, parts: &[&str]) -> bool {
        matches_node(&self.root, parts, 0)
    }
}

fn resolve_node(
    node: &Node,
    parts: &[&str],
    index: usize,
    params: &mut HashMap<String, String>,
) -> Option<ResolveResult> {
    if index == parts.len() {
        if let Some(handler) = &node.handler {
            return Some(ResolveResult::Found {
                handler: Arc::clone(handler),
                params: params.clone(),
            });
        }

        if let Some(catch_all) = &node.catch_all_child {
            params.insert(catch_all.name.clone(), String::new());
            return Some(ResolveResult::Found {
                handler: Arc::clone(&catch_all.handler),
                params: params.clone(),
            });
        }

        return None;
    }

    let segment = parts[index];

    if let Some(child) = node.static_children.get(segment) {
        if let Some(found) = resolve_node(child, parts, index + 1, params) {
            return Some(found);
        }
    }

    if let Some(param) = &node.param_child {
        params.insert(param.name.clone(), segment.to_string());
        if let Some(found) = resolve_node(param.node.as_ref(), parts, index + 1, params) {
            return Some(found);
        }
        params.remove(&param.name);
    }

    if let Some(catch_all) = &node.catch_all_child {
        params.insert(catch_all.name.clone(), parts[index..].join("/"));
        return Some(ResolveResult::Found {
            handler: Arc::clone(&catch_all.handler),
            params: params.clone(),
        });
    }

    None
}

fn matches_node(node: &Node, parts: &[&str], index: usize) -> bool {
    if index == parts.len() {
        return node.handler.is_some() || node.catch_all_child.is_some();
    }

    let segment = parts[index];
    if let Some(child) = node.static_children.get(segment) {
        if matches_node(child, parts, index + 1) {
            return true;
        }
    }

    if let Some(param) = &node.param_child {
        if matches_node(param.node.as_ref(), parts, index + 1) {
            return true;
        }
    }

    node.catch_all_child.is_some()
}

#[derive(Debug, Clone)]
struct RoutePattern {
    original: String,
    segments: Vec<Segment>,
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
            });
        }

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
                segments.push(Segment::CatchAll(name.to_string()));
                continue;
            }

            segments.push(Segment::Static(part.to_string()));
        }

        Ok(Self {
            original: normalize_path(pattern),
            segments,
        })
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
        .filter(|part| !part.is_empty())
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
            ResolveResult::MethodNotAllowed { allow } if allow == vec!["GET"]
        ));
    }

    #[test]
    fn reports_allowed_methods_for_path() {
        let mut router = Router::new();
        router.add_route("GET", "/users/:id", ok_handler).unwrap();
        router.add_route("PATCH", "/users/:id", ok_handler).unwrap();

        let allow = router.allowed_methods("/users/42");
        assert_eq!(allow, vec!["GET".to_string(), "PATCH".to_string()]);
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
    fn detects_ambiguous_param_names() {
        let mut router = Router::new();
        router.add_route("GET", "/users/:id", ok_handler).unwrap();
        let result = router.add_route("GET", "/users/:name", ok_handler);

        assert!(result.is_err());
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
            elapsed < Duration::from_secs(3),
            "route resolution sanity check too slow: {elapsed:?}"
        );
    }
}

use crate::http::middleware::{MiddlewareFuture, Next};
use crate::http::{Request, Response};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct StaticDirOptions {
    pub url_prefix: String,
    pub root: PathBuf,
    pub index_file: Option<String>,
    pub cache_control: Option<String>,
    pub etag: bool,
    pub memory_cache: bool,
    pub cache_ttl: Duration,
    pub cache_max_entries: usize,
    pub cache_max_bytes: usize,
    pub fallthrough: bool,
    pub allow_dotfiles: bool,
}

impl Default for StaticDirOptions {
    fn default() -> Self {
        Self {
            url_prefix: "/".to_string(),
            root: PathBuf::from("."),
            index_file: Some("index.html".to_string()),
            cache_control: Some("public, max-age=300".to_string()),
            etag: true,
            memory_cache: true,
            cache_ttl: Duration::from_secs(60),
            cache_max_entries: 256,
            cache_max_bytes: 16 * 1024 * 1024,
            fallthrough: true,
            allow_dotfiles: false,
        }
    }
}

pub fn static_dir(
    options: StaticDirOptions,
) -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    let normalized_prefix = normalize_prefix(&options.url_prefix);
    let canonical_root = std::fs::canonicalize(&options.root).ok();
    let cache = Arc::new(Mutex::new(StaticFileCache::new(
        options.cache_max_entries,
        options.cache_max_bytes,
    )));

    move |request: Request, next: Next| {
        let normalized_prefix = normalized_prefix.clone();
        let canonical_root = canonical_root.clone();
        let cache = Arc::clone(&cache);
        let options = options.clone();

        Box::pin(async move {
            if request.method != "GET" && request.method != "HEAD" {
                return next.run(request).await;
            }

            let Some(relative_raw) = path_under_prefix(&request.path, &normalized_prefix) else {
                return next.run(request).await;
            };

            let Some(canonical_root) = canonical_root else {
                return if options.fallthrough {
                    next.run(request).await
                } else {
                    Response::internal_server_error().text_body("Static root unavailable")
                };
            };

            let mut relative_path =
                match sanitize_relative_path(&relative_raw, options.allow_dotfiles) {
                    Ok(path) => path,
                    Err(StaticPathError::InvalidPath) => {
                        return Response::bad_request().text_body("Invalid asset path");
                    }
                    Err(StaticPathError::DotfileDenied) => {
                        return if options.fallthrough {
                            next.run(request).await
                        } else {
                            Response::not_found().text_body("Not Found")
                        };
                    }
                };

            if request.path.ends_with('/') || relative_path.as_os_str().is_empty() {
                if let Some(index_file) = &options.index_file {
                    relative_path.push(index_file);
                }
            }

            if relative_path.as_os_str().is_empty() {
                return if options.fallthrough {
                    next.run(request).await
                } else {
                    Response::not_found().text_body("Not Found")
                };
            }

            let cache_key = relative_path.to_string_lossy().into_owned();
            let mut entry = if options.memory_cache {
                cache
                    .lock()
                    .ok()
                    .and_then(|mut state| state.get_fresh(&cache_key, options.cache_ttl))
            } else {
                None
            };

            if entry.is_none() {
                let target_path = canonical_root.join(&relative_path);
                let canonical_target = match canonicalize_path(target_path).await {
                    Ok(path) => path,
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {
                        return if options.fallthrough {
                            next.run(request).await
                        } else {
                            Response::not_found().text_body("Not Found")
                        };
                    }
                    Err(_) => {
                        return Response::internal_server_error()
                            .text_body("Failed to read static file");
                    }
                };

                if !canonical_target.starts_with(&canonical_root) {
                    return Response::bad_request().text_body("Invalid asset path");
                }

                let loaded = match load_file_entry(canonical_target).await {
                    Ok(loaded) => loaded,
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {
                        return if options.fallthrough {
                            next.run(request).await
                        } else {
                            Response::not_found().text_body("Not Found")
                        };
                    }
                    Err(_) => {
                        return Response::internal_server_error()
                            .text_body("Failed to read static file");
                    }
                };

                if options.memory_cache {
                    if let Ok(mut state) = cache.lock() {
                        state.insert(cache_key.clone(), loaded.clone());
                    }
                }

                entry = Some(loaded);
            }

            let entry = entry.expect("static file entry should exist after load");

            if options.etag {
                if let Some(if_none_match) = request.header("if-none-match") {
                    if request_matches_etag(if_none_match, &entry.etag) {
                        let mut response =
                            Response::new(304, "Not Modified").header("ETag", entry.etag.clone());
                        if let Some(cache_control) = &options.cache_control {
                            response = response.header("Cache-Control", cache_control);
                        }
                        if request.method == "HEAD" {
                            response = response.into_head_response();
                        }
                        return response;
                    }
                }
            }

            let mut response = Response::ok()
                .header("Content-Type", entry.content_type)
                .body(entry.bytes.as_ref().clone());

            if options.etag {
                response = response.header("ETag", entry.etag.clone());
            }

            if let Some(cache_control) = &options.cache_control {
                response = response.header("Cache-Control", cache_control);
            }

            if request.method == "HEAD" {
                response = response.into_head_response();
            }

            response
        })
    }
}

pub fn logger() -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    |request: Request, next: Next| {
        Box::pin(async move {
            let method = request.method.clone();
            let path = request.path.clone();
            let request_id = request
                .header("x-request-id")
                .unwrap_or("missing")
                .to_string();
            let started = Instant::now();

            let response = next.run(request).await;
            let elapsed_ms = started.elapsed().as_millis();

            println!(
                "[request] id={request_id} {method} {path} -> {} ({elapsed_ms}ms)",
                response.status_code()
            );

            response
        })
    }
}

pub fn request_id() -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    |request: Request, next: Next| {
        Box::pin(async move {
            let request_id = request
                .header("x-request-id")
                .filter(|value| {
                    !value.trim().is_empty()
                        && value.len() <= 128
                        && !value.contains('\r')
                        && !value.contains('\n')
                })
                .map(str::to_owned)
                .unwrap_or_else(generate_request_id);

            let mut request = request;
            request
                .headers
                .insert("x-request-id".to_string(), request_id.clone());

            next.run(request).await.header("X-Request-Id", request_id)
        })
    }
}

fn generate_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("req-{nanos:x}-{sequence:x}")
}

pub fn security_headers() -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    |request: Request, next: Next| {
        Box::pin(async move {
            next.run(request)
                .await
                .header_if_missing("X-Frame-Options", "DENY")
                .header_if_missing("Referrer-Policy", "strict-origin-when-cross-origin")
                .header_if_missing(
                    "Permissions-Policy",
                    "geolocation=(), microphone=(), camera=()",
                )
                .header_if_missing("Cross-Origin-Resource-Policy", "same-origin")
        })
    }
}

pub fn cors(
    allow_origin: &'static str,
    allow_methods: &'static str,
    allow_headers: &'static str,
) -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    move |request: Request, next: Next| {
        Box::pin(async move {
            let mut response = next
                .run(request)
                .await
                .header_if_missing("Access-Control-Allow-Origin", allow_origin)
                .header_if_missing("Access-Control-Allow-Methods", allow_methods)
                .header_if_missing("Access-Control-Allow-Headers", allow_headers)
                .header_if_missing("Vary", "Origin");

            if response.status_code() == 204 {
                response = response.header_if_missing("Access-Control-Max-Age", "600");
            }

            response
        })
    }
}

pub fn rate_limit(
    max_requests: u32,
    window: std::time::Duration,
) -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    let state = Arc::new(std::sync::Mutex::new(HashMap::<
        String,
        (u32, std::time::Instant),
    >::new()));

    move |request: Request, next: Next| {
        let state = Arc::clone(&state);
        Box::pin(async move {
            let key = request
                .header("x-forwarded-for")
                .and_then(|value| value.split(',').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown")
                .to_string();

            let now = std::time::Instant::now();
            let mut limited = false;
            let mut retry_after = 1u64;

            if let Ok(mut buckets) = state.lock() {
                let entry = buckets.entry(key).or_insert((0, now));
                if now.duration_since(entry.1) >= window {
                    *entry = (0, now);
                }

                entry.0 = entry.0.saturating_add(1);
                if entry.0 > max_requests {
                    limited = true;
                    retry_after = window
                        .saturating_sub(now.duration_since(entry.1))
                        .as_secs()
                        .max(1);
                }

                if buckets.len() > 4096 {
                    buckets.retain(|_, (_, started)| now.duration_since(*started) < window);
                }
            }

            if limited {
                return Response::new(429, "Too Many Requests")
                    .header("Retry-After", retry_after.to_string())
                    .text_body("Too Many Requests");
            }

            next.run(request).await
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum StaticPathError {
    InvalidPath,
    DotfileDenied,
}

#[derive(Clone)]
struct StaticFileEntry {
    bytes: Arc<Vec<u8>>,
    content_type: &'static str,
    etag: String,
    inserted_at: Instant,
    size: usize,
}

struct StaticFileCache {
    entries: HashMap<String, StaticFileEntry>,
    order: VecDeque<String>,
    max_entries: usize,
    max_bytes: usize,
    total_bytes: usize,
}

impl StaticFileCache {
    fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
            max_bytes: max_bytes.max(1024),
            total_bytes: 0,
        }
    }

    fn get_fresh(&mut self, key: &str, ttl: Duration) -> Option<StaticFileEntry> {
        let Some(entry) = self.entries.get(key).cloned() else {
            return None;
        };

        if entry.inserted_at.elapsed() > ttl {
            self.remove(key);
            return None;
        }

        self.touch(key);
        Some(entry)
    }

    fn insert(&mut self, key: String, entry: StaticFileEntry) {
        if let Some(existing) = self.entries.get(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.size);
        }

        self.entries.insert(key.clone(), entry.clone());
        self.total_bytes = self.total_bytes.saturating_add(entry.size);
        self.touch(&key);
        self.evict_if_needed();
    }

    fn remove(&mut self, key: &str) {
        if let Some(entry) = self.entries.remove(key) {
            self.total_bytes = self.total_bytes.saturating_sub(entry.size);
        }
        self.order.retain(|candidate| candidate != key);
    }

    fn touch(&mut self, key: &str) {
        self.order.retain(|candidate| candidate != key);
        self.order.push_back(key.to_string());
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.max_entries || self.total_bytes > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.remove(&oldest);
        }
    }
}

fn normalize_prefix(prefix: &str) -> String {
    if prefix == "/" {
        return "/".to_string();
    }

    let trimmed = prefix.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

fn path_under_prefix(path: &str, prefix: &str) -> Option<String> {
    if prefix == "/" {
        return Some(path.trim_start_matches('/').to_string());
    }

    if path == prefix {
        return Some(String::new());
    }

    let boundary = format!("{prefix}/");
    path.strip_prefix(&boundary)
        .map(|value| value.trim_start_matches('/').to_string())
}

fn sanitize_relative_path(path: &str, allow_dotfiles: bool) -> Result<PathBuf, StaticPathError> {
    let mut sanitized = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => {
                let part_str = part.to_string_lossy();
                if !allow_dotfiles && part_str.starts_with('.') {
                    return Err(StaticPathError::DotfileDenied);
                }
                sanitized.push(part);
            }
            Component::CurDir => continue,
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err(StaticPathError::InvalidPath);
            }
        }
    }

    Ok(sanitized)
}

async fn canonicalize_path(path: PathBuf) -> io::Result<PathBuf> {
    smol::unblock(move || std::fs::canonicalize(path)).await
}

async fn read_file_bytes(path: PathBuf) -> io::Result<Vec<u8>> {
    smol::unblock(move || std::fs::read(path)).await
}

async fn file_modified_secs(path: PathBuf) -> io::Result<u64> {
    smol::unblock(move || {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;
        let secs = modified
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        Ok(secs)
    })
    .await
}

async fn load_file_entry(path: PathBuf) -> io::Result<StaticFileEntry> {
    let bytes = read_file_bytes(path.clone()).await?;
    let modified_secs = file_modified_secs(path.clone()).await.unwrap_or(0);
    let etag = build_etag(&path, bytes.len(), modified_secs);
    let size = bytes.len();

    Ok(StaticFileEntry {
        bytes: Arc::new(bytes),
        content_type: content_type_for_path(&path),
        etag,
        inserted_at: Instant::now(),
        size,
    })
}

fn build_etag(path: &Path, len: usize, modified_secs: u64) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    len.hash(&mut hasher);
    modified_secs.hash(&mut hasher);
    let digest = hasher.finish();
    format!("\"{digest:x}-{len:x}-{modified_secs:x}\"")
}

fn request_matches_etag(if_none_match: &str, etag: &str) -> bool {
    if if_none_match.trim() == "*" {
        return true;
    }

    if_none_match
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == etag)
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::{StaticDirOptions, cors, rate_limit, request_id, security_headers, static_dir};
    use crate::http::middleware::{MiddlewareFn, Next, dispatch};
    use crate::http::router::Handler;
    use crate::http::{Request, Response};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_request(path: &str) -> Request {
        let raw = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        Request::from_bytes(raw.as_bytes()).expect("request should parse")
    }

    fn test_request_with_method(method: &str, path: &str) -> Request {
        let raw = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        Request::from_bytes(raw.as_bytes()).expect("request should parse")
    }

    fn test_request_with_headers(method: &str, path: &str, headers: &[(&str, &str)]) -> Request {
        let mut raw = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n");
        for (name, value) in headers {
            raw.push_str(name);
            raw.push_str(": ");
            raw.push_str(value);
            raw.push_str("\r\n");
        }
        raw.push_str("\r\n");
        Request::from_bytes(raw.as_bytes()).expect("request should parse")
    }

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&path).expect("temp dir should be created");
        path
    }

    #[test]
    fn middleware_runs_in_expected_order() {
        smol::block_on(async {
            let trace = Arc::new(Mutex::new(Vec::<String>::new()));

            let mw1_trace = Arc::clone(&trace);
            let mw1: MiddlewareFn = Arc::new(move |request: Request, next: Next| {
                let mw1_trace = Arc::clone(&mw1_trace);
                Box::pin(async move {
                    mw1_trace
                        .lock()
                        .expect("trace lock")
                        .push("mw1-before".to_string());
                    let response = next.run(request).await;
                    mw1_trace
                        .lock()
                        .expect("trace lock")
                        .push("mw1-after".to_string());
                    response
                })
            });

            let mw2_trace = Arc::clone(&trace);
            let mw2: MiddlewareFn = Arc::new(move |request: Request, next: Next| {
                let mw2_trace = Arc::clone(&mw2_trace);
                Box::pin(async move {
                    mw2_trace
                        .lock()
                        .expect("trace lock")
                        .push("mw2-before".to_string());
                    let response = next.run(request).await;
                    mw2_trace
                        .lock()
                        .expect("trace lock")
                        .push("mw2-after".to_string());
                    response
                })
            });

            let handler_trace = Arc::clone(&trace);
            let endpoint: Handler = Arc::new(move |_request: Request| {
                let handler_trace = Arc::clone(&handler_trace);
                Box::pin(async move {
                    handler_trace
                        .lock()
                        .expect("trace lock")
                        .push("handler".to_string());
                    Response::ok()
                })
            });

            let response = dispatch(
                0,
                test_request("/hello"),
                Arc::new(vec![mw1, mw2]),
                endpoint,
            )
            .await;

            assert_eq!(response.status_code(), 200);
            let actual = trace.lock().expect("trace lock").clone();
            let expected = vec![
                "mw1-before".to_string(),
                "mw2-before".to_string(),
                "handler".to_string(),
                "mw2-after".to_string(),
                "mw1-after".to_string(),
            ];
            assert_eq!(actual, expected);
        });
    }

    #[test]
    fn middleware_can_short_circuit_without_calling_next() {
        smol::block_on(async {
            let handler_called = Arc::new(Mutex::new(false));

            let handler_called_in_handler = Arc::clone(&handler_called);
            let endpoint: Handler = Arc::new(move |_request: Request| {
                let handler_called_in_handler = Arc::clone(&handler_called_in_handler);
                Box::pin(async move {
                    *handler_called_in_handler.lock().expect("flag lock") = true;
                    Response::ok()
                })
            });

            let blocker: MiddlewareFn = Arc::new(move |_request: Request, _next: Next| {
                Box::pin(async move { Response::new(401, "Unauthorized").text_body("blocked") })
            });

            let response = dispatch(
                0,
                test_request("/private"),
                Arc::new(vec![blocker]),
                endpoint,
            )
            .await;

            assert_eq!(response.status_code(), 401);
            assert!(!*handler_called.lock().expect("flag lock"));
        });
    }

    #[test]
    fn request_id_is_added_when_missing() {
        smol::block_on(async {
            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::ok() }));
            let middleware: MiddlewareFn = Arc::new(request_id());

            let response =
                dispatch(0, test_request("/"), Arc::new(vec![middleware]), endpoint).await;
            let raw =
                String::from_utf8(response.to_http_bytes()).expect("response should be utf-8");

            assert!(raw.contains("X-Request-Id: req-"));
        });
    }

    #[test]
    fn security_headers_are_added() {
        smol::block_on(async {
            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::ok() }));
            let middleware: MiddlewareFn = Arc::new(security_headers());

            let response =
                dispatch(0, test_request("/"), Arc::new(vec![middleware]), endpoint).await;
            let raw =
                String::from_utf8(response.to_http_bytes()).expect("response should be utf-8");

            assert!(raw.contains("X-Frame-Options: DENY"));
            assert!(raw.contains("Referrer-Policy: strict-origin-when-cross-origin"));
        });
    }

    #[test]
    fn cors_headers_are_added() {
        smol::block_on(async {
            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::no_content() }));
            let middleware: MiddlewareFn = Arc::new(cors("*", "GET, POST", "Content-Type"));

            let response =
                dispatch(0, test_request("/"), Arc::new(vec![middleware]), endpoint).await;
            let raw =
                String::from_utf8(response.to_http_bytes()).expect("response should be utf-8");

            assert!(raw.contains("Access-Control-Allow-Origin: *"));
            assert!(raw.contains("Access-Control-Max-Age: 600"));
        });
    }

    #[test]
    fn rate_limit_returns_429_after_threshold() {
        smol::block_on(async {
            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::ok() }));
            let middleware: MiddlewareFn =
                Arc::new(rate_limit(1, std::time::Duration::from_secs(60)));

            let first = dispatch(
                0,
                test_request("/"),
                Arc::new(vec![Arc::clone(&middleware)]),
                Arc::clone(&endpoint),
            )
            .await;
            let second = dispatch(0, test_request("/"), Arc::new(vec![middleware]), endpoint).await;

            assert_eq!(first.status_code(), 200);
            assert_eq!(second.status_code(), 429);
        });
    }

    #[test]
    fn static_dir_serves_file_with_cache_header() {
        smol::block_on(async {
            let root = temp_dir("static-dir");
            fs::write(root.join("app.css"), "body{}").expect("fixture file should be written");

            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::not_found() }));
            let middleware: MiddlewareFn = Arc::new(static_dir(StaticDirOptions {
                url_prefix: "/assets".to_string(),
                root: root.clone(),
                ..StaticDirOptions::default()
            }));

            let response = dispatch(
                0,
                test_request("/assets/app.css"),
                Arc::new(vec![middleware]),
                endpoint,
            )
            .await;
            let raw =
                String::from_utf8(response.to_http_bytes()).expect("response should be utf-8");

            assert_eq!(response.status_code(), 200);
            assert!(raw.contains("Content-Type: text/css; charset=utf-8"));
            assert!(raw.contains("Cache-Control: public, max-age=300"));

            fs::remove_dir_all(root).expect("temp dir cleanup should succeed");
        });
    }

    #[test]
    fn static_dir_denies_dotfiles_by_default() {
        smol::block_on(async {
            let root = temp_dir("static-dotfiles");
            fs::write(root.join(".env"), "SECRET=1").expect("fixture file should be written");

            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::not_found() }));
            let middleware: MiddlewareFn = Arc::new(static_dir(StaticDirOptions {
                url_prefix: "/assets".to_string(),
                root: root.clone(),
                ..StaticDirOptions::default()
            }));

            let response = dispatch(
                0,
                test_request("/assets/.env"),
                Arc::new(vec![middleware]),
                endpoint,
            )
            .await;

            assert_eq!(response.status_code(), 404);
            fs::remove_dir_all(root).expect("temp dir cleanup should succeed");
        });
    }

    #[test]
    fn static_dir_can_serve_head_without_body() {
        smol::block_on(async {
            let root = temp_dir("static-head");
            fs::write(root.join("hello.txt"), "hello").expect("fixture file should be written");

            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::not_found() }));
            let middleware: MiddlewareFn = Arc::new(static_dir(StaticDirOptions {
                url_prefix: "/assets".to_string(),
                root: root.clone(),
                ..StaticDirOptions::default()
            }));

            let response = dispatch(
                0,
                test_request_with_method("HEAD", "/assets/hello.txt"),
                Arc::new(vec![middleware]),
                endpoint,
            )
            .await;
            let raw =
                String::from_utf8(response.to_http_bytes()).expect("response should be utf-8");

            assert_eq!(response.status_code(), 200);
            assert!(raw.contains("Content-Length: 5\r\n"));
            assert!(!raw.ends_with("hello"));

            fs::remove_dir_all(root).expect("temp dir cleanup should succeed");
        });
    }

    #[test]
    fn static_dir_returns_not_modified_when_etag_matches() {
        smol::block_on(async {
            let root = temp_dir("static-etag");
            fs::write(root.join("etag.txt"), "etag-content")
                .expect("fixture file should be written");

            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::not_found() }));
            let middleware: MiddlewareFn = Arc::new(static_dir(StaticDirOptions {
                url_prefix: "/assets".to_string(),
                root: root.clone(),
                cache_ttl: std::time::Duration::from_secs(30),
                ..StaticDirOptions::default()
            }));
            let stack = Arc::new(vec![middleware]);

            let first = dispatch(
                0,
                test_request("/assets/etag.txt"),
                Arc::clone(&stack),
                Arc::clone(&endpoint),
            )
            .await;
            let first_raw =
                String::from_utf8(first.to_http_bytes()).expect("response should be utf-8");
            let etag = first_raw
                .lines()
                .find_map(|line| line.strip_prefix("ETag: "))
                .expect("etag header should exist")
                .to_string();

            let second = dispatch(
                0,
                test_request_with_headers("GET", "/assets/etag.txt", &[("If-None-Match", &etag)]),
                stack,
                endpoint,
            )
            .await;
            let second_raw =
                String::from_utf8(second.to_http_bytes()).expect("response should be utf-8");

            assert_eq!(first.status_code(), 200);
            assert_eq!(second.status_code(), 304);
            assert!(!second_raw.ends_with("etag-content"));

            fs::remove_dir_all(root).expect("temp dir cleanup should succeed");
        });
    }

    #[test]
    fn static_dir_options_object_can_build_middleware() {
        smol::block_on(async {
            let root = temp_dir("static-builder");
            fs::write(root.join("builder.txt"), "ok").expect("fixture file should be written");

            let endpoint: Handler =
                Arc::new(move |_request: Request| Box::pin(async { Response::not_found() }));
            let middleware: MiddlewareFn = Arc::new(static_dir(StaticDirOptions {
                url_prefix: "/assets".to_string(),
                root: root.clone(),
                index_file: None,
                cache_control: Some("public, max-age=120".to_string()),
                etag: true,
                memory_cache: true,
                cache_ttl: std::time::Duration::from_secs(30),
                cache_max_entries: 10,
                cache_max_bytes: 1024 * 1024,
                fallthrough: true,
                allow_dotfiles: false,
            }));

            let response = dispatch(
                0,
                test_request("/assets/builder.txt"),
                Arc::new(vec![middleware]),
                endpoint,
            )
            .await;

            assert_eq!(response.status_code(), 200);
            fs::remove_dir_all(root).expect("temp dir cleanup should succeed");
        });
    }
}

use crate::http::router::Handler;
use crate::http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub type MiddlewareFuture = Pin<Box<dyn Future<Output = Response> + Send + 'static>>;
pub type MiddlewareFn = Arc<dyn Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static>;

#[derive(Clone)]
pub struct Next {
    index: usize,
    middlewares: Arc<Vec<MiddlewareFn>>,
    endpoint: Handler,
}

impl Next {
    pub async fn run(self, request: Request) -> Response {
        dispatch(self.index, request, self.middlewares, self.endpoint).await
    }
}

pub(crate) fn dispatch(
    index: usize,
    request: Request,
    middlewares: Arc<Vec<MiddlewareFn>>,
    endpoint: Handler,
) -> MiddlewareFuture {
    if let Some(middleware) = middlewares.get(index) {
        let next = Next {
            index: index + 1,
            middlewares: Arc::clone(&middlewares),
            endpoint: Arc::clone(&endpoint),
        };
        return middleware(request, next);
    }

    endpoint(request)
}

pub fn logger() -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    |request: Request, next: Next| {
        Box::pin(async move {
            let method = request.method.clone();
            let path = request.path.clone();
            let started = Instant::now();

            let response = next.run(request).await;
            let elapsed_ms = started.elapsed().as_millis();

            println!(
                "[request] {method} {path} -> {} ({elapsed_ms}ms)",
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
    let state = Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
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

#[cfg(test)]
mod tests {
    use super::{MiddlewareFn, Next, cors, dispatch, rate_limit, request_id, security_headers};
    use crate::http::router::Handler;
    use crate::http::{Request, Response};
    use std::sync::{Arc, Mutex};

    fn test_request(path: &str) -> Request {
        let raw = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n");
        Request::from_bytes(raw.as_bytes()).expect("request should parse")
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
}

use crate::http::router::Handler;
use crate::http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

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

#[cfg(test)]
mod tests {
    use super::{MiddlewareFn, Next, dispatch};
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
}

use crate::http::router::Handler;
use crate::http::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

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

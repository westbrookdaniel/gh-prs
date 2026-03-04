use crate::http::middleware::{MiddlewareFn, Next, dispatch};
use crate::http::router::{Handler, ResolveResult, Router};
use crate::http::{Request, Response};
use async_net::{TcpListener, TcpStream};
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use std::io;
use std::sync::Arc;

const MAX_REQUEST_SIZE: usize = 1024 * 1024;

pub struct App {
    router: Router,
    middlewares: Vec<MiddlewareFn>,
}

impl App {
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            middlewares: Vec::new(),
        }
    }

    pub fn get<F, Fut>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.router
            .add_route("GET", pattern, handler)
            .expect("invalid GET route pattern");
        self
    }

    pub fn post<F, Fut>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.router
            .add_route("POST", pattern, handler)
            .expect("invalid POST route pattern");
        self
    }

    pub fn any<F, Fut>(mut self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.router
            .add_route("ANY", pattern, handler)
            .expect("invalid ANY route pattern");
        self
    }

    pub fn middleware<F, Fut>(mut self, middleware: F) -> Self
    where
        F: Fn(Request, Next) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.middlewares.push(Arc::new(move |request, next| {
            Box::pin(middleware(request, next))
        }));
        self
    }

    pub async fn serve(self, address: &str) -> io::Result<()> {
        let listener = TcpListener::bind(address).await?;
        let router = Arc::new(self.router);
        let middlewares = Arc::new(self.middlewares);
        println!("[startup] listening on http://{address}");

        loop {
            let (stream, _) = listener.accept().await?;
            let router = Arc::clone(&router);
            let middlewares = Arc::clone(&middlewares);
            smol::spawn(async move {
                if let Err(err) = handle_connection(stream, router, middlewares).await {
                    eprintln!("connection error: {err}");
                }
            })
            .detach();
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    router: Arc<Router>,
    middlewares: Arc<Vec<MiddlewareFn>>,
) -> io::Result<()> {
    let bytes = read_request_bytes(&mut stream).await?;
    let request = match Request::from_bytes(&bytes) {
        Ok(request) => request,
        Err(_) => {
            write_response(
                &mut stream,
                Response::bad_request().text_body("Bad Request"),
            )
            .await?;
            return Ok(());
        }
    };

    let method = request.method.clone();
    let path = request.path.clone();
    let (request, endpoint): (Request, Handler) = match router.resolve(&method, &path) {
        ResolveResult::Found { handler, params } => (request.with_params(params), handler),
        ResolveResult::MethodNotAllowed => (
            request,
            Arc::new(|_request: Request| {
                Box::pin(async { Response::method_not_allowed().text_body("Method Not Allowed") })
            }),
        ),
        ResolveResult::NotFound => (
            request,
            Arc::new(|_request: Request| {
                Box::pin(async { Response::not_found().text_body("Not Found") })
            }),
        ),
    };

    let response = dispatch(0, request, middlewares, endpoint).await;

    write_response(&mut stream, response).await
}

async fn write_response(stream: &mut TcpStream, response: Response) -> io::Result<()> {
    stream.write_all(&response.to_http_bytes()).await?;
    stream.flush().await
}

async fn read_request_bytes(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut buffer = vec![0u8; 4096];
    let mut bytes = Vec::with_capacity(4096);
    let mut expected_len: Option<usize> = None;

    loop {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            break;
        }

        bytes.extend_from_slice(&buffer[..read]);

        if bytes.len() > MAX_REQUEST_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request exceeded maximum allowed size",
            ));
        }

        if expected_len.is_none() {
            if let Some(header_end) = find_bytes(&bytes, b"\r\n\r\n") {
                let content_length = parse_content_length(&bytes[..header_end]);
                expected_len = Some(header_end + 4 + content_length);
            }
        }

        if let Some(total_expected) = expected_len {
            if bytes.len() >= total_expected {
                break;
            }
        }
    }

    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "empty request",
        ));
    }

    Ok(bytes)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_content_length(header_bytes: &[u8]) -> usize {
    let headers = String::from_utf8_lossy(header_bytes);
    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                if let Ok(len) = value.trim().parse::<usize>() {
                    return len;
                }
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::parse_content_length;

    #[test]
    fn parses_content_length_from_headers() {
        let headers = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 12\r\n";
        assert_eq!(parse_content_length(headers), 12);
    }
}

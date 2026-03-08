use crate::http::middleware::{MiddlewareFn, Next, dispatch};
use crate::http::request::{find_bytes, parse_content_length_from_head};
use crate::http::router::{Handler, ResolveResult, Router};
use crate::http::{Request, Response};
use async_net::{TcpListener, TcpStream};
use futures_lite::future;
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const DEFAULT_MAX_REQUEST_SIZE: usize = 1024 * 1024;
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_MAX_CONNECTIONS: usize = 1024;

pub struct App {
    router: Router,
    middlewares: Vec<MiddlewareFn>,
    max_request_size: usize,
    read_timeout: Duration,
    max_connections: usize,
}

struct ActiveConnectionGuard {
    active_connections: Arc<AtomicUsize>,
}

impl ActiveConnectionGuard {
    fn new(active_connections: Arc<AtomicUsize>) -> Self {
        Self { active_connections }
    }
}

impl Drop for ActiveConnectionGuard {
    fn drop(&mut self) {
        self.active_connections.fetch_sub(1, Ordering::AcqRel);
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            middlewares: Vec::new(),
            max_request_size: DEFAULT_MAX_REQUEST_SIZE,
            read_timeout: DEFAULT_READ_TIMEOUT,
            max_connections: DEFAULT_MAX_CONNECTIONS,
        }
    }

    pub fn max_request_size(mut self, bytes: usize) -> Self {
        self.max_request_size = bytes.max(1024);
        self
    }

    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = timeout;
        self
    }

    pub fn max_connections(mut self, max_connections: usize) -> Self {
        self.max_connections = max_connections.max(1);
        self
    }

    pub fn route<F, Fut>(mut self, method: &str, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.router
            .add_route(method, pattern, handler)
            .expect("invalid route pattern");
        self
    }

    pub fn get<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("GET", pattern, handler)
    }

    pub fn post<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("POST", pattern, handler)
    }

    pub fn put<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("PUT", pattern, handler)
    }

    pub fn patch<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("PATCH", pattern, handler)
    }

    pub fn delete<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("DELETE", pattern, handler)
    }

    pub fn head<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("HEAD", pattern, handler)
    }

    pub fn options<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("OPTIONS", pattern, handler)
    }

    pub fn any<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.route("ANY", pattern, handler)
    }

    pub fn try_route<F, Fut>(
        mut self,
        method: &str,
        pattern: &str,
        handler: F,
    ) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.router.add_route(method, pattern, handler)?;
        Ok(self)
    }

    pub fn try_get<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("GET", pattern, handler)
    }

    pub fn try_post<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("POST", pattern, handler)
    }

    pub fn try_put<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("PUT", pattern, handler)
    }

    pub fn try_patch<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("PATCH", pattern, handler)
    }

    pub fn try_delete<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("DELETE", pattern, handler)
    }

    pub fn try_head<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("HEAD", pattern, handler)
    }

    pub fn try_options<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("OPTIONS", pattern, handler)
    }

    pub fn try_any<F, Fut>(self, pattern: &str, handler: F) -> Result<Self, String>
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.try_route("ANY", pattern, handler)
    }

    pub fn middleware<F, Fut>(mut self, middleware: F) -> Self
    where
        F: Fn(Request, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        self.middlewares.push(Arc::new(move |request, next| {
            Box::pin(middleware(request, next))
        }));
        self
    }

    pub async fn serve(self, address: &str) -> io::Result<()> {
        self.serve_with_shutdown(address, future::pending::<()>())
            .await
    }

    pub async fn serve_with_shutdown<F>(self, address: &str, shutdown: F) -> io::Result<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let listener = TcpListener::bind(address).await?;
        let router = Arc::new(self.router);
        let middlewares = Arc::new(self.middlewares);
        let active_connections = Arc::new(AtomicUsize::new(0));
        let max_request_size = self.max_request_size;
        let read_timeout = self.read_timeout;
        let max_connections = self.max_connections;
        let (shutdown_tx, shutdown_rx) = async_channel::bounded::<()>(1);

        smol::spawn(async move {
            shutdown.await;
            let _ = shutdown_tx.send(()).await;
        })
        .detach();

        tracing::info!(server.address = %address, "listening for http requests");

        loop {
            let event = future::or(
                async {
                    let _ = shutdown_rx.recv().await;
                    AcceptEvent::Shutdown
                },
                async {
                    match listener.accept().await {
                        Ok((stream, _)) => AcceptEvent::Accepted(stream),
                        Err(err) => AcceptEvent::AcceptError(err),
                    }
                },
            )
            .await;

            let mut stream = match event {
                AcceptEvent::Shutdown => break,
                AcceptEvent::AcceptError(err) => return Err(err),
                AcceptEvent::Accepted(stream) => stream,
            };

            let previous = active_connections.fetch_add(1, Ordering::AcqRel);
            if previous >= max_connections {
                active_connections.fetch_sub(1, Ordering::AcqRel);
                let response = Response::new(503, "Service Unavailable")
                    .header("Retry-After", "1")
                    .text_body("Server Busy");
                write_response(&mut stream, response, false).await?;
                continue;
            }

            let router = Arc::clone(&router);
            let middlewares = Arc::clone(&middlewares);
            let guard = ActiveConnectionGuard::new(Arc::clone(&active_connections));

            smol::spawn(async move {
                let _guard = guard;
                if let Err(err) =
                    handle_connection(stream, router, middlewares, read_timeout, max_request_size)
                        .await
                {
                    tracing::warn!(error = %err, "connection error");
                }
            })
            .detach();
        }

        while active_connections.load(Ordering::Acquire) > 0 {
            smol::Timer::after(Duration::from_millis(25)).await;
        }

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

enum AcceptEvent {
    Shutdown,
    Accepted(TcpStream),
    AcceptError(io::Error),
}

async fn handle_connection(
    mut stream: TcpStream,
    router: Arc<Router>,
    middlewares: Arc<Vec<MiddlewareFn>>,
    read_timeout: Duration,
    max_request_size: usize,
) -> io::Result<()> {
    let mut keep_alive = true;
    let mut buffered = Vec::with_capacity(4096);
    while keep_alive {
        let bytes =
            match read_request_bytes(&mut stream, &mut buffered, max_request_size, read_timeout)
                .await
            {
                Ok(request_bytes) => request_bytes,
                Err(err) => {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        return Ok(());
                    }

                    let response = response_for_read_error(&err);
                    write_response(&mut stream, response, false).await?;
                    return Ok(());
                }
            };

        let request = match Request::from_bytes(&bytes) {
            Ok(request) => request,
            Err(err) => {
                let response = response_for_parse_error(&err);
                write_response(&mut stream, response, false).await?;
                return Ok(());
            }
        };

        keep_alive = request.should_keep_alive();

        let method = request.method.clone();
        let path = request.path.clone();

        if method == "OPTIONS" {
            let allowed = router.allowed_methods(&path);
            let response = if allowed.is_empty() {
                Response::not_found().text_body("Not Found")
            } else {
                Response::no_content().header("Allow", format_allow_header(&allowed))
            };
            write_response(&mut stream, response, keep_alive).await?;
            if !keep_alive {
                break;
            }
            continue;
        }

        let resolved = if method == "HEAD" {
            match router.resolve("HEAD", &path) {
                ResolveResult::Found {
                    handler,
                    params,
                    matched_route,
                } => ResolveResult::Found {
                    handler,
                    params,
                    matched_route,
                },
                _ => router.resolve("GET", &path),
            }
        } else {
            router.resolve(&method, &path)
        };

        let (request, endpoint): (Request, Handler) = match resolved {
            ResolveResult::Found {
                handler,
                params,
                matched_route,
            } => (
                request
                    .with_params(params)
                    .with_matched_route(matched_route),
                handler,
            ),
            ResolveResult::MethodNotAllowed { allow } => {
                let allow_header = format_allow_header(&allow);
                (
                    request,
                    Arc::new(move |_request: Request| {
                        let allow_header = allow_header.clone();
                        Box::pin(async move {
                            Response::method_not_allowed()
                                .header("Allow", allow_header)
                                .text_body("Method Not Allowed")
                        })
                    }),
                )
            }
            ResolveResult::NotFound => (
                request,
                Arc::new(|_request: Request| {
                    Box::pin(async { Response::not_found().text_body("Not Found") })
                }),
            ),
        };

        let mut response = dispatch(0, request, Arc::clone(&middlewares), endpoint).await;
        if method == "HEAD" {
            response = response.into_head_response();
        }

        write_response(&mut stream, response, keep_alive).await?;

        if !keep_alive {
            break;
        }
    }

    Ok(())
}

async fn write_response(
    stream: &mut TcpStream,
    response: Response,
    keep_alive: bool,
) -> io::Result<()> {
    let response = response
        .header(
            "Connection",
            if keep_alive { "keep-alive" } else { "close" },
        )
        .header_if_missing("X-Content-Type-Options", "nosniff");

    stream.write_all(&response.to_http_bytes()).await?;
    stream.flush().await
}

async fn read_request_bytes(
    stream: &mut TcpStream,
    buffered: &mut Vec<u8>,
    max_request_size: usize,
    read_timeout: Duration,
) -> io::Result<Vec<u8>> {
    let mut buffer = [0u8; 4096];
    let mut expected_len = expected_request_len(buffered, max_request_size)?;

    loop {
        if let Some(total_expected) = expected_len
            && buffered.len() >= total_expected
        {
            let mut tail = buffered.split_off(total_expected);
            let request = std::mem::take(buffered);
            std::mem::swap(buffered, &mut tail);
            return Ok(request);
        }

        let read = future::or(
            async {
                smol::Timer::after(read_timeout).await;
                Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "request read timed out",
                ))
            },
            async {
                let read = stream.read(&mut buffer).await?;
                Ok(read)
            },
        )
        .await?;

        if read == 0 {
            if buffered.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "empty request",
                ));
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete request",
            ));
        }

        buffered.extend_from_slice(&buffer[..read]);

        if buffered.len() > max_request_size && expected_len.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request exceeded maximum allowed size",
            ));
        }

        if expected_len.is_none() {
            expected_len = expected_request_len(buffered, max_request_size)?;
        }
    }
}

fn expected_request_len(bytes: &[u8], max_request_size: usize) -> io::Result<Option<usize>> {
    let Some(header_end) = find_bytes(bytes, b"\r\n\r\n") else {
        return Ok(None);
    };

    let content_length = parse_content_length(&bytes[..header_end])?;
    let expected = header_end + 4 + content_length;
    if expected > max_request_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request exceeded maximum allowed size",
        ));
    }

    Ok(Some(expected))
}

fn parse_content_length(header_bytes: &[u8]) -> io::Result<usize> {
    parse_content_length_from_head(header_bytes)
}

fn response_for_read_error(err: &io::Error) -> Response {
    if err.kind() == io::ErrorKind::TimedOut {
        Response::request_timeout().text_body("Request Timeout")
    } else if err.kind() == io::ErrorKind::InvalidData {
        let message = err.to_string();
        if message.contains("maximum allowed size") {
            Response::payload_too_large().text_body("Payload Too Large")
        } else if message.contains("transfer-encoding") {
            Response::not_implemented().text_body("Not Implemented")
        } else {
            Response::bad_request().text_body("Bad Request")
        }
    } else {
        Response::bad_request().text_body("Bad Request")
    }
}

fn response_for_parse_error(err: &io::Error) -> Response {
    if err.kind() == io::ErrorKind::InvalidData {
        if err.to_string().contains("transfer-encoding") {
            Response::not_implemented().text_body("Not Implemented")
        } else {
            Response::bad_request().text_body("Bad Request")
        }
    } else {
        Response::internal_server_error().text_body("Internal Server Error")
    }
}

fn format_allow_header(methods: &[String]) -> String {
    let mut allow = methods.to_vec();
    if allow.iter().any(|method| method == "GET") && !allow.iter().any(|method| method == "HEAD") {
        allow.push("HEAD".to_string());
    }
    if !allow.iter().any(|method| method == "OPTIONS") {
        allow.push("OPTIONS".to_string());
    }
    allow.sort_unstable();
    allow.dedup();
    allow.join(", ")
}

#[cfg(test)]
mod tests {
    use super::{
        App, expected_request_len, format_allow_header, parse_content_length,
        response_for_read_error,
    };
    use crate::http::{Request, Response};
    use async_channel::Sender;
    use async_net::TcpStream;
    use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
    use std::io;
    use std::net::TcpListener as StdTcpListener;
    use std::time::Duration;

    async fn ok_handler(_request: Request) -> Response {
        Response::ok()
    }

    async fn start_test_server(
        app: App,
    ) -> io::Result<(String, Sender<()>, smol::Task<io::Result<()>>)> {
        let address = next_test_address()?;
        let (shutdown_tx, shutdown_rx) = async_channel::bounded::<()>(1);
        let server_address = address.clone();

        let task = smol::spawn(async move {
            app.serve_with_shutdown(&server_address, async move {
                let _ = shutdown_rx.recv().await;
            })
            .await
        });

        wait_for_server(&address).await?;
        Ok((address, shutdown_tx, task))
    }

    fn next_test_address() -> io::Result<String> {
        let listener = StdTcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        drop(listener);
        Ok(address.to_string())
    }

    async fn wait_for_server(address: &str) -> io::Result<()> {
        for _ in 0..100 {
            if TcpStream::connect(address).await.is_ok() {
                return Ok(());
            }
            smol::Timer::after(Duration::from_millis(10)).await;
        }

        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "server did not start in time",
        ))
    }

    async fn send_single_request(address: &str, raw_request: &str) -> io::Result<Vec<u8>> {
        let mut stream = TcpStream::connect(address).await?;
        stream.write_all(raw_request.as_bytes()).await?;
        stream.flush().await?;
        read_http_response(&mut stream).await
    }

    async fn read_http_response(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
        let mut response = Vec::with_capacity(1024);
        let mut buffer = [0u8; 1024];
        let mut expected_len: Option<usize> = None;

        loop {
            if let Some(expected_len) = expected_len
                && response.len() >= expected_len
            {
                break;
            }

            let read = stream.read(&mut buffer).await?;
            if read == 0 {
                if response.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "connection closed before response",
                    ));
                }
                break;
            }

            response.extend_from_slice(&buffer[..read]);
            if expected_len.is_none()
                && let Some(header_end) = super::find_bytes(&response, b"\r\n\r\n")
            {
                let content_length = parse_content_length(&response[..header_end]).unwrap_or(0);
                expected_len = Some(header_end + 4 + content_length);
            }
        }

        Ok(response)
    }

    fn status_code(response: &[u8]) -> u16 {
        let text = String::from_utf8_lossy(response);
        text.lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0)
    }

    fn header_value(response: &[u8], name: &str) -> Option<String> {
        let text = String::from_utf8_lossy(response);
        for line in text.lines().skip(1) {
            if line.is_empty() {
                break;
            }
            if let Some((header_name, header_value)) = line.split_once(':')
                && header_name.trim().eq_ignore_ascii_case(name)
            {
                return Some(header_value.trim().to_string());
            }
        }

        None
    }

    fn body_bytes(response: &[u8]) -> Vec<u8> {
        if let Some(header_end) = super::find_bytes(response, b"\r\n\r\n") {
            return response[header_end + 4..].to_vec();
        }

        Vec::new()
    }

    #[test]
    fn parses_content_length_from_headers() {
        let headers = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 12\r\n";
        assert_eq!(parse_content_length(headers).expect("content length"), 12);
    }

    #[test]
    fn rejects_conflicting_content_lengths() {
        let headers =
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nContent-Length: 9\r\n";
        assert!(parse_content_length(headers).is_err());
    }

    #[test]
    fn maps_timeout_error_to_408() {
        let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "request read timed out");
        let response = response_for_read_error(&err);
        assert_eq!(response.status_code(), 408);
    }

    #[test]
    fn maps_large_payload_to_413() {
        let err = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "request exceeded maximum allowed size",
        );
        let response = response_for_read_error(&err);
        assert_eq!(response.status_code(), 413);
    }

    #[test]
    fn allow_header_includes_head_and_options() {
        let allow = format_allow_header(&["GET".to_string(), "POST".to_string()]);
        assert_eq!(allow, "GET, HEAD, OPTIONS, POST");
    }

    #[test]
    fn builder_api_supports_all_http_methods() {
        let _app = App::new()
            .max_request_size(2048)
            .read_timeout(Duration::from_millis(500))
            .max_connections(10)
            .get("/", ok_handler)
            .post("/", ok_handler)
            .put("/", ok_handler)
            .patch("/", ok_handler)
            .delete("/", ok_handler)
            .head("/", ok_handler)
            .options("/", ok_handler)
            .any("/*path", ok_handler);
    }

    #[test]
    fn try_builders_allow_error_handling() {
        let app = App::new()
            .try_get("/", ok_handler)
            .expect("valid route")
            .try_post("/", ok_handler)
            .expect("valid route")
            .try_put("/", ok_handler)
            .expect("valid route")
            .try_patch("/", ok_handler)
            .expect("valid route")
            .try_delete("/", ok_handler)
            .expect("valid route")
            .try_head("/", ok_handler)
            .expect("valid route")
            .try_options("/", ok_handler)
            .expect("valid route")
            .try_any("/*path", ok_handler)
            .expect("valid route");

        assert!(app.try_get("invalid", ok_handler).is_err());
    }

    #[test]
    fn calculates_expected_request_length() {
        let request = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhelloextra";
        let expected = expected_request_len(request, 1024).expect("length parse");

        assert_eq!(expected, Some(60));
    }

    #[test]
    fn integration_serves_get_route_over_tcp() {
        smol::block_on(async {
            async fn route(_request: Request) -> Response {
                Response::text("ok")
            }

            let (address, shutdown_tx, task) = start_test_server(App::new().get("/", route))
                .await
                .expect("server should start");

            let response = send_single_request(
                &address,
                "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("request should succeed");

            assert_eq!(status_code(&response), 200);
            assert_eq!(body_bytes(&response), b"ok");

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());
        });
    }

    #[test]
    fn integration_supports_keep_alive_multiple_requests() {
        smol::block_on(async {
            async fn route(_request: Request) -> Response {
                Response::text("pong")
            }

            let (address, shutdown_tx, task) =
                start_test_server(App::new().get("/ping", route).max_connections(32))
                    .await
                    .expect("server should start");

            let mut stream = TcpStream::connect(&address)
                .await
                .expect("client should connect");

            stream
                .write_all(
                    b"GET /ping HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
                )
                .await
                .expect("first request write should succeed");
            stream.flush().await.expect("flush should succeed");
            let first = read_http_response(&mut stream)
                .await
                .expect("first response should succeed");

            assert_eq!(status_code(&first), 200);
            assert_eq!(
                header_value(&first, "Connection").as_deref(),
                Some("keep-alive")
            );
            assert_eq!(body_bytes(&first), b"pong");

            stream
                .write_all(b"GET /ping HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await
                .expect("second request write should succeed");
            stream.flush().await.expect("flush should succeed");
            let second = read_http_response(&mut stream)
                .await
                .expect("second response should succeed");

            assert_eq!(status_code(&second), 200);
            assert_eq!(
                header_value(&second, "Connection").as_deref(),
                Some("close")
            );

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());
        });
    }

    #[test]
    fn integration_head_uses_get_handler_without_body() {
        smol::block_on(async {
            async fn route(_request: Request) -> Response {
                Response::text("hello")
            }

            let (address, shutdown_tx, task) =
                start_test_server(App::new().get("/users/:id", route))
                    .await
                    .expect("server should start");

            let response = send_single_request(
                &address,
                "HEAD /users/42 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("request should succeed");

            assert_eq!(status_code(&response), 200);
            assert_eq!(
                header_value(&response, "Content-Length").as_deref(),
                Some("5")
            );
            assert!(body_bytes(&response).is_empty());

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());
        });
    }

    #[test]
    fn integration_attaches_matched_route_to_request() {
        smol::block_on(async {
            async fn route(request: Request) -> Response {
                Response::text(request.matched_route().unwrap_or("missing"))
            }

            let (address, shutdown_tx, task) =
                start_test_server(App::new().get("/users/:id", route))
                    .await
                    .expect("server should start");

            let response = send_single_request(
                &address,
                "GET /users/42 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("request should succeed");

            assert_eq!(status_code(&response), 200);
            assert_eq!(body_bytes(&response), b"/users/:id");

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());
        });
    }

    #[test]
    fn integration_options_returns_allow_header() {
        smol::block_on(async {
            async fn route(_request: Request) -> Response {
                Response::text("ok")
            }

            let (address, shutdown_tx, task) =
                start_test_server(App::new().get("/users/:id", route))
                    .await
                    .expect("server should start");

            let response = send_single_request(
                &address,
                "OPTIONS /users/9 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("request should succeed");

            assert_eq!(status_code(&response), 204);
            let allow = header_value(&response, "Allow").unwrap_or_default();
            assert!(allow.contains("GET"));
            assert!(allow.contains("HEAD"));
            assert!(allow.contains("OPTIONS"));

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());
        });
    }

    #[test]
    fn integration_rejects_payload_too_large() {
        smol::block_on(async {
            async fn route(_request: Request) -> Response {
                Response::ok()
            }

            let body = "a".repeat(4096);
            let request = format!(
                "POST /upload HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );

            let (address, shutdown_tx, task) =
                start_test_server(App::new().max_request_size(128).post("/upload", route))
                    .await
                    .expect("server should start");

            let response = send_single_request(&address, &request)
                .await
                .expect("request should succeed");

            assert_eq!(status_code(&response), 413);

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());
        });
    }

    #[test]
    fn integration_times_out_incomplete_request() {
        smol::block_on(async {
            async fn route(_request: Request) -> Response {
                Response::ok()
            }

            let (address, shutdown_tx, task) = start_test_server(
                App::new()
                    .read_timeout(Duration::from_millis(30))
                    .get("/", route),
            )
            .await
            .expect("server should start");

            let mut stream = TcpStream::connect(&address)
                .await
                .expect("client should connect");
            stream
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n")
                .await
                .expect("partial request should write");
            stream.flush().await.expect("flush should succeed");

            let response = read_http_response(&mut stream)
                .await
                .expect("timeout response should be returned");
            assert_eq!(status_code(&response), 408);

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());
        });
    }

    #[test]
    fn integration_graceful_shutdown_stops_server() {
        smol::block_on(async {
            async fn route(_request: Request) -> Response {
                Response::ok()
            }

            let (address, shutdown_tx, task) = start_test_server(App::new().get("/", route))
                .await
                .expect("server should start");

            shutdown_tx.send(()).await.expect("shutdown signal");
            assert!(task.await.is_ok());

            let reconnect = TcpStream::connect(&address).await;
            assert!(reconnect.is_err());
        });
    }
}

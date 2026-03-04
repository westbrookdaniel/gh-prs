mod http;

use askama::Template;
use http::{App, Request, Response, StaticDir, cors, logger, request_id, security_headers};

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    title: &'a str,
    message: &'a str,
}

async fn hello(_request: Request) -> Response {
    let html = HelloTemplate {
        title: "smol + askama",
        message: "Hello, world!",
    }
    .render()
    .unwrap_or_else(|err| format!("<h1>Template Error</h1><p>{err}</p>"));

    Response::html(html)
}

async fn user_by_id(request: Request) -> Response {
    let id = request.param("id").unwrap_or("unknown");
    Response::text(format!("user id: {id}"))
}

async fn not_found(request: Request) -> Response {
    let path = request.param("path").unwrap_or_default();
    Response::not_found().text_body(format!("Not Found: /{path}"))
}

fn main() -> std::io::Result<()> {
    smol::block_on(async {
        App::new()
            .middleware(request_id())
            .middleware(security_headers())
            .middleware(cors(
                "*",
                "GET, POST, PUT, PATCH, DELETE, OPTIONS",
                "Content-Type, Authorization",
            ))
            .middleware(logger())
            .middleware(
                StaticDir::builder("/assets", "assets")
                    .cache_control("public, max-age=86400")
                    .fallthrough(true)
                    .into_middleware(),
            )
            .get("/", hello)
            .get("/users/:id", user_by_id)
            .any("/*path", not_found)
            .serve("127.0.0.1:3000")
            .await
    })
}

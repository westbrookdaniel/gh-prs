mod http;

use askama::Template;
use http::{
    App, Request, Response, StaticDirOptions, cors, logger, request_id, security_headers,
    static_dir,
};

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
        let static_assets = StaticDirOptions {
            url_prefix: "/assets".to_string(),
            root: "assets".into(),
            cache_control: Some("public, max-age=86400".to_string()),
            fallthrough: true,
            ..StaticDirOptions::default()
        };

        App::new()
            .middleware(request_id())
            .middleware(security_headers())
            .middleware(cors(
                "*",
                "GET, POST, PUT, PATCH, DELETE, OPTIONS",
                "Content-Type, Authorization",
            ))
            .middleware(logger())
            .middleware(static_dir(static_assets))
            .get("/", hello)
            .get("/users/:id", user_by_id)
            .any("/*path", not_found)
            .serve("127.0.0.1:3000")
            .await
    })
}

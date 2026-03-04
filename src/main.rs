mod http;

use askama::Template;
use http::{App, Request, Response, logger};
use std::path::{Component, PathBuf};

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

async fn assets(request: Request) -> Response {
    let Some(path) = request.param("path") else {
        return Response::not_found().text_body("Not Found");
    };

    let mut full_path = PathBuf::from("assets");
    for component in PathBuf::from(path).components() {
        match component {
            Component::Normal(part) => full_path.push(part),
            _ => return Response::bad_request().text_body("Invalid asset path"),
        }
    }

    match std::fs::read(&full_path) {
        Ok(bytes) => {
            let content_type = match full_path.extension().and_then(|ext| ext.to_str()) {
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "application/javascript; charset=utf-8",
                Some("html") => "text/html; charset=utf-8",
                _ => "application/octet-stream",
            };

            Response::ok()
                .header("Content-Type", content_type)
                .body(bytes)
        }
        Err(_) => Response::not_found().text_body("Not Found"),
    }
}

async fn not_found(request: Request) -> Response {
    let path = request.param("path").unwrap_or_default();
    Response::not_found().text_body(format!("Not Found: /{path}"))
}

fn main() -> std::io::Result<()> {
    smol::block_on(async {
        App::new()
            .middleware(logger())
            .get("/", hello)
            .get("/users/:id", user_by_id)
            .get("/assets/*path", assets)
            .any("/*path", not_found)
            .serve("127.0.0.1:3000")
            .await
    })
}

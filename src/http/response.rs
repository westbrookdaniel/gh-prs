use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Response {
    status_code: u16,
    reason_phrase: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    pub fn new(status_code: u16, reason_phrase: &'static str) -> Self {
        Self {
            status_code,
            reason_phrase,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn ok() -> Self {
        Self::new(200, "OK")
    }

    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }

    pub fn method_not_allowed() -> Self {
        Self::new(405, "Method Not Allowed")
    }

    pub fn bad_request() -> Self {
        Self::new(400, "Bad Request")
    }

    pub fn internal_server_error() -> Self {
        Self::new(500, "Internal Server Error")
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    pub fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::ok()
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(text)
    }

    pub fn html(html: impl Into<String>) -> Self {
        let html = html.into();
        Self::ok()
            .header("Content-Type", "text/html; charset=utf-8")
            .body(html)
    }

    pub fn json<T: Serialize>(value: &T) -> Self {
        match serde_json::to_vec(value) {
            Ok(body) => Self::ok()
                .header("Content-Type", "application/json")
                .body(body),
            Err(err) => Self::internal_server_error()
                .text_body(format!("failed to serialize json response: {err}")),
        }
    }

    pub fn text_body(mut self, text: impl Into<String>) -> Self {
        self.headers.push((
            "Content-Type".to_string(),
            "text/plain; charset=utf-8".to_string(),
        ));
        self.body = text.into().into_bytes();
        self
    }

    pub fn to_http_bytes(&self) -> Vec<u8> {
        let mut response = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.reason_phrase);

        let mut has_content_length = false;
        for (name, value) in &self.headers {
            if name.eq_ignore_ascii_case("content-length") {
                has_content_length = true;
            }
            response.push_str(name);
            response.push_str(": ");
            response.push_str(value);
            response.push_str("\r\n");
        }

        if !has_content_length {
            response.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }

        response.push_str("Connection: close\r\n\r\n");

        let mut bytes = response.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::Response;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestPayload {
        name: &'static str,
    }

    #[test]
    fn serializes_json_response() {
        let response = Response::json(&TestPayload { name: "smol" });
        let raw =
            String::from_utf8(response.to_http_bytes()).expect("http response should be utf-8");

        assert!(raw.contains("HTTP/1.1 200 OK"));
        assert!(raw.contains("Content-Type: application/json"));
        assert!(raw.contains("{\"name\":\"smol\"}"));
    }
}

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

    pub fn no_content() -> Self {
        Self::new(204, "No Content")
    }

    pub fn bad_request() -> Self {
        Self::new(400, "Bad Request")
    }

    pub fn request_timeout() -> Self {
        Self::new(408, "Request Timeout")
    }

    pub fn payload_too_large() -> Self {
        Self::new(413, "Payload Too Large")
    }

    pub fn not_implemented() -> Self {
        Self::new(501, "Not Implemented")
    }

    pub fn internal_server_error() -> Self {
        Self::new(500, "Internal Server Error")
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        let value = value.into();

        self.insert_header(name, value);
        self
    }

    pub fn insert_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();

        if !is_valid_header_name(&name) || !is_valid_header_value(&value) {
            return;
        }

        if let Some(existing) = self
            .headers
            .iter_mut()
            .find(|(existing_name, _)| existing_name.eq_ignore_ascii_case(&name))
        {
            existing.1 = value;
        } else {
            self.headers.push((name, value));
        }
    }

    pub fn header_if_missing(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        if self
            .headers
            .iter()
            .any(|(existing_name, _)| existing_name.eq_ignore_ascii_case(&name))
        {
            return self;
        }

        self.header(name, value)
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
        self = self.header("Content-Type", "text/plain; charset=utf-8");
        self.body = text.into().into_bytes();
        self
    }

    pub fn into_head_response(mut self) -> Self {
        let body_len = self.body.len();
        if !self
            .headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        {
            self = self.header("Content-Length", body_len.to_string());
        }
        self.body.clear();
        self
    }

    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    pub fn to_http_bytes(&self) -> Vec<u8> {
        let mut response = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.reason_phrase);

        let mut has_content_length = false;
        let mut has_connection = false;
        for (name, value) in &self.headers {
            if name.eq_ignore_ascii_case("content-length") {
                has_content_length = true;
            }
            if name.eq_ignore_ascii_case("connection") {
                has_connection = true;
            }
            response.push_str(name);
            response.push_str(": ");
            response.push_str(value);
            response.push_str("\r\n");
        }

        if !has_content_length {
            response.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }

        if !has_connection {
            response.push_str("Connection: close\r\n");
        }

        response.push_str("\r\n");

        let mut bytes = response.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            matches!(
                byte,
                b'!' | b'#'..=b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'0'..=b'9'
                    | b'A'..=b'Z'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'a'..=b'z'
                    | b'|'
                    | b'~'
            )
        })
}

fn is_valid_header_value(value: &str) -> bool {
    !value.bytes().any(|byte| byte == b'\r' || byte == b'\n')
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

    #[test]
    fn ignores_invalid_header_values() {
        let response = Response::ok()
            .header("X-Test", "good")
            .header("X-Test", "bad\r\nInjected: yes");
        let raw =
            String::from_utf8(response.to_http_bytes()).expect("http response should be utf-8");

        assert!(raw.contains("X-Test: good\r\n"));
        assert!(!raw.contains("Injected"));
    }

    #[test]
    fn head_response_preserves_original_content_length() {
        let response = Response::text("hello").into_head_response();
        let raw =
            String::from_utf8(response.to_http_bytes()).expect("http response should be utf-8");

        assert!(raw.contains("Content-Length: 5\r\n"));
        assert!(!raw.ends_with("hello"));
    }

    #[test]
    fn preserves_custom_connection_header() {
        let response = Response::ok().header("Connection", "keep-alive");
        let raw =
            String::from_utf8(response.to_http_bytes()).expect("http response should be utf-8");

        assert!(raw.contains("Connection: keep-alive\r\n"));
        assert!(!raw.contains("Connection: close\r\n"));
    }
}

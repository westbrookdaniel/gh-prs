use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::io;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    query_params: HashMap<String, Vec<String>>,
    params: HashMap<String, String>,
    matched_route: Option<String>,
}

impl Request {
    pub(crate) fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let mut header_end = find_bytes(bytes, b"\r\n\r\n").ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing header terminator")
        })?;

        if header_end > 64 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request headers exceeded maximum size",
            ));
        }

        let mut headers_buf = [httparse::EMPTY_HEADER; 64];
        let mut parsed = httparse::Request::new(&mut headers_buf);
        match parsed.parse(bytes) {
            Ok(httparse::Status::Complete(consumed)) => {
                header_end = consumed.saturating_sub(4);
            }
            Ok(httparse::Status::Partial) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "incomplete request headers",
                ));
            }
            Err(err) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid request headers: {err}"),
                ));
            }
        }

        let method = parsed
            .method
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing method"))?
            .to_ascii_uppercase();
        let target = parsed
            .path
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing target"))?;
        let version = match parsed.version {
            Some(1) => "HTTP/1.1".to_string(),
            Some(0) => "HTTP/1.0".to_string(),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported http version",
                ));
            }
        };

        if !target.starts_with('/') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request target must be an absolute path",
            ));
        }

        let mut headers: HashMap<String, String> = HashMap::new();
        for header in parsed.headers.iter() {
            let name = header.name.to_ascii_lowercase();
            let value = std::str::from_utf8(header.value)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid header value"))?
                .trim()
                .to_string();

            if name == "content-length"
                && let Some(existing) = headers.get(&name)
            {
                if existing != &value {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "conflicting content-length headers",
                    ));
                }
                continue;
            }

            if let Some(existing) = headers.get_mut(&name) {
                existing.push_str(", ");
                existing.push_str(&value);
            } else {
                headers.insert(name, value);
            }
        }

        if headers.contains_key("transfer-encoding") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "transfer-encoding is not supported",
            ));
        }

        let body = bytes[header_end + 4..].to_vec();
        if let Some(content_length) = headers
            .get("content-length")
            .map(String::as_str)
            .map(parse_content_length)
            .transpose()?
            && body.len() != content_length
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request body length does not match content-length",
            ));
        }

        let (path, query) = if let Some((path, query)) = target.split_once('?') {
            (path.to_string(), Some(query.to_string()))
        } else {
            (target.to_string(), None)
        };

        let query_params = parse_query_params(query.as_deref());

        Ok(Self {
            method,
            path,
            query,
            version,
            headers,
            body,
            query_params,
            params: HashMap::new(),
            matched_route: None,
        })
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn query_param(&self, name: &str) -> Option<&str> {
        self.query_params
            .get(name)
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    pub fn matched_route(&self) -> Option<&str> {
        self.matched_route.as_deref()
    }

    pub fn query_values(&self, name: &str) -> Option<&[String]> {
        self.query_params.get(name).map(Vec::as_slice)
    }

    pub fn query_as<T: DeserializeOwned>(&self) -> io::Result<T> {
        let query = self.query.as_deref().unwrap_or_default();
        serde_urlencoded::from_str(query).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid query parameters: {err}"),
            )
        })
    }

    pub fn content_length(&self) -> Option<usize> {
        self.header("content-length")
            .and_then(|value| value.parse::<usize>().ok())
    }

    pub fn should_keep_alive(&self) -> bool {
        match self.header("connection") {
            Some(value) if value.eq_ignore_ascii_case("close") => false,
            Some(value) if value.eq_ignore_ascii_case("keep-alive") => true,
            _ => self.version.eq_ignore_ascii_case("HTTP/1.1"),
        }
    }

    pub fn json<T: DeserializeOwned>(&self) -> io::Result<T> {
        serde_json::from_slice(&self.body).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid json request body: {err}"),
            )
        })
    }

    pub(crate) fn with_params(mut self, params: HashMap<String, String>) -> Self {
        self.params = params;
        self
    }

    pub(crate) fn with_matched_route(mut self, matched_route: String) -> Self {
        self.matched_route = Some(matched_route);
        self
    }
}

pub(crate) fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub(crate) fn parse_content_length_from_head(header_bytes: &[u8]) -> io::Result<usize> {
    let headers = String::from_utf8_lossy(header_bytes);
    let mut content_length: Option<usize> = None;

    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("transfer-encoding") {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "transfer-encoding is not supported",
                ));
            }

            if name.trim().eq_ignore_ascii_case("content-length") {
                let parsed = parse_content_length(value.trim())?;
                if let Some(existing) = content_length {
                    if existing != parsed {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "conflicting content-length headers",
                        ));
                    }
                } else {
                    content_length = Some(parsed);
                }
            }
        }
    }

    Ok(content_length.unwrap_or(0))
}

fn parse_content_length(value: &str) -> io::Result<usize> {
    value
        .parse::<usize>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid content-length header"))
}

fn parse_query_params(query: Option<&str>) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    let Some(query) = query else {
        return map;
    };

    for (key, value) in
        serde_urlencoded::from_str::<Vec<(String, String)>>(query).unwrap_or_default()
    {
        map.entry(key).or_insert_with(Vec::new).push(value);
    }

    map
}

#[cfg(test)]
mod tests {
    use super::{Request, parse_content_length_from_head};
    use serde::Deserialize;

    #[test]
    fn parses_request_with_body_and_query() {
        let raw = b"POST /users/42?full=true HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhello";
        let request = Request::from_bytes(raw).expect("request should parse");

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/users/42");
        assert_eq!(request.query.as_deref(), Some("full=true"));
        assert_eq!(request.query_param("full"), Some("true"));
        assert_eq!(request.version, "HTTP/1.1");
        assert_eq!(request.header("host"), Some("localhost"));
        assert_eq!(request.content_length(), Some(5));
        assert_eq!(request.body, b"hello");
    }

    #[test]
    fn rejects_unsupported_transfer_encoding() {
        let raw = b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n";
        let err = Request::from_bytes(raw).expect_err("request should fail");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn rejects_conflicting_content_lengths() {
        let raw = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nContent-Length: 7\r\n\r\nhello";
        let err = Request::from_bytes(raw).expect_err("request should fail");

        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn parses_json_body() {
        #[derive(Deserialize)]
        struct Payload {
            id: u32,
        }

        let raw = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 8\r\n\r\n{\"id\":7}";
        let request = Request::from_bytes(raw).expect("request should parse");
        let payload: Payload = request.json().expect("json should parse");

        assert_eq!(payload.id, 7);
    }

    #[test]
    fn keep_alive_defaults_match_http_version() {
        let http11 = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let http10 = b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n";
        let req11 = Request::from_bytes(http11).expect("http/1.1 request should parse");
        let req10 = Request::from_bytes(http10).expect("http/1.0 request should parse");

        assert!(req11.should_keep_alive());
        assert!(!req10.should_keep_alive());
    }

    #[test]
    fn parses_repeated_query_keys() {
        let raw = b"GET /?tag=rust&tag=http HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::from_bytes(raw).expect("request should parse");
        let values = request.query_values("tag").expect("query values");

        assert_eq!(values, &["rust".to_string(), "http".to_string()]);
    }

    #[test]
    fn query_can_deserialize_to_struct() {
        #[derive(Deserialize)]
        struct Query {
            page: u32,
        }

        let raw = b"GET /?page=2 HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let request = Request::from_bytes(raw).expect("request should parse");
        let query: Query = request.query_as().expect("query should deserialize");

        assert_eq!(query.page, 2);
    }

    #[test]
    fn parses_content_length_from_head_bytes() {
        let head = b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 12\r\n";
        assert_eq!(
            parse_content_length_from_head(head).expect("content length"),
            12
        );
    }
}

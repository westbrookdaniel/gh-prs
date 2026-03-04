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
    params: HashMap<String, String>,
}

impl Request {
    pub(crate) fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let header_end = find_bytes(bytes, b"\r\n\r\n").ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing header terminator")
        })?;

        let head = std::str::from_utf8(&bytes[..header_end]).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid utf-8 in headers: {err}"),
            )
        })?;

        let mut lines = head.split("\r\n");
        let request_line = lines
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty request"))?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing method"))?;
        let target = request_parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing target"))?;
        let version = request_parts
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing http version"))?;

        let mut headers = HashMap::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }

            let (name, value) = line.split_once(':').ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid header line: {line}"),
                )
            })?;
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }

        let (path, query) = if let Some((path, query)) = target.split_once('?') {
            (path.to_string(), Some(query.to_string()))
        } else {
            (target.to_string(), None)
        };

        Ok(Self {
            method: method.to_string(),
            path,
            query,
            version: version.to_string(),
            headers,
            body: bytes[header_end + 4..].to_vec(),
            params: HashMap::new(),
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

    pub(crate) fn with_params(mut self, params: HashMap<String, String>) -> Self {
        self.params = params;
        self
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::Request;

    #[test]
    fn parses_request_with_body_and_query() {
        let raw = b"POST /users/42?full=true HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhello";
        let request = Request::from_bytes(raw).expect("request should parse");

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/users/42");
        assert_eq!(request.query.as_deref(), Some("full=true"));
        assert_eq!(request.version, "HTTP/1.1");
        assert_eq!(request.header("host"), Some("localhost"));
        assert_eq!(request.body, b"hello");
    }
}

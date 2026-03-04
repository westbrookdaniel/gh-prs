use serde::Deserialize;

use crate::http::Request;

#[derive(Debug, Deserialize)]
pub struct CommentForm {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct ReviewForm {
    pub event: String,
    pub body: String,
}

pub fn parse_form<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, String> {
    serde_urlencoded::from_bytes::<T>(&request.body).map_err(|err| err.to_string())
}

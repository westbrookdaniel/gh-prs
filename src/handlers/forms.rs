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

#[derive(Debug, Deserialize)]
pub struct ReviewersForm {
    pub reviewers: String,
}

#[derive(Debug, Deserialize)]
pub struct MergeForm {
    pub method: String,
    #[serde(default)]
    pub delete_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StateForm {
    pub state: String,
}

pub fn parse_form<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, String> {
    serde_urlencoded::from_bytes::<T>(&request.body).map_err(|err| err.to_string())
}

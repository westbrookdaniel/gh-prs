pub mod helpers;
pub mod templates;
pub mod types;

pub use crate::search::SearchArgs;
pub use templates::{
    ErrorTemplate, NotFoundTemplate, PrChangesTemplate, PrDetailTemplate, PrListTemplate,
};
pub use types::error_page_model;

pub fn not_found_page_model() -> crate::views::types::ErrorPageModel {
    crate::views::types::ErrorPageModel {
        page_title: "Not Found".to_string(),
        heading: "Page Not Found".to_string(),
        status_code: 404,
        message: "The page you requested does not exist.".to_string(),
        remediation: "Check the URL or return to the pull request list.".to_string(),
        details: None,
    }
}

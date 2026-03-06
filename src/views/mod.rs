pub mod builders;
pub mod helpers;
pub mod templates;
pub mod types;

pub use crate::search::SearchArgs;
pub use builders::{changes_page_model, detail_page_model, error_page_model, list_page_model};
pub use templates::{
    ErrorTemplate, NotFoundTemplate, PrChangesContentTemplate, PrChangesTemplate,
    PrDetailContentTemplate, PrDetailTemplate, PrListResultsTemplate, PrListTemplate,
};
pub use types::FlashMessageView;

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

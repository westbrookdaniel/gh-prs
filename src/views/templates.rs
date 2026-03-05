use askama::Template;

use crate::views::types::{ErrorPageModel, PrChangesPageModel, PrDetailPageModel, PrListPageModel};

#[derive(Template)]
#[template(path = "pages/pr_list.html")]
pub struct PrListTemplate {
    pub model: PrListPageModel,
}

#[derive(Template)]
#[template(path = "pages/pr_detail.html")]
pub struct PrDetailTemplate {
    pub model: PrDetailPageModel,
}

#[derive(Template)]
#[template(path = "pages/pr_changes.html")]
pub struct PrChangesTemplate {
    pub model: PrChangesPageModel,
}

#[derive(Template)]
#[template(path = "pages/error.html")]
pub struct ErrorTemplate {
    pub model: ErrorPageModel,
}

#[derive(Template)]
#[template(path = "pages/not_found.html")]
pub struct NotFoundTemplate {
    pub model: ErrorPageModel,
}

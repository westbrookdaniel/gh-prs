use askama::Template;

use crate::views::types::{ErrorPageModel, PrChangesPageModel, PrDetailPageModel, PrListPageModel};

#[derive(Template)]
#[template(path = "pages/pr_list.html")]
pub struct PrListTemplate {
    pub model: PrListPageModel,
}

#[derive(Template)]
#[template(path = "pages/fragments/pr_list_content.html")]
pub struct PrListContentTemplate {
    pub model: PrListPageModel,
}

#[derive(Template)]
#[template(path = "pages/fragments/pr_list_results.html")]
pub struct PrListResultsTemplate {
    pub model: PrListPageModel,
}

#[derive(Template)]
#[template(path = "pages/pr_detail.html")]
pub struct PrDetailTemplate {
    pub model: PrDetailPageModel,
}

#[derive(Template)]
#[template(path = "pages/fragments/pr_detail_content.html")]
pub struct PrDetailContentTemplate {
    pub model: PrDetailPageModel,
}

#[derive(Template)]
#[template(path = "pages/pr_changes.html")]
pub struct PrChangesTemplate {
    pub model: PrChangesPageModel,
}

#[derive(Template)]
#[template(path = "pages/fragments/pr_changes_content.html")]
pub struct PrChangesContentTemplate {
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

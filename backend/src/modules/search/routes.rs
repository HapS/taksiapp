use crate::app_state::AppState;
use crate::modules::search::controllers::{api, web};
use axum::{routing::get, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/search", get(web::search::search_page))
        .route("/api/search", get(api::search::search_api))
}

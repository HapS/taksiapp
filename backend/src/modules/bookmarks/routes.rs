use super::controllers::api as api_controllers;
use crate::app_state::AppState;
use axum::{routing::delete, routing::get, routing::post, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/user/bookmarks",
            get(api_controllers::bookmarks_api::list_bookmarks),
        )
        .route(
            "/api/user/bookmarks",
            post(api_controllers::bookmarks_api::create_bookmark),
        )
        .route(
            "/api/user/bookmarks/{id}",
            delete(api_controllers::bookmarks_api::delete_bookmark),
        )
}

use super::controllers::admin::api::comments_api as admin_api;
use super::controllers::admin::html::comments_html;
use super::controllers::api as api_controllers;
use crate::app_state::AppState;
use axum::{routing::delete, routing::get, routing::post, Router};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/comments", get(comments_html::comment_list_page))
        .route("/admin/api/comments", get(admin_api::list_comments))
        .route(
            "/admin/api/comments/{id}",
            delete(admin_api::delete_comment),
        )
        .route(
            "/admin/api/comments/{id}/toggle-publish",
            post(admin_api::toggle_publish),
        )
        .route(
            "/api/comments",
            get(api_controllers::comments_api::list_comments),
        )
        .route(
            "/api/comments",
            post(api_controllers::comments_api::create_comment),
        )
        .route(
            "/api/comments/{id}",
            get(api_controllers::comments_api::get_comment),
        )
        .route(
            "/api/comments/{id}",
            delete(api_controllers::comments_api::delete_comment),
        )
}

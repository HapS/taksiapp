use super::controllers::api as api_controllers;
use crate::app_state::AppState;
use axum::{
    routing::get,
    Router,
};

// Timeline modülü URL patterns
pub fn routes() -> Router<AppState> {
    Router::new()
        // API: Timeline endpoints
        .route("/api/timeline/user", get(api_controllers::timeline::get_user_timeline))
        .route("/api/timeline/content/{module_type}/{content_type}/{content_id}", 
               get(api_controllers::timeline::get_content_timeline))
        .route("/api/timeline/events", get(api_controllers::timeline::list_timeline_events))
}
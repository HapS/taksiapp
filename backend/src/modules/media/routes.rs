// Media Routes
use super::controllers::{api, web};
use crate::app_state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        // Web: Media Explorer HTML View
        .route("/admin/media", get(web::media::media_explorer))
        // API: Media Management
        .route(
            "/admin/api/media",
            get(api::media::list_media).post(api::media::upload_media),
        )
        .route("/admin/api/media/clone", post(api::media::clone_media))
        .route(
            "/admin/api/media/{id}",
            get(api::media::get_media)
                .put(api::media::update_media)
                .delete(api::media::delete_media),
        )
}

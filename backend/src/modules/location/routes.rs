use crate::app_state::AppState;
use crate::middleware::jwt::jwt_auth_middleware;
use crate::modules::location::controllers::location;
use axum::{
    routing::{delete, get, post, put},
    Router,
};

pub fn routes() -> Router<AppState> {
    // Herkese açık arama (JWT gerekli, admin değil)
    let public = Router::new()
        .route("/api/locations/search", get(location::search_locations))
        .layer(axum::middleware::from_fn(jwt_auth_middleware));

    // Admin CRUD (JWT + admin yetkisi gerekli — controller içinde kontrol)
    let admin = Router::new()
        .route("/admin/api/locations", post(location::admin_create_location))
        .route("/admin/api/locations", get(location::admin_list_locations))
        .route("/admin/api/locations/{id}", get(location::admin_get_location))
        .route("/admin/api/locations/{id}", put(location::admin_update_location))
        .route("/admin/api/locations/{id}", delete(location::admin_delete_location))
        .layer(axum::middleware::from_fn(jwt_auth_middleware));

    Router::new().merge(public).merge(admin)
}
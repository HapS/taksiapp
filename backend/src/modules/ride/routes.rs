use crate::app_state::AppState;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use crate::middleware::jwt::jwt_auth_middleware;

pub fn routes() -> Router<AppState> {
    let protected = Router::new()
        .route(
            "/api/ride/request",
            post(super::controllers::ride::request_ride),
        )
        .route(
            "/api/ride/driver/active",
            get(super::controllers::ride::get_driver_active_ride),
        )
        .route(
            "/api/ride/passenger/active",
            get(super::controllers::ride::get_passenger_active_ride),
        )
        .route("/api/ride/{id}", get(super::controllers::ride::get_ride))
        .route(
            "/api/ride/{id}/status",
            post(super::controllers::ride::update_ride_status),
        )
        .route(
            "/api/ride/{id}/cancel",
            post(super::controllers::ride::cancel_ride),
        )
        .route(
            "/api/ride/route",
            get(super::controllers::ride::get_route),
        )
        .route(
            "/api/ride/drivers/nearby",
            get(super::controllers::ride::get_nearby_drivers),
        )
        .route(
            "/api/ride/history",
            get(super::controllers::ride::get_ride_history),
        )
        .layer(middleware::from_fn(jwt_auth_middleware));

    Router::new()
        .merge(protected)
        .route("/ws/driver", get(super::ws::handler::driver_ws_handler))
        .route(
            "/ws/passenger",
            get(super::ws::handler::passenger_ws_handler),
        )
}

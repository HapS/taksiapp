use axum::{routing::{get, post}, Router};
use crate::app_state::AppState;
use crate::modules::iot::controllers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/iot-config", get(controllers::iot_config))
        .route("/iot", post(controllers::esp32c6))
}
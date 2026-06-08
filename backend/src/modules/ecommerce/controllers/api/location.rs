use crate::app_state::AppState;
use crate::modules::ecommerce::services::address_service::AddressService;
use axum::{
    extract::{Path, State},
    response::Json,
};

/// List all countries
pub async fn get_countries(State(state): State<AppState>) -> Json<serde_json::Value> {
    match AddressService::list_countries(&state.db).await {
        Ok(countries) => Json(serde_json::json!({
            "status": "success",
            "data": countries
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

/// List cities by country id
pub async fn get_cities(
    State(state): State<AppState>,
    Path(country_id): Path<i64>,
) -> Json<serde_json::Value> {
    match AddressService::list_cities(&state.db, country_id).await {
        Ok(cities) => Json(serde_json::json!({
            "status": "success",
            "data": cities
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

/// List districts by city id
pub async fn get_districts(
    State(state): State<AppState>,
    Path(city_id): Path<i64>,
) -> Json<serde_json::Value> {
    match AddressService::list_districts(&state.db, city_id).await {
        Ok(districts) => Json(serde_json::json!({
            "status": "success",
            "data": districts
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

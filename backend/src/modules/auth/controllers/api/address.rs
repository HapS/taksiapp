use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::ecommerce::services::address_service::AddressService;
use axum::{
    extract::{Path, State},
    response::Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateAddressRequest {
    pub title: String,
    pub country_id: i64,
    pub city_id: i64,
    pub district_id: i64,
    pub address_line: String,
    pub is_default: bool,
    pub phone_country_code: String,
    pub phone_number: String,
    pub address_type: String,
    pub company_name: Option<String>,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub id_number: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateAddressRequest {
    pub title: Option<String>,
    pub country_id: Option<i64>,
    pub city_id: Option<i64>,
    pub district_id: Option<i64>,
    pub address_line: Option<String>,
    pub is_default: Option<bool>,
    pub phone_country_code: Option<String>,
    pub phone_number: Option<String>,
    pub address_type: Option<String>,
    pub company_name: Option<String>,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub id_number: Option<String>,
}

/// List all addresses for the current user
pub async fn list_addresses(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Json<serde_json::Value> {
    match AddressService::list_addresses(&state.db, auth_user.id).await {
        Ok(addresses) => Json(serde_json::json!({
            "status": "success",
            "data": addresses
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

/// Create a new address
pub async fn create_address(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateAddressRequest>,
) -> Json<serde_json::Value> {
    match AddressService::create_address(
        &state.db,
        auth_user.id,
        payload.title,
        payload.country_id,
        payload.city_id,
        payload.district_id,
        payload.address_line,
        payload.is_default,
        payload.phone_country_code,
        payload.phone_number,
        payload.address_type,
        payload.company_name,
        payload.tax_office,
        payload.tax_number,
        payload.id_number,
    )
    .await
    {
        Ok(address) => Json(serde_json::json!({
            "status": "success",
            "data": address
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

/// Update an address
pub async fn update_address(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateAddressRequest>,
) -> Json<serde_json::Value> {
    match AddressService::update_address(
        &state.db,
        id,
        auth_user.id,
        payload.title,
        payload.country_id,
        payload.city_id,
        payload.district_id,
        payload.address_line,
        payload.is_default,
        payload.phone_country_code,
        payload.phone_number,
        payload.address_type,
        payload.company_name,
        payload.tax_office,
        payload.tax_number,
        payload.id_number,
    )
    .await
    {
        Ok(address) => Json(serde_json::json!({
            "status": "success",
            "data": address
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

/// Delete an address
pub async fn delete_address(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    match AddressService::delete_address(&state.db, id, auth_user.id).await {
        Ok(_) => Json(serde_json::json!({
            "status": "success",
            "message": "Address deleted successfully"
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

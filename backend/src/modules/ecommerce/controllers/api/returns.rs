// Returns API Controller - İade talebi endpoint'leri (müşteri tarafı)
use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::ecommerce::services::return_service::{
    self, CreateReturnRequest, ReturnRequestResponse, UpdateCargoRequest,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// POST /api/orders/:cart_id/items/:item_id/return - Yeni iade talebi oluştur
pub async fn create_return_request(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((cart_id, item_id)): Path<(i64, i64)>,
    Json(request): Json<CreateReturnRequest>,
) -> Response {
    let user_id = auth.id;

    match return_service::create_return_request(&state.db, user_id, cart_id, item_id, request).await
    {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("Sipariş veya ürün bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::BadRequest(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::InvalidOperation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// DELETE /api/returns/:return_id - İade talebini iptal et
pub async fn cancel_return_request(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(return_id): Path<i64>,
) -> Response {
    let user_id = auth.id;

    match return_service::cancel_return_request(&state.db, user_id, return_id).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::InvalidOperation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /api/returns/:return_id/cargo - Kargo takip bilgisi gir
pub async fn update_return_cargo(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(return_id): Path<i64>,
    Json(request): Json<UpdateCargoRequest>,
) -> Response {
    let user_id = auth.id;

    match return_service::update_return_cargo(&state.db, user_id, return_id, request).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::BadRequest(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::InvalidOperation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// GET /api/returns - Kullanıcının tüm iade taleplerini listele
/// GET /api/returns?cart_id=123 - Belirli siparişin iade talepleri
pub async fn list_return_requests(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let user_id = auth.id;
    let cart_id = params.get("cart_id").and_then(|v| v.parse::<i64>().ok());

    match return_service::get_user_return_requests(&state.db, user_id, cart_id).await {
        Ok(returns) => (
            StatusCode::OK,
            Json(ApiResponse::<Vec<ReturnRequestResponse>> {
                success: true,
                data: Some(returns),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<ReturnRequestResponse>> {
                success: false,
                data: None,
                error: Some(format!("İade talepleri getirilemedi: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// GET /api/returns/:return_id - Tek bir iade talebini getir
pub async fn get_return_request(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(return_id): Path<i64>,
) -> Response {
    let user_id = auth.id;

    match return_service::get_return_request(&state.db, user_id, return_id).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_service::ReturnServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<ReturnRequestResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {}", e)),
            }),
        )
            .into_response(),
    }
}

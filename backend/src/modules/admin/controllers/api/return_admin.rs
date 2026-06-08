// Admin Return Request API Controller - İade talebi yönetimi (admin tarafı)
use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::services::return_admin_service::{
    self, AdminReturnListQuery, AdminReturnResponse, ApproveReturnRequest, CompleteReturnRequest,
    RejectReturnRequest,
};
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub meta: Option<PaginationMeta>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
}

#[derive(Deserialize)]
pub struct ReturnListQueryParams {
    pub status: Option<String>,
    pub user_id: Option<i64>,
    pub cart_id: Option<i64>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Deserialize)]
pub struct AdminNotesRequest {
    pub admin_notes: Option<String>,
}

/// GET /admin/api/returns - Admin: İade taleplerini listele
pub async fn list_return_requests(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(params): Query<ReturnListQueryParams>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let query = AdminReturnListQuery {
        status: params.status,
        user_id: params.user_id,
        cart_id: params.cart_id,
        page: params.page,
        per_page: params.per_page,
    };

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    match return_admin_service::get_admin_return_requests(&state.db, &query).await {
        Ok((returns, total)) => {
            let total_pages = if total == 0 {
                0
            } else {
                (total + per_page - 1) / per_page
            };

            (
                StatusCode::OK,
                Json(PaginatedResponse {
                    success: true,
                    data: Some(returns),
                    meta: Some(PaginationMeta {
                        page,
                        per_page,
                        total,
                        total_pages,
                    }),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PaginatedResponse::<Vec<AdminReturnResponse>> {
                success: false,
                data: None,
                meta: None,
                error: Some(format!("İade talepleri getirilemedi: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// GET /admin/api/returns/:return_id - Admin: Tek iade talebi detayı
pub async fn get_return_request(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(return_id): Path<i64>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match return_admin_service::get_admin_return_request(&state.db, return_id).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(format!("Hata: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /admin/api/returns/:return_id/approve - Admin: İade talebini onayla
pub async fn approve_return_request(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(return_id): Path<i64>,
    Json(request): Json<ApproveReturnRequest>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match return_admin_service::approve_return_request(&state.db, return_id, request).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::InvalidOperation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(format!("Hata: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /admin/api/returns/:return_id/reject - Admin: İade talebini reddet
pub async fn reject_return_request(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(return_id): Path<i64>,
    Json(request): Json<RejectReturnRequest>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match return_admin_service::reject_return_request(&state.db, return_id, request).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::InvalidOperation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(format!("Hata: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /admin/api/returns/:return_id/received - Admin: İade ürünü teslim alındı
pub async fn mark_return_received(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(return_id): Path<i64>,
    Json(request): Json<AdminNotesRequest>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match return_admin_service::mark_return_received(&state.db, return_id, request.admin_notes)
        .await
    {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::InvalidOperation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(format!("Hata: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /admin/api/returns/:return_id/complete - Admin: İade tamamla (refund bilgisi ile)
pub async fn complete_return_request(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(return_id): Path<i64>,
    Json(request): Json<CompleteReturnRequest>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match return_admin_service::complete_return_request(&state.db, return_id, request).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::InvalidOperation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(format!("Hata: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /admin/api/returns/:return_id/notes - Admin: Admin notunu güncelle
pub async fn update_admin_notes(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(return_id): Path<i64>,
    Json(request): Json<AdminNotesRequest>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let notes = request.admin_notes.unwrap_or_default();

    match return_admin_service::update_admin_notes(&state.db, return_id, notes).await {
        Ok(ret) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(ret),
                error: None,
            }),
        )
            .into_response(),
        Err(return_admin_service::AdminReturnError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some("İade talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminReturnResponse> {
                success: false,
                data: None,
                error: Some(format!("Hata: {}", e)),
            }),
        )
            .into_response(),
    }
}

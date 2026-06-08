use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{
    app_state::AppState,
    middleware::auth::AuthenticatedUser,
    modules::b2b::{
        dto::company_dto::*,
        services::company_service::CompanyService,
    },
};

#[derive(Debug, Deserialize)]
pub struct CompanyListQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub current_page: u64,
    pub total_pages: u64,
    pub per_page: u64,
    pub total: u64,
}

/// GET /admin/api/b2b/companies - Liste
pub async fn list_companies(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Query(params): Query<CompanyListQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);
    
    match CompanyService::list_companies(
        &state.db,
        page,
        per_page,
        params.search,
        params.is_active,
    )
    .await
    {
        Ok((companies, total)) => {
            let total_pages = (total as f64 / per_page as f64).ceil() as u64;
            
            let response = PaginatedResponse {
                data: companies,
                meta: PaginationMeta {
                    current_page: page,
                    total_pages,
                    per_page,
                    total,
                },
            };
            
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            eprintln!("Error listing companies: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to list companies"
                }))
            ).into_response()
        }
    }
}

/// GET /admin/api/b2b/companies/{id} - Detay
pub async fn get_company(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match CompanyService::get_company_by_id(&state.db, id).await {
        Ok(Some(company)) => (StatusCode::OK, Json(company)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Company not found"
            }))
        ).into_response(),
        Err(e) => {
            eprintln!("Error fetching company: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch company"
                }))
            ).into_response()
        }
    }
}

/// POST /admin/api/b2b/companies - Yeni şirket oluştur
pub async fn create_company(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CompanyCreateRequest>,
) -> impl IntoResponse {
    match CompanyService::create_company(&state.db, payload, user.id).await {
        Ok(company) => (StatusCode::CREATED, Json(company)).into_response(),
        Err(e) => {
            eprintln!("Error creating company: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to create company: {}", e)
                }))
            ).into_response()
        }
    }
}

/// PUT /admin/api/b2b/companies/{id} - Şirket güncelle
pub async fn update_company(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(payload): Json<CompanyUpdateRequest>,
) -> impl IntoResponse {
    match CompanyService::update_company(&state.db, id, payload).await {
        Ok(company) => (StatusCode::OK, Json(company)).into_response(),
        Err(e) => {
            eprintln!("Error updating company: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to update company: {}", e)
                }))
            ).into_response()
        }
    }
}

/// PUT /admin/api/b2b/companies/{id}/admin - Admin güncelleme
pub async fn admin_update_company(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(payload): Json<CompanyAdminUpdateRequest>,
) -> impl IntoResponse {
    match CompanyService::admin_update_company(&state.db, id, payload).await {
        Ok(company) => (StatusCode::OK, Json(company)).into_response(),
        Err(e) => {
            eprintln!("Error updating company: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to update company: {}", e)
                }))
            ).into_response()
        }
    }
}

/// DELETE /admin/api/b2b/companies/{id} - Şirket sil
pub async fn delete_company(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match CompanyService::delete_company(&state.db, id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "Company deleted successfully"
            }))
        ).into_response(),
        Err(e) => {
            eprintln!("Error deleting company: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to delete company: {}", e)
                }))
            ).into_response()
        }
    }
}

/// POST /admin/api/b2b/companies/{id}/approve - Şirket onayla
pub async fn approve_company(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match CompanyService::approve_company(&state.db, id, user.id).await {
        Ok(company) => (StatusCode::OK, Json(company)).into_response(),
        Err(e) => {
            eprintln!("Error approving company: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to approve company: {}", e)
                }))
            ).into_response()
        }
    }
}

/// POST /admin/api/b2b/companies/{id}/toggle-active - Aktif/Pasif değiştir
pub async fn toggle_active(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match CompanyService::toggle_active(&state.db, id).await {
        Ok(company) => (StatusCode::OK, Json(company)).into_response(),
        Err(e) => {
            eprintln!("Error toggling active status: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to toggle active status: {}", e)
                }))
            ).into_response()
        }
    }
}

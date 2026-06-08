use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::dto::credit_dto::{CreateAdjustmentRequest, CreatePaymentRequest};
use crate::modules::admin::services::credit_service;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    Extension,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct CreditTransactionQuery {
    pub company_id: Option<i64>,
    pub transaction_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// GET /admin/api/credit/transactions - Kredi işlemlerini listele
pub async fn get_credit_transactions(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
    Query(query): Query<CreditTransactionQuery>,
) -> impl IntoResponse {
    let user_id = match user_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Unauthorized" })),
            )
                .into_response()
        }
    };

    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, user_id).await {
        return response;
    }

    match credit_service::get_all_credit_transactions(
        &state.db,
        query.company_id,
        query.transaction_type,
        query.start_date,
        query.end_date,
        query.sort_by,
        query.sort_order,
        query.limit,
        query.offset,
    )
    .await
    {
        Ok(transactions) => (StatusCode::OK, Json(json!({ "data": transactions }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct CompanyCreditQuery {
    pub company_id: i64,
}

/// GET /admin/api/credit/summary - Şirket kredi özetini getir
pub async fn get_company_credit_summary(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
    Query(query): Query<CompanyCreditQuery>,
) -> impl IntoResponse {
    let user_id = match user_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Unauthorized" })),
            )
                .into_response()
        }
    };

    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, user_id).await {
        return response;
    }

    match credit_service::get_company_credit_summary(&state.db, query.company_id).await {
        Ok(summary) => (StatusCode::OK, Json(json!({ "data": summary }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

/// POST /admin/api/credit/payment - Manuel ödeme kaydet
pub async fn create_payment(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<CreatePaymentRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match credit_service::create_manual_payment(&state.db, request, auth_user.id).await {
        Ok(transaction) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Ödeme başarıyla kaydedildi",
                "data": transaction
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("{}", e)
            })),
        )
            .into_response(),
    }
}

/// POST /admin/api/credit/adjustment - Kredi düzeltme
pub async fn create_adjustment(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<CreateAdjustmentRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match credit_service::create_credit_adjustment(&state.db, request, auth_user.id).await {
        Ok(transaction) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Kredi düzeltmesi başarıyla yapıldı",
                "data": transaction
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!("{}", e)
            })),
        )
            .into_response(),
    }
}

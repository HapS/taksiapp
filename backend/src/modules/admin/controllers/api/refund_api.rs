use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::dto::refund_dto::{
    BulkMarkBankRefundedRequest, BulkRefundToB2BCreditRequest,
};
use crate::modules::admin::services::refund_service;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde_json::json;

/// POST /admin/api/refund/bulk-b2b-credit
/// Tüm iptal edilen ürünler için toplu B2B kredi iadesi
pub async fn bulk_refund_to_b2b_credit(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<BulkRefundToB2BCreditRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match refund_service::bulk_refund_to_b2b_credit(&state.db, request, auth_user.id).await {
        Ok(transaction) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Toplu kredi iadesi yapıldı (kargo düşülmüş)",
                "data": {
                    "transaction_id": transaction.id,
                    "amount": transaction.amount.to_string(),
                    "currency": transaction.currency,
                }
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

/// POST /admin/api/refund/bulk-bank
/// Tüm iptal edilen ürünler için toplu banka iadesi
pub async fn bulk_mark_bank_refunded(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<BulkMarkBankRefundedRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match refund_service::bulk_mark_bank_refunded(&state.db, request, auth_user.id).await {
        Ok(result) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": format!("{} ürün banka iadesi yapıldı (kargo düşülmüş)", result.refunded_count),
                "data": {
                    "refunded_count": result.refunded_count,
                    "net_refund_total": result.net_refund_total.to_string(),
                    "cargo_deducted": result.cargo_deducted.to_string(),
                    "currency": result.refund_currency,
                }
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

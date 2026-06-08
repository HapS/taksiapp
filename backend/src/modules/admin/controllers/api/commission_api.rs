use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::dto::commission_dto::{
    CreateCommissionAdjustmentRequest, CreateCommissionPaymentRequest,
};
use crate::modules::admin::services::commission_service;
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
pub struct CommissionTransactionQuery {
    pub representative_id: Option<i64>,
    pub transaction_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// GET /admin/api/commission/transactions - Komisyon işlemlerini listele
pub async fn get_commission_transactions(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
    Query(query): Query<CommissionTransactionQuery>,
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

    match commission_service::get_all_commission_transactions(
        &state.db,
        query.representative_id,
        None, // company_id - not used in UI filter
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
pub struct RepresentativeCommissionQuery {
    pub representative_id: i64,
}

/// GET /admin/api/commission/summary - Temsilci komisyon özetini getir
pub async fn get_representative_commission_summary(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
    Query(query): Query<RepresentativeCommissionQuery>,
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

    match commission_service::get_representative_commission_summary(&state.db, query.representative_id).await {
        Ok(summary) => (StatusCode::OK, Json(json!({ "data": summary }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

/// POST /admin/api/commission/payment - Komisyon ödemesi kaydet
pub async fn create_commission_payment(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<CreateCommissionPaymentRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match commission_service::create_manual_commission_payment(&state.db, request, auth_user.id).await {
        Ok(transaction) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Komisyon ödemesi başarıyla kaydedildi",
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

/// POST /admin/api/commission/adjustment - Komisyon düzeltme
pub async fn create_commission_adjustment(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<CreateCommissionAdjustmentRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match commission_service::create_commission_adjustment(&state.db, request, auth_user.id).await {
        Ok(transaction) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Komisyon düzeltmesi başarıyla yapıldı",
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

/// GET /admin/api/commission/representatives - Temsilcileri listele
pub async fn get_representatives(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
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

    match commission_service::list_representatives(&state.db, 1, 1000, None).await {
        Ok((representatives, _total)) => (StatusCode::OK, Json(json!({ "data": representatives }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

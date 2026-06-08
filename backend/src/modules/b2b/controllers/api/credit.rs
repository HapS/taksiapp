use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::services::credit_service;
use crate::modules::b2b::entities::company_users;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct UserCreditQuery {
    pub transaction_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// GET /api/b2b/credit/me - Kullanıcının şirketinin kredi özetini getir
pub async fn get_my_company_credit_summary(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> impl IntoResponse {
    let user_id = auth_user.id;

    match company_users::Entity::find()
        .filter(company_users::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(company_user)) => {
            let company_id = company_user.company_id;
            match credit_service::get_company_credit_summary(&state.db, company_id).await {
                Ok(summary) => (
                    StatusCode::OK,
                    Json(json!({ "data": summary })),
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Database error: {}", e) })),
                ).into_response(),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Şirket bulunamadı" })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        ).into_response(),
    }
}

/// GET /api/b2b/credit/transactions - Kullanıcının şirketinin işlem geçmişini getir
pub async fn get_my_credit_transactions(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<UserCreditQuery>,
) -> impl IntoResponse {
    let user_id = auth_user.id;

    match company_users::Entity::find()
        .filter(company_users::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(company_user)) => {
            let company_id = company_user.company_id;

            match credit_service::get_all_credit_transactions(
                &state.db,
                Some(company_id),
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
                Ok(transactions) => (
                    StatusCode::OK,
                    Json(json!({ "data": transactions })),
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Database error: {}", e) })),
                ).into_response(),
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Şirket bulunamadı" })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        ).into_response(),
    }
}
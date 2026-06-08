use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::b2b::services::company_service;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

/// Get current user's company info
pub async fn get_my_company(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> impl IntoResponse {
    match company_service::CompanyService::get_company_by_user_id(&state.db, auth_user.id).await {
        Ok(Some(company)) => (StatusCode::OK, Json(company)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Company not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

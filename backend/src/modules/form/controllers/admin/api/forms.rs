use crate::{
    app_state::AppState, middleware::auth::AuthenticatedUser,
    modules::form::services::form_service::FormService,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct FormListQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub form_id: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[allow(dead_code)]
    pub sort_by: Option<String>,
    #[allow(dead_code)]
    pub sort_order: Option<String>,
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
pub async fn list_forms_data(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Query(params): Query<FormListQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);

    match FormService::list_form_data(
        &state.db,
        page,
        per_page,
        params.search,
        params.form_id,
        params.start_date,
        params.end_date,
    )
    .await
    {
        Ok((forms, total)) => {
            let total_pages = (total as f64 / per_page as f64).ceil() as u64;

            let response = PaginatedResponse {
                data: forms,
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
            eprintln!("Error listing forms: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to list companies"
                })),
            )
                .into_response()
        }
    }
}

/// GET /admin/api/b2b/companies/{id} - Detay
pub async fn get_form_data(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match FormService::get_form_data_by_id(&state.db, id).await {
        Ok(Some(form)) => (StatusCode::OK, Json(form)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Form not found"
            })),
        )
            .into_response(),
        Err(e) => {
            eprintln!("Error fetching form: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch form"
                })),
            )
                .into_response()
        }
    }
}

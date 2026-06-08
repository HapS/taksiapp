// API Controllers - JSON endpoints
use crate::app_state::AppState;
use crate::config;
use crate::modules::content::helpers::PageResponse;
use crate::modules::content::services::page_service;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub lang: Option<String>,
}

/// API: List all published pages
pub async fn list(State(state): State<AppState>, Query(query): Query<PageQuery>) -> Response {
    let config = config::get_config();
    let language = config.get_language_or_default(query.lang.as_deref());

    match page_service::list_pages(&state.db, &language).await {
        Ok(pages) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(pages),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<PageResponse>> {
                success: false,
                data: None,
                error: Some(format!("Failed to fetch pages: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// API: Get page by ID
pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<PageQuery>,
) -> Response {
    let config = config::get_config();
    let language = config.get_language_or_default(query.lang.as_deref());

    match page_service::get_page(&state.db, &language, None, Some(id)).await {
        Ok(page) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(page),
                error: None,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<PageResponse> {
                success: false,
                data: None,
                error: Some("Page not found".to_string()),
            }),
        )
            .into_response(),
    }
}

/// API: Get page by slug
pub async fn get_by_slug(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let config = config::get_config();
    let language = config.get_language_or_default(query.lang.as_deref());

    match page_service::get_page(&state.db, &language, Some(&slug), None).await {
        Ok(page) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(page),
                error: None,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<PageResponse> {
                success: false,
                data: None,
                error: Some("Page not found".to_string()),
            }),
        )
            .into_response(),
    }
}

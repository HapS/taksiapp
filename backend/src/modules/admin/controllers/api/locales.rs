use crate::app_state::AppState;
use crate::modules::admin::services::locale_service;
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use std::collections::HashMap;
use tower_sessions::Session;

#[derive(Debug, Deserialize)]
pub struct UpdateLocaleRequest {
    pub key: String,
    pub old_key: Option<String>,
    pub translations: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct LocaleQueryParams {
    theme: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<LocaleQueryParams>,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return (StatusCode::FORBIDDEN, "Unauthorized").into_response();
    }

    let config = crate::config::get_config();
    let supported_langs: Vec<String> = config.supported_languages.keys().cloned().collect();
    let theme = params.theme.unwrap_or_else(|| "admin".to_string());

    match locale_service::get_locales(&supported_langs, &theme) {
        Ok(data) => {
            tracing::debug!("Loaded {} locale keys for theme: {}", data.keys.len(), theme);
            (StatusCode::OK, Json(data)).into_response()
        }
        Err(e) => {
            tracing::error!("Error loading locales: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

pub async fn update(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<LocaleQueryParams>,
    Json(payload): Json<UpdateLocaleRequest>,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return (StatusCode::FORBIDDEN, "Unauthorized").into_response();
    }

    let config = crate::config::get_config();
    let supported_langs: Vec<String> = config.supported_languages.keys().cloned().collect();
    let theme = params.theme.unwrap_or_else(|| "admin".to_string());

    // If old_key is provided and different, delete the old one first
    if let Some(old_key) = payload.old_key {
        if old_key != payload.key {
            if let Err(e) = locale_service::delete_key(&old_key, &supported_langs, &theme) {
                return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
            }
        }
    }

    match locale_service::update_key(payload.key, payload.translations, &supported_langs, &theme) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "success"})),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

pub async fn delete(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<LocaleQueryParams>,
    Json(payload): Json<HashMap<String, String>>,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return (StatusCode::FORBIDDEN, "Unauthorized").into_response();
    }

    let key = match payload.get("key") {
        Some(k) => k,
        None => return (StatusCode::BAD_REQUEST, "Missing key").into_response(),
    };

    let config = crate::config::get_config();
    let supported_langs: Vec<String> = config.supported_languages.keys().cloned().collect();
    let theme = params.theme.unwrap_or_else(|| "admin".to_string());

    match locale_service::delete_key(key, &supported_langs, &theme) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "success"})),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

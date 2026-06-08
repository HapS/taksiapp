// Vocabulary API Controller - JSON REST endpoints
use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::controllers::web::homepage::clear_homepage_cache;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::taxonomy::helpers::{
    CreateVocabularyRequest, UpdateVocabularyRequest, VocabularyResponse,
};
use crate::modules::taxonomy::services::vocabulary_service;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct VocabularyQueryParams {
    #[serde(rename = "type")]
    pub vocabulary_type: Option<String>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

// Use common RBAC helper
/// API: List all vocabularies with optional type filter
pub async fn list(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(params): Query<VocabularyQueryParams>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match vocabulary_service::list_vocabularies(
        &state.db,
        params.search,
        params.sort_by,
        params.sort_order,
        "tr",
    )
    .await
    {
        Ok(mut vocabularies) => {
            // Type filtresi varsa uygula
            if let Some(vocab_type) = params.vocabulary_type {
                vocabularies.retain(|v| v.vocabulary_type == vocab_type);
            }

            let response = ApiResponse {
                success: true,
                data: Some(vocabularies),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<Vec<VocabularyResponse>> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

pub async fn list_by_type(
    State(state): State<AppState>,
    Path(vocabulary_type): Path<String>,
    auth_user: AuthenticatedUser,
    Query(params): Query<VocabularyQueryParams>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match vocabulary_service::list_vocabularies_by_type(
        &state.db,
        &vocabulary_type,
        params.search,
        params.sort_by,
        params.sort_order,
        "tr",
    )
    .await
    {
        Ok(vocabularies) => {
            let response = ApiResponse {
                success: true,
                data: Some(vocabularies),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<Vec<VocabularyResponse>> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// API: Get vocabulary by ID
pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    auth_user: AuthenticatedUser,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match vocabulary_service::get_vocabulary_by_id(&state.db, id, "tr").await {
        Ok(vocabulary) => {
            let response = ApiResponse {
                success: true,
                data: Some(vocabulary),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<VocabularyResponse> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::NOT_FOUND, Json(response)).into_response()
        }
    }
}

/// API: Create vocabulary
pub async fn create(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    payload: Result<Json<CreateVocabularyRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    // Validate JSON body parse errors and return a friendly message when deserialization fails
    let json = match payload {
        Ok(Json(json)) => json,
        Err(rej) => {
            // Provide a concise, user-friendly error for enum/field issues
            let msg = if rej.to_string().contains("unknown variant")
                || rej.to_string().contains("unknown field")
            {
                "Invalid value for 'vocabulary_type'. Accepted values: tag, category, product_attributes, menu, options.".to_string()
            } else {
                format!("Failed to parse request body: {}", rej)
            };
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some(msg),
            };
            return (StatusCode::BAD_REQUEST, Json(response)).into_response();
        }
    };

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    clear_homepage_cache(&state);

    match vocabulary_service::create_vocabulary_with_cache_refresh(
        &state.db,
        json,
        &state.global_context_cache,
    )
    .await
    {
        Ok(vocabulary) => {
            let response = ApiResponse {
                success: true,
                data: Some(vocabulary),
                error: None,
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<crate::modules::taxonomy::models::vocabulary::Model> =
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// API: Update vocabulary
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    auth_user: AuthenticatedUser,
    payload: Result<Json<UpdateVocabularyRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    // Validate JSON body parse errors and return a friendly message when deserialization fails
    let json = match payload {
        Ok(Json(json)) => json,
        Err(rej) => {
            let msg = if rej.to_string().contains("unknown variant")
                || rej.to_string().contains("unknown field")
            {
                "Invalid value for 'vocabulary_type'. Accepted values: tag, category, product_attributes, menu, options.".to_string()
            } else {
                format!("Failed to parse request body: {}", rej)
            };
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some(msg),
            };
            return (StatusCode::BAD_REQUEST, Json(response)).into_response();
        }
    };

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    clear_homepage_cache(&state);

    match vocabulary_service::update_vocabulary_with_cache_refresh(
        &state.db,
        id,
        json,
        &state.global_context_cache,
    )
    .await
    {
        Ok(vocabulary) => {
            let response = ApiResponse {
                success: true,
                data: Some(vocabulary),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<crate::modules::taxonomy::models::vocabulary::Model> =
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// API: Delete vocabulary
pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    auth_user: AuthenticatedUser,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match vocabulary_service::delete_vocabulary_with_cache_refresh(
        &state.db,
        id,
        &state.global_context_cache,
    )
    .await
    {
        Ok(_) => {
            let response: ApiResponse<()> = ApiResponse {
                success: true,
                data: None,
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// API: Update vocabulary order
#[derive(serde::Deserialize)]
pub struct UpdateOrderRequest {
    pub orders: Vec<OrderItem>,
}

#[derive(serde::Deserialize)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i32,
}

pub async fn update_order(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(json): Json<UpdateOrderRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let orders: Vec<(i64, i32)> = json
        .orders
        .into_iter()
        .map(|item| (item.id, item.order_id))
        .collect();

    match vocabulary_service::update_vocabulary_order(&state.db, orders).await {
        Ok(_) => {
            let response: ApiResponse<()> = ApiResponse {
                success: true,
                data: None,
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

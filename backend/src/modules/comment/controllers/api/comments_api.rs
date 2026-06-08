use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::comment::services::comment_service::CommentService;
use crate::modules::utils::ip_helper::get_client_ip_from_headers;
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Json, Response},
    Extension,
};
use axum::http::HeaderMap;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub content_type: String,
    pub content_id: i64,
    pub content: String,
    #[serde(default = "default_star")]
    pub star: i32,
}

fn default_star() -> i32 {
    5
}

// #[derive(Debug, Deserialize)]
// pub struct UpdateCommentRequest {
//     pub content: String,
//     pub star: Option<i32>,
// }

#[derive(Debug, Deserialize)]
pub struct ListCommentsQuery {
    pub content_type: String,
    pub content_id: i64,
    pub published_only: Option<bool>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

pub async fn list_comments(
    State(state): State<AppState>,
    Extension(current_language): Extension<crate::middleware::global_context::CurrentLanguage>,
    Query(query): Query<ListCommentsQuery>,
) -> Json<serde_json::Value> {
    let published_only = query.published_only.unwrap_or(true);
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    match CommentService::list_comments(
        &state.db,
        &current_language.0,
        &query.content_type,
        query.content_id,
        published_only,
        page,
        per_page,
    )
    .await
    {
        Ok(result) => Json(serde_json::json!({
            "status": "success",
            "data": result.data,
            "pagination": result.pagination
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

pub async fn get_comment(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    match CommentService::get_comment(&state.db, id).await {
        Ok(Some(comment)) => Json(serde_json::json!({
            "status": "success",
            "data": comment
        })),
        Ok(None) => Json(serde_json::json!({
            "status": "error",
            "message": "Yorum bulunamadı."
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

pub async fn create_comment(
    State(state): State<AppState>,
    Extension(current_language): Extension<crate::middleware::global_context::CurrentLanguage>,
    headers: HeaderMap,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateCommentRequest>,
) -> Response {
    let ip_address = get_client_ip_from_headers(&headers);

    match CommentService::create_comment(
        &state.db,
        auth_user.id,
        current_language.0.clone(),
        payload.content_type,
        payload.content_id,
        payload.content,
        payload.star,
        ip_address,
    )
    .await
    {
        Ok(comment) => Json(serde_json::json!({
            "status": "success",
            "data": comment
        }))
        .into_response(),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        }))
        .into_response(),
    }
}

// pub async fn update_comment(
//     State(state): State<AppState>,
//     Path(id): Path<i64>,
//     auth_user: AuthenticatedUser,
//     Json(payload): Json<UpdateCommentRequest>,
// ) -> Response {
//     match CommentService::update_comment(&state.db, id, auth_user.id, payload.content, payload.star).await {
//         Ok(comment) => Json(serde_json::json!({
//             "status": "success",
//             "data": comment
//         })).into_response(),
//         Err(e) => Json(serde_json::json!({
//             "status": "error",
//             "message": e.to_string()
//         })).into_response(),
//     }
// }

pub async fn delete_comment(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    auth_user: AuthenticatedUser,
) -> Json<serde_json::Value> {
    match CommentService::delete_comment(&state.db, id, auth_user.id).await {
        Ok(_) => Json(serde_json::json!({
            "status": "success",
            "message": "Comment deleted successfully"
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

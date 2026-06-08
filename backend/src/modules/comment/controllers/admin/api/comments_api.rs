use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::comment::services::comment_service::CommentService;
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListCommentsQuery {
    pub content_type: Option<String>,
    pub content_id: Option<i64>,
    pub lang: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub search: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub include_unpublished: Option<bool>,
}

pub async fn list_comments(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<ListCommentsQuery>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let lang = query.lang.clone().unwrap_or_else(|| "tr".to_string());
    let include_unpublished = query.include_unpublished.unwrap_or(false);

    match CommentService::admin_list_comments(
        &state.db,
        &lang,
        query.content_type.as_deref(),
        query.content_id,
        query.search.as_deref(),
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        include_unpublished,
        page,
        per_page,
    )
    .await
    {
        Ok((comments, total)) => {
            let total_pages = if total == 0 {
                1
            } else {
                (total + per_page - 1) / per_page
            };

            Json(serde_json::json!({
                "status": "success",
                "data": comments,
                "meta": {
                    "total": total,
                    "page": page,
                    "per_page": per_page,
                    "total_pages": total_pages
                }
            }))
            .into_response()
        }
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": e.to_string()
        }))
        .into_response(),
    }
}

pub async fn delete_comment(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match CommentService::admin_delete_comment(&state.db, id).await {
        Ok(_) => Json(serde_json::json!({
            "status": "success",
            "message": "Yorum başarıyla silindi."
        }))
        .into_response(),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": e.to_string()
        }))
        .into_response(),
    }
}

pub async fn toggle_publish(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match CommentService::admin_toggle_publish(&state.db, id).await {
        Ok(comment) => Json(serde_json::json!({
            "status": "success",
            "data": comment
        }))
        .into_response(),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "error": e.to_string()
        }))
        .into_response(),
    }
}

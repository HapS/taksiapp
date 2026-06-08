// Admin User API Controllers - JSON responses for AJAX calls
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::auth::services::auth_service;
use sea_orm::*;

// Helper function to get user roles
async fn get_user_roles(db: &DatabaseConnection, user_id: i64) -> Result<Vec<UserRoleInfo>, DbErr> {
    use crate::modules::auth::models::{role, user_role};

    let roles = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id))
        .find_also_related(role::Entity)
        .all(db)
        .await?;

    let role_infos: Vec<UserRoleInfo> = roles
        .into_iter()
        .filter_map(|(_, role_opt)| {
            role_opt.map(|role| UserRoleInfo {
                id: role.id,
                name: role.name,
                description: role.description,
            })
        })
        .collect();

    Ok(role_infos)
}

// Helper function to check if user has admin access
async fn check_user_admin_access(db: &DatabaseConnection, user_id: i64) -> bool {
    use crate::modules::auth::models::User;

    match User::find_by_id(user_id).one(db).await {
        Ok(Some(user)) => match user.has_permission(db, "system.admin_access").await {
            Ok(has_access) => has_access,
            Err(_) => false,
        },
        Ok(None) => false,
        Err(_) => false,
    }
}
// ============ API QUERY/RESPONSE MODELS ============

#[derive(Deserialize)]
pub struct UserQueryParams {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub search: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub is_guest: Option<bool>,
}

#[derive(Serialize)]
pub struct UserListResponse {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub phone_country_code: Option<String>,
    pub profile: Option<serde_json::Value>,
    pub roles: Option<Vec<UserRoleInfo>>,
    pub has_admin_access: bool,
    pub is_guest: bool,
    pub created_at: Option<String>,
}

#[derive(Serialize)]
pub struct UserRoleInfo {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedUsersResponse {
    pub data: Vec<UserListResponse>,
    pub meta: PaginationMeta,
}

#[derive(Serialize)]
pub struct PaginationMeta {
    pub total: u64,
    pub page: u64,
    pub limit: u64,
    pub total_pages: u64,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub phone_country_code: Option<String>,
    pub profile: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub username: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone_number: Option<String>,
    pub phone_country_code: Option<String>,
    pub profile: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdatePasswordRequest {
    pub password: String,
}

// ============ API ENDPOINTS ============

/// API: List users with pagination
pub async fn list_users(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<UserQueryParams>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let (users_raw, total) = match auth_service::list_users(
        &state.db,
        page,
        limit,
        query.search.as_deref(),
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        query.is_guest,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Veritabanı hatası"
                })),
            )
                .into_response();
        }
    };

    // Convert to response format
    let mut users: Vec<UserListResponse> = Vec::new();

    for u in users_raw {
        let roles = get_user_roles(&state.db, u.id).await.unwrap_or_default();
        let has_admin_access = check_user_admin_access(&state.db, u.id).await;

        users.push(UserListResponse {
            id: u.id,
            username: u.username,
            email: u.email,
            first_name: u.first_name,
            last_name: u.last_name,
            phone_number: u.phone_number,
            phone_country_code: u.phone_country_code,
            profile: u.profile,
            roles: if roles.is_empty() { None } else { Some(roles) },
            has_admin_access,
            is_guest: u.is_guest,
            created_at: u
                .created_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
        });
    }

    let total_pages = (total + limit - 1) / limit;

    let response = PaginatedUsersResponse {
        data: users,
        meta: PaginationMeta {
            total,
            page,
            limit,
            total_pages,
        },
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// API: Get user by ID
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    auth_user: AuthenticatedUser,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match auth_service::get_user_by_id(&state.db, user_id).await {
        Ok(user) => {
            let roles = get_user_roles(&state.db, user.id).await.unwrap_or_default();
            let has_admin_access = check_user_admin_access(&state.db, user.id).await;

            let response = UserListResponse {
                id: user.id,
                username: user.username,
                email: user.email,
                first_name: user.first_name,
                last_name: user.last_name,
                phone_number: user.phone_number,
                phone_country_code: user.phone_country_code,
                profile: user.profile,
                roles: if roles.is_empty() { None } else { Some(roles) },
                has_admin_access,
                is_guest: user.is_guest,
                created_at: user
                    .created_at
                    .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "User not found"
            })),
        )
            .into_response(),
    }
}

/// API: Create user
pub async fn create_user(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateUserRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match auth_service::create_user(
        &state.db,
        &payload.username,
        &payload.email,
        &payload.password,
        payload.first_name,
        payload.last_name,
        payload.phone_number,
        payload.phone_country_code,
        payload.profile,
    )
    .await
    {
        Ok(user) => {
            let roles = get_user_roles(&state.db, user.id).await.unwrap_or_default();
            let has_admin_access = check_user_admin_access(&state.db, user.id).await;

            let response = UserListResponse {
                id: user.id,
                username: user.username,
                email: user.email,
                first_name: user.first_name,
                last_name: user.last_name,
                phone_number: user.phone_number,
                phone_country_code: user.phone_country_code,
                profile: user.profile,
                roles: if roles.is_empty() { None } else { Some(roles) },
                has_admin_access,
                is_guest: user.is_guest,
                created_at: user
                    .created_at
                    .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

/// API: Update user
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<UpdateUserRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match auth_service::update_user(
        &state.db,
        user_id,
        payload.username,
        payload.email,
        payload.first_name,
        payload.last_name,
        payload.phone_number,
        payload.phone_country_code,
        payload.profile,
    )
    .await
    {
        Ok(user) => {
            let roles = get_user_roles(&state.db, user.id).await.unwrap_or_default();
            let has_admin_access = check_user_admin_access(&state.db, user.id).await;

            let response = UserListResponse {
                id: user.id,
                username: user.username,
                email: user.email,
                first_name: user.first_name,
                last_name: user.last_name,
                phone_number: user.phone_number,
                phone_country_code: user.phone_country_code,
                profile: user.profile,
                roles: if roles.is_empty() { None } else { Some(roles) },
                has_admin_access,
                is_guest: user.is_guest,
                created_at: user
                    .created_at
                    .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

/// API: Update user password
pub async fn update_password(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<UpdatePasswordRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match auth_service::update_user_password(&state.db, user_id, &payload.password).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "Password updated successfully"
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

/// API: Delete user
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    auth_user: AuthenticatedUser,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match auth_service::delete_user(&state.db, user_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "User deleted successfully"
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

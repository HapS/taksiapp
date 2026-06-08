use crate::app_state::AppState;
use crate::modules::auth::services::permission_service;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::middleware::auth::AuthenticatedUser;


#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

// Use common RBAC helper
/// API: List all roles
pub async fn list_roles(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }


    match permission_service::list_roles(&state.db).await {
        Ok(roles) => {
            let response = ApiResponse {
                success: true,
                data: Some(roles),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<Vec<crate::modules::auth::models::RoleModel>> =
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// API: Get user's roles
pub async fn get_user_roles(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<i64>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }
    match permission_service::get_user_roles(&state.db, user_id).await {
        Ok(roles) => {
            let response = ApiResponse {
                success: true,
                data: Some(roles),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<Vec<crate::modules::auth::models::RoleModel>> =
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AssignRoleRequest {
    pub role_id: i64,
}

/// API: Assign role to user
pub async fn assign_role(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<i64>,
    Json(req): Json<AssignRoleRequest>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }


    match permission_service::assign_role_to_user(&state.db, user_id, req.role_id).await {
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

/// API: Remove role from user
pub async fn remove_role(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((user_id, role_id)): Path<(i64, i64)>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match permission_service::remove_role_from_user(&state.db, user_id, role_id).await {
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

/// API: Get user's permissions
pub async fn get_user_permissions(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<i64>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match permission_service::get_user_permissions(&state.db, user_id).await {
        Ok(permissions) => {
            let response = ApiResponse {
                success: true,
                data: Some(permissions),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<Vec<crate::modules::auth::models::PermissionModel>> =
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// API: List all permissions grouped by module
pub async fn list_permissions_by_module(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match permission_service::list_permissions_by_module(&state.db).await {
        Ok(permissions) => {
            let response = ApiResponse {
                success: true,
                data: Some(permissions),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<
                std::collections::BTreeMap<
                    String,
                    Vec<crate::modules::auth::models::PermissionModel>,
                >,
            > = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GrantPermissionRequest {
    pub permission_id: i64,
}

/// API: Grant permission to user (override)
pub async fn grant_permission(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<i64>,
    Json(req): Json<GrantPermissionRequest>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match permission_service::grant_permission_to_user(&state.db, user_id, req.permission_id).await
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

/// API: Revoke permission from user (deny override)
pub async fn revoke_permission(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((user_id, permission_id)): Path<(i64, i64)>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match permission_service::revoke_permission_from_user(&state.db, user_id, permission_id).await
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

/// API: Remove permission override (let role permissions take effect)
pub async fn remove_permission_override(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((user_id, permission_id)): Path<(i64, i64)>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match permission_service::remove_user_permission_override(&state.db, user_id, permission_id)
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

/// API: Get user's permission overrides
pub async fn get_user_permission_overrides(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(user_id): Path<i64>,
) -> Response {

    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match permission_service::get_user_permission_overrides(&state.db, user_id).await {
        Ok(overrides) => {
            let response = ApiResponse {
                success: true,
                data: Some(overrides),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<
                Vec<(crate::modules::auth::models::PermissionModel, bool)>,
            > = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}




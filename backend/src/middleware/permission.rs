use crate::app_state::AppState;
use crate::middleware::auth::verify_auth;
use crate::modules::auth::models::User;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use sea_orm::EntityTrait;
use tower_sessions::Session;

/// Middleware to check if user has a specific permission
#[allow(dead_code)]
pub async fn require_permission(
    permission: &'static str,
) -> impl Fn(
    State<AppState>,
    Session,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
       + Clone {
    move |state: State<AppState>, session: Session, request: Request, next: Next| {
        Box::pin(async move {
            check_permission_middleware(state, session, request, next, permission).await
        })
    }
}

#[allow(dead_code)]
async fn check_permission_middleware(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
    permission: &str,
) -> Response {
    // Get user from auth verify (JWT or Session)
    let user_id = match verify_auth(request.headers(), &session).await {
        Some(id) => id,
        None => {
            return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
        }
    };

    // Get user from database
    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        _ => {
            return (StatusCode::UNAUTHORIZED, "User not found").into_response();
        }
    };

    // Check permission
    match user.has_permission(&state.db, permission).await {
        Ok(true) => next.run(request).await,
        Ok(false) => (
            StatusCode::FORBIDDEN,
            format!("Permission denied: {}", permission),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Permission check failed: {}", e),
        )
            .into_response(),
    }
}

/// Middleware to check if user has any of the given permissions
#[allow(dead_code)]
pub async fn require_any_permission(
    permissions: &'static [&'static str],
) -> impl Fn(
    State<AppState>,
    Session,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
       + Clone {
    move |state: State<AppState>, session: Session, request: Request, next: Next| {
        Box::pin(async move {
            check_any_permission_middleware(state, session, request, next, permissions).await
        })
    }
}

#[allow(dead_code)]
async fn check_any_permission_middleware(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
    permissions: &[&str],
) -> Response {
    // Get user from auth verify (JWT or Session)
    let user_id = match verify_auth(request.headers(), &session).await {
        Some(id) => id,
        None => {
            return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
        }
    };

    // Get user from database
    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        _ => {
            return (StatusCode::UNAUTHORIZED, "User not found").into_response();
        }
    };

    // Check if user has any of the permissions
    match user.has_any_permission(&state.db, permissions).await {
        Ok(true) => next.run(request).await,
        Ok(false) => (
            StatusCode::FORBIDDEN,
            format!("Permission denied. Required one of: {:?}", permissions),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Permission check failed: {}", e),
        )
            .into_response(),
    }
}

/// Middleware to check if user has a specific role
#[allow(dead_code)]
pub async fn require_role(
    role: &'static str,
) -> impl Fn(
    State<AppState>,
    Session,
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
       + Clone {
    move |state: State<AppState>, session: Session, request: Request, next: Next| {
        Box::pin(async move { check_role_middleware(state, session, request, next, role).await })
    }
}

#[allow(dead_code)]
async fn check_role_middleware(
    State(state): State<AppState>,
    session: Session,
    request: Request,
    next: Next,
    role: &str,
) -> Response {
    // Get user from auth verify (JWT or Session)
    let user_id = match verify_auth(request.headers(), &session).await {
        Some(id) => id,
        None => {
            return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
        }
    };

    // Get user from database
    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        _ => {
            return (StatusCode::UNAUTHORIZED, "User not found").into_response();
        }
    };

    // Check role
    match user.has_role(&state.db, role).await {
        Ok(true) => next.run(request).await,
        Ok(false) => (StatusCode::FORBIDDEN, format!("Role required: {}", role)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Role check failed: {}", e),
        )
            .into_response(),
    }
}

/// Helper function to check permission in controllers
/// Controller seviyesinde, bir işlemi yapmadan önce “bu kullanıcı bu işlemi yapabilir mi?” sorusunun cevabını verir ve uygun şekilde işlem yapılmasını sağlar.
#[allow(dead_code)]
pub async fn check_permission(
    state: &AppState,
    session: &Session,
    permission: &str,
) -> Result<(), Response> {
    // Only Session based for now as controllers should use AuthenticatedUser extractor usually
    let user_id = match session.get::<i64>("user_id").await {
        Ok(Some(id)) => id,
        _ => {
            return Err((StatusCode::UNAUTHORIZED, "Not authenticated").into_response());
        }
    };

    // Get user from database
    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        _ => {
            return Err((StatusCode::UNAUTHORIZED, "User not found").into_response());
        }
    };

    // Check permission
    match user.has_permission(&state.db, permission).await {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            StatusCode::FORBIDDEN,
            format!("Permission denied: {}", permission),
        )
            .into_response()),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Permission check failed: {}", e),
        )
            .into_response()),
    }
}

/// Helper function to check if user owns the resource
#[allow(dead_code)]
pub async fn check_ownership_or_permission(
    state: &AppState,
    session: &Session,
    resource_user_id: i64,
    own_permission: &str,
    any_permission: &str,
) -> Result<(), Response> {
    // Only Session based for now
    let user_id = match session.get::<i64>("user_id").await {
        Ok(Some(id)) => id,
        _ => {
            return Err((StatusCode::UNAUTHORIZED, "Not authenticated").into_response());
        }
    };

    // Get user from database
    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        _ => {
            return Err((StatusCode::UNAUTHORIZED, "User not found").into_response());
        }
    };

    // Check if user can edit ANY resource
    match user.has_permission(&state.db, any_permission).await {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Permission check failed: {}", e),
            )
                .into_response())
        }
    }

    // Check if user can edit OWN resource and owns it
    match user.has_permission(&state.db, own_permission).await {
        Ok(true) => {
            if user_id == resource_user_id {
                return Ok(());
            } else {
                return Err((StatusCode::FORBIDDEN, "Not your resource").into_response());
            }
        }
        Ok(false) => {}
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Permission check failed: {}", e),
            )
                .into_response())
        }
    }

    Err((StatusCode::FORBIDDEN, "Permission denied").into_response())
}

// Admin User Web Controllers - HTML views
use crate::app_state::AppState;
use crate::modules::auth::services::auth_service;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// User list page
pub async fn user_list(State(state): State<AppState>, session: Session) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Kullanıcı Yönetimi");
    context.insert("current_path", "/admin/users");

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/auth/user_list.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// User create page
pub async fn user_create(State(state): State<AppState>, session: Session) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Yeni Kullanıcı Ekle");
    context.insert("current_path", "/admin/users/new");

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/auth/user_add_edit.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// User edit page
pub async fn user_edit(
    State(state): State<AppState>,
    session: Session,
    Path(user_id): Path<i64>,
) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let user = match auth_service::get_user_by_id(&state.db, user_id).await {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "User not found").into_response();
        }
    };

    let mut context = Context::new();
    context.insert("title", "Kullanıcı Düzenle");
    context.insert("current_path", &format!("/admin/users/{}", user_id));
    context.insert("user_id", &user_id);
    // Raw user data'yı gönder - Vue.js parse edecek
    context.insert("user_data", &serde_json::to_value(&user).unwrap());

    // Add session user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/auth/user_add_edit.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

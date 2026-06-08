use crate::app_state::AppState;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

/// GET /admin/orders - Sipariş listesi
pub async fn orders_list(State(state): State<AppState>, session: Session) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Sipariş Yönetimi");
    context.insert("current_path", "/admin/orders");

    // User bilgilerini ekle
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/orders/order_list.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// GET /admin/orders/{id} - Sipariş düzenleme
pub async fn order_edit(
    State(state): State<AppState>,
    Path(order_id): Path<i64>,
    session: Session,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Sipariş Düzenle");
    context.insert("current_path", "/admin/orders");
    context.insert("order_id", &order_id);

    // User bilgilerini ekle
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/orders/order_edit.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

// Helper: Admin kontrolü
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

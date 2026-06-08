// Admin Page Web Controllers - HTML views
use crate::app_state::AppState;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

// Helper: Admin kontrolü
// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

// Admin sayfa listesi
pub async fn products_import(State(state): State<AppState>, session: Session) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Toplu Ürün Yükleme");

    // Query string'li current_path oluştur
    let current_path = "/admin/contents/products-import".to_string();
    let active_menu = String::from("contents:all");

    context.insert("current_path", &current_path);
    context.insert("active_menu", &active_menu);

    // User bilgilerini ekle
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/contents/products_import.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode, otherwise return generic 500
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

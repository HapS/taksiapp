use crate::app_state::AppState;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// GET /admin/returns - İade talepleri listesi sayfası
pub async fn returns_list(State(state): State<AppState>, session: Session) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "İade Talepleri Yönetimi");
    context.insert("current_path", "/admin/returns");

    // User bilgilerini ekle
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/returns/return_list.html", &context) {
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

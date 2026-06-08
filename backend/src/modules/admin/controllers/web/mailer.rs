// Admin Mailer Web Controllers - HTML views
use crate::app_state::AppState;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

// Admin mail kuyruğu listesi
pub async fn admin_mailer_list(State(state): State<AppState>, session: Session) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Mail Kuyruğu Yönetimi");
    context.insert("current_path", "/admin/mailer");

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/mailer/mail_queue_list.html", &context) {
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

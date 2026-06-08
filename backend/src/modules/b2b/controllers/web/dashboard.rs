use axum::{
    extract::{
        // Path,
        State,
    },
    // http::StatusCode,
    // Extension,
    response::{Html, IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::app_state::AppState;
// use crate::config;
use crate::middleware::global_context::ViewContext;

// use crate::config;
// use crate::modules::b2b::services::product_service;

// use sea_orm::EntityTrait;
// use tera::Context;
// use tower_sessions::Session;
// use crate::modules::b2b::helpers::language_helper::{validate_language, LanguageValidation};
// use rust_i18n::t;

// use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use crate::modules::auth::helpers::rbac::has_b2b_access as is_b2b;

pub async fn b2b_home(
    State(state): State<AppState>,
    session: Session,
    mut ctx: ViewContext,
    // _auth: crate::middleware::auth::AuthenticatedUser,
) -> Response {
    // let config = config::get_config();

    // kullanıcı tipi B2B değilse yönlendir, burada işi yok
    if !is_b2b(&state, &session).await {
        return Redirect::to("/").into_response();
    }

    ctx.0.insert("title", "Bayi Paneli");
    ctx.0.insert("request_path", "/my-account/b2b");

    // Template render et - standart error handling ile
    match state.render_frontend_template("auth/my_account/b2b.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            return crate::middleware::error_handler::handle_template_error(
                &e,
                state.config.is_debug(),
            );
        }
    }
}

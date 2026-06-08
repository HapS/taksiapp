use crate::app_state::AppState;
use crate::middleware::global_context::ViewContext;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
    Extension,
};
use rust_i18n::t;

/// Profile sayfası
pub async fn profile_page(
    State(state): State<AppState>,
    mut context: ViewContext,
    Extension(user_id): Extension<Option<i64>>,
) -> Response {
    context.0.insert(
        "title",
        &t!(
            "page_title_profile",
            locale = &state.config.default_language
        ),
    );
    context.0.insert("request_path", "/my-account/profile");

    //verify auth kullanılarak yapılabilir
    if user_id.is_none() {
        return Redirect::to("/login").into_response();
    }

    match state.render_frontend_template("auth/my_account/profile.html", &context.0) {
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

/// My Account Home Page
pub async fn home_page(
    State(state): State<AppState>,
    mut context: ViewContext,
    Extension(user_id): Extension<Option<i64>>,
) -> Response {
    context.0.insert(
        "title",
        &t!(
            "page_title_my_account",
            locale = &state.config.default_language
        ),
    );
    context.0.insert("request_path", "/my-account");

    if user_id.is_none() {
        return Redirect::to("/login").into_response();
    }

    match state.render_frontend_template("auth/my_account/index.html", &context.0) {
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

pub async fn my_bookmarks(
    State(state): State<AppState>,
    mut context: ViewContext,
    _auth: crate::middleware::auth::AuthenticatedUser,
) -> Response {
    context.0.insert("title", "Favorilerim");
    context.0.insert("request_path", "/my-account/bookmarks");

    // No login check needed, AuthenticatedUser handles guest creation if needed

    match state.render_frontend_template("auth/my_account/bookmarks.html", &context.0) {
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

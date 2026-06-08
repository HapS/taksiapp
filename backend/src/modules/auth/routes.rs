use super::controllers::{api as api_controllers, web as web_controllers};
use crate::app_state::AppState;
use axum::{
    routing::{get, post},
    Router,
};

// Auth app URL patterns - Axum router
pub fn routes() -> Router<AppState> {
    Router::new()
        // Web: Auth HTML Pages
        .route("/login", get(web_controllers::login_page))
        .route("/register", get(web_controllers::register_page))
        .route(
            "/forgot-password",
            get(web_controllers::forgot_password_page),
        )
        .route("/reset-password", get(web_controllers::reset_password_page))
        .route("/my-account", get(web_controllers::my_account::home_page))
        .route(
            "/my-account/profile",
            get(web_controllers::my_account::profile_page),
        )
        .route(
            "/my-account/bookmarks",
            get(web_controllers::my_account::my_bookmarks),
        )
        // Web: Auth Form Actions
        .route("/login", post(web_controllers::login_form))
        .route("/register", post(web_controllers::register_form))
        .route(
            "/forgot-password",
            post(web_controllers::forgot_password_form),
        )
        .route(
            "/reset-password",
            post(web_controllers::reset_password_form),
        )
        .route("/logout", post(web_controllers::logout))
        // Social Auth
        .route(
            "/auth/google/login",
            get(api_controllers::social_auth::google_login),
        )
        .route(
            "/auth/google/callback",
            get(api_controllers::social_auth::google_callback),
        )
        // Mobile API: JWT-based endpoints
        .route("/api/auth/login", post(api_controllers::auth::login))
        .route("/api/auth/register", post(api_controllers::auth::register))
        .route(
            "/api/auth/refresh",
            post(api_controllers::auth::refresh_token),
        )
        // API: User Profile Management (hem web hem mobil için)
        .route(
            "/api/user/profile",
            get(api_controllers::user_profile::get_profile),
        )
        .route(
            "/api/user/profile",
            post(api_controllers::user_profile::update_profile),
        )
        .route(
            "/api/user/change-password",
            post(api_controllers::user_profile::change_password),
        )
}

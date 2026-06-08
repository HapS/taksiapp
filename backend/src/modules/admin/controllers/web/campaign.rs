use crate::app_state::AppState;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

pub async fn campaign_list(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Kampanya Yönetimi");
    context.insert("current_path", "/admin/campaigns");
    context.insert("active_menu", "campaigns");

    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/campaigns/campaign_list.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            )
        }
    }
}

pub async fn campaign_create(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Yeni Kampanya");
    context.insert("current_path", "/admin/campaigns/new");
    context.insert("active_menu", "campaigns");
    context.insert("campaign_id", &None::<i64>);

    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/campaigns/campaign_form.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            )
        }
    }
}

pub async fn campaign_edit(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Kampanya Düzenle");
    context.insert("current_path", &format!("/admin/campaigns/{}", id));
    context.insert("active_menu", "campaigns");
    context.insert("campaign_id", &id);

    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/campaigns/campaign_form.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            )
        }
    }
}

pub async fn coupon_list(
    State(state): State<AppState>,
    session: Session,
    Path(campaign_id): Path<i64>,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Kupon Yönetimi");
    context.insert("current_path", &format!("/admin/campaigns/{}/coupons", campaign_id));
    context.insert("active_menu", "campaigns");
    context.insert("campaign_id", &campaign_id);

    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/campaigns/coupon_list.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            )
        }
    }
}
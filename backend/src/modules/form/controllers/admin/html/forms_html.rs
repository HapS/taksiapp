use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use crate::{app_state::AppState, middleware::global_context::ViewContext};
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
};

use tower_sessions::Session;

pub async fn form_list_page(
    State(state): State<AppState>,
    session: Session,
    mut ctx: ViewContext,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    ctx.0.insert("title", "Form Listesi");
    ctx.0.insert("current_path", "/admin/forms");

    match state.render_template("admin/forms/form_list.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            eprintln!("Template render error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Template render failed",
            )
                .into_response()
        }
    }
}

pub async fn form_details_page(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    session: Session,
    mut ctx: ViewContext,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }
    ctx.0.insert("title", "Form Detayları");
    ctx.0.insert("form_id", &id);
    ctx.0
        .insert("current_path", &format!("/admin/forms/{}", id));

    match state.render_template("admin/forms/form_details.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            eprintln!("Template render error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Template render failed",
            )
                .into_response()
        }
    }
}

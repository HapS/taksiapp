use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use crate::{app_state::AppState, middleware::global_context::ViewContext};
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
};
use tower_sessions::Session;
/// GET /admin/b2b/companies - Liste sayfası render
pub async fn company_list_page(
    State(state): State<AppState>,
    session: Session,
    mut ctx: ViewContext,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    ctx.0.insert("title", "B2B Şirket Yönetimi");
    ctx.0.insert("current_path", "/admin/b2b/companies");

    match state.render_template("admin/b2b/company_list.html", &ctx.0) {
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

/// GET /admin/b2b/companies/new - Yeni şirket sayfası render
pub async fn company_add_page(
    State(state): State<AppState>,
    session: Session,
    mut ctx: ViewContext,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    ctx.0.insert("title", "Yeni Şirket Ekle");
    ctx.0.insert("company_id", &None::<i64>);
    ctx.0.insert("current_path", "/admin/b2b/companies/new");

    match state.render_template("admin/b2b/company_add_edit.html", &ctx.0) {
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

/// GET /admin/b2b/companies/{id} - Şirket düzenle sayfası render
pub async fn company_edit_page(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    mut ctx: ViewContext,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    ctx.0.insert("title", "Şirket Düzenle");
    ctx.0.insert("company_id", &id);
    ctx.0
        .insert("current_path", &format!("/admin/b2b/companies/{}", id));

    match state.render_template("admin/b2b/company_add_edit.html", &ctx.0) {
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

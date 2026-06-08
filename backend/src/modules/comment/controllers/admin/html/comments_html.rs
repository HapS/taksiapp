use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use crate::{app_state::AppState, middleware::global_context::ViewContext};
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect},
};
use tower_sessions::Session;

pub async fn comment_list_page(
    State(state): State<AppState>,
    session: Session,
    mut ctx: ViewContext,
) -> impl IntoResponse {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    ctx.0.insert("title", "Yorum Yönetimi");
    ctx.0.insert("current_path", "/admin/comments");

    match state.render_template("admin/comments/comment_list.html", &ctx.0) {
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

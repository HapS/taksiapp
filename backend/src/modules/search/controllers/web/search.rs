use crate::app_state::AppState;
use crate::middleware::global_context::ViewContext;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub type_: Option<String>,
}

pub async fn search_page(
    State(state): State<AppState>,
    context: ViewContext,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let mut ctx = context.0;

    ctx.insert("title", "Arama Sonuçları");

    if let Some(q) = &params.q {
        ctx.insert("query", q);
    }

    if let Some(t) = &params.type_ {
        ctx.insert("selected_type", t);
    } else {
        ctx.insert("selected_type", "product");
    }

    match state.render_frontend_template("search/index.html", &ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            eprintln!("Template render error: {}", e);
            Redirect::to("/").into_response()
        }
    }
}

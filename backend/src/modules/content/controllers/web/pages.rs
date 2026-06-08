// Web Controllers - Pages list and detail
use crate::app_state::AppState;
use crate::config;
use crate::modules::content::helpers::page_helper;
use crate::modules::content::services::page_service;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};

use crate::middleware::global_context::ViewContext;
use crate::modules::content::helpers::language_helper::{validate_language, LanguageValidation};
use rust_i18n::t;

/// List all published pages
pub async fn list(
    State(state): State<AppState>,
    mut ctx: ViewContext,
    Path(language): Path<String>,
) -> Response {
    let config = config::get_config();

    // TODO: bu eski bunu değiştir
    let lang = match validate_language(&language, &config) {
        LanguageValidation::Valid(lang) => lang,
        LanguageValidation::ReservedPath => {
            return StatusCode::NOT_FOUND.into_response();
        }
        LanguageValidation::Unsupported { redirect_to } => {
            return Redirect::to(&redirect_to).into_response();
        }
    };

    let pages = match page_service::list_pages(&state.db, &lang).await {
        Ok(p) => p,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch pages").into_response();
        }
    };

    ctx.0.insert("title", &t!("pages_list", locale = lang));
    ctx.0.insert("pages", &pages);
    ctx.0.insert("request_path", &format!("/{}/pages", lang));

    match state.render_frontend_template("pages/pages_list.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode, otherwise return generic 500
            return crate::middleware::error_handler::handle_template_error(
                &e,
                state.config.is_debug(),
            );
        }
    }
}

/// Page detail by slug-id format (e.g., "my-article-123")
pub async fn detail(
    State(state): State<AppState>,
    mut ctx: ViewContext,
    Path((language, slug_id)): Path<(String, String)>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let config = config::get_config();

    // Validate the language parameter
    let lang = match validate_language(&language, &config) {
        LanguageValidation::Valid(lang) => lang,
        LanguageValidation::ReservedPath => {
            return StatusCode::NOT_FOUND.into_response();
        }
        LanguageValidation::Unsupported { redirect_to } => {
            return Redirect::to(&redirect_to).into_response();
        }
    };

    // Parse slug-id format: "my-article-123" -> slug="my-article", id=123
    // Find the last dash and split there
    let (slug, id) = match slug_id.rfind('-') {
        Some(pos) => {
            let (slug_part, id_part) = slug_id.split_at(pos);
            let id_str = &id_part[1..]; // Skip the dash
            match id_str.parse::<i64>() {
                Ok(id) => (slug_part.to_string(), id),
                Err(_) => {
                    return (StatusCode::BAD_REQUEST, "Invalid page ID format").into_response();
                }
            }
        }
        None => {
            return (StatusCode::BAD_REQUEST, "Invalid slug-id format").into_response();
        }
    };

    let page = match page_service::get_page(&state.db, &lang, Some(&slug), Some(id)).await {
        Ok(p) => p,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Page not found").into_response();
        }
    };

    // Check if slug matches the actual page slug
    if !page.slug.is_empty() && page.slug != slug {
        // Redirect to correct slug
        let correct_url = format!("/{}/page/{}-{}", lang, page.slug, id);
        return Redirect::permanent(&correct_url).into_response();
    }

    // Content navigation: parent + siblings (tam PageResponse olarak)
    let content_nav = page_helper::load_content_nav(&state.db, id, page.parent_id, &lang).await;

    ctx.0.insert("title", &page.title);
    ctx.0.insert("content_nav", &content_nav);
    ctx.0.insert("page", &page);
    ctx.0
        .insert("request_path", &format!("/{}/page/{}-{}", lang, slug, id));

    // ?json=true  Json response  tera ile aynı çıktığı verir  context i doğrudan json yapıyoruz
    if query.get("json").map(|v| v == "true").unwrap_or(false) {
        // Context'i serialize edilebilir bir map'e çevir
        let json_data: std::collections::HashMap<String, serde_json::Value> = ctx
            .0
            .into_json()
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        //  result.page.product sil // json_data mut yap öyle sil
        // if let Some(result_value) = json_data.get_mut("page") {
        //     if let Some(result_obj) = result_value.as_object_mut() {
        //         if let Some(data_value) = result_obj.get_mut("data") {
        //             if let Some(data_obj) = data_value.as_object_mut() {
        //                 data_obj.remove("product");
        //             }
        //         }
        //     }
        // }

        return axum::Json(json_data).into_response();
    }

    // Admin panelden seçilen template'i kullan, yoksa default
    let template_inner = if let Some(template) = &page.template {
        if !template.is_empty() {
            format!("pages/{}", template)
        } else {
            "pages/page_detail.html".to_string()
        }
    } else {
        // Default template
        "pages/page_detail.html".to_string()
    };

    println!("template path : {}", template_inner);

    match state.render_frontend_template(&template_inner, &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Prefer showing the debug template error page (or JSON if requested) instead of falling back
            let prefer_json = query.get("json").map(|v| v == "true").unwrap_or(false);
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                config.is_debug(),
                prefer_json,
                Some(&state),
            );
        }
    }
}

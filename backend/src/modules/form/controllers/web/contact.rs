// Web Controllers - Pages list and detail
use crate::app_state::AppState;
use crate::config;
use crate::modules::content::services::page_service;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};

use crate::middleware::global_context::ViewContext;
use crate::modules::content::helpers::language_helper::{validate_language, LanguageValidation};
// use rust_i18n::t;

pub async fn contact(
    State(state): State<AppState>,
    mut ctx: ViewContext,
    Path(language): Path<String>,
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

    let default_contact_form =
        crate::modules::admin::services::settings_service::get_settings(&state.db)
            .await
            .ok()
            .and_then(|settings| settings.default_contact_form);

    println!("default_contact_form id: {:?}", default_contact_form);

    let page = match page_service::get_page(&state.db, &lang, None, default_contact_form).await {
        Ok(p) => p,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Page not found").into_response();
        }
    };

    ctx.0.insert("title", &page.title);
    ctx.0.insert("page", &page);
    ctx.0
        .insert("request_path", &format!("/{}/form/contact", lang));

    // ?json=true  Json response  tera ile aynı çıktığı verir  context i doğrudan json yapıyoruz
    if query.get("json").map(|v| v == "true").unwrap_or(false) {
        // Context'i serialize edilebilir bir map'e çevir
        let json_data: std::collections::HashMap<String, serde_json::Value> = ctx
            .0
            .into_json()
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

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
            // Template bulunamazsa fallback template kullan
            if config.is_debug() {
                eprintln!("❌ Template error: {}", e);
                eprintln!("❌ Tried template: {}", template_inner);
                eprintln!("💡 Falling back to default template");
            }

            // Fallback to default template
            match state.render_frontend_template("pages/page_detail.html", &ctx.0) {
                Ok(html) => Html(html).into_response(),
                // Err(fallback_err) => crate::middleware::error_handler::handle_template_error(
                //     fallback_err,
                //     config.is_debug(),
                // ),
                Err(e) => {
                    // Final fallback failed — surface the raw Tera error page in debug mode
                    return crate::middleware::error_handler::handle_template_error(
                        &e,
                        config.is_debug(),
                    );
                }
            }
        }
    }
}

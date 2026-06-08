// Advanced Settings Controller
use crate::app_state::AppState;
use crate::modules::admin::models::settings::SettingsData;
use crate::modules::admin::services::settings_service;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::Multipart;
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// Advanced settings page - GET
pub async fn advanced_settings(State(state): State<AppState>, session: Session) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    // Get current settings
    let current_settings = match settings_service::get_settings(&state.db).await {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Settings error: {:?}", e);
            SettingsData::default()
        }
    };

    let mut context = Context::new();
    // context.insert("analytics_code", &current_settings.analytics_code.unwrap_or_default());
    // context.insert("custom_css", &current_settings.custom_css.unwrap_or_default());
    // context.insert("custom_js", &current_settings.custom_js.unwrap_or_default());

    // Vocabulary settings
    context.insert(
        "vocab_navbar_menu",
        &current_settings.vocab_navbar_menu.unwrap_or(1),
    );
    context.insert(
        "vocab_footer_menu",
        &current_settings.vocab_footer_menu.unwrap_or(2),
    );
    context.insert(
        "vocab_product_categories",
        &current_settings.vocab_product_categories.unwrap_or(3),
    );
    context.insert(
        "vocab_blog_categories",
        &current_settings.vocab_blog_categories.unwrap_or(4),
    );
    context.insert(
        "vocab_news_categories",
        &current_settings.vocab_news_categories.unwrap_or(5),
    );
    context.insert(
        "vocab_page_categories",
        &current_settings.vocab_page_categories.unwrap_or(6),
    );
    context.insert(
        "vocab_tags_categories",
        &current_settings.vocab_tags_categories.unwrap_or(7),
    );
    // Ödeme metodları için taxonomy vocabulary ID
    context.insert(
        "vocab_payment_methods",
        &current_settings.vocab_payment_methods.unwrap_or(12),
    );
    context.insert(
        "default_contact_form",
        &current_settings.default_contact_form.unwrap_or(90),
    );

    // Default content settings
    context.insert(
        "default_home_content_id",
        &current_settings.default_home_content_id.unwrap_or(70),
    );

    match super::render_settings_page(
        &state,
        "advanced",
        "Gelişmiş Ayarlar",
        "admin/settings/sections/advanced.html",
        context,
        None,
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Advanced settings page - POST
pub async fn update_advanced_settings(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    let mut form_data = std::collections::HashMap::new();

    // Parse multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if let Ok(value) = field.text().await {
            form_data.insert(name, value);
        }
    }

    // Get current settings
    let mut current_settings = match settings_service::get_settings(&state.db).await {
        Ok(settings) => settings,
        Err(_) => SettingsData::default(),
    };

    // Update analytics and custom code settings
    if let Some(value) = form_data.get("analytics_code") {
        current_settings.analytics_code = if value.is_empty() {
            None
        } else {
            Some(value.clone())
        };
    }
    if let Some(value) = form_data.get("custom_css") {
        current_settings.custom_css = if value.is_empty() {
            None
        } else {
            Some(value.clone())
        };
    }
    if let Some(value) = form_data.get("custom_js") {
        current_settings.custom_js = if value.is_empty() {
            None
        } else {
            Some(value.clone())
        };
    }

    // Update vocabulary settings
    if let Some(value) = form_data.get("vocab_navbar_menu") {
        current_settings.vocab_navbar_menu = value.parse().ok();
    }
    if let Some(value) = form_data.get("vocab_footer_menu") {
        current_settings.vocab_footer_menu = value.parse().ok();
    }
    if let Some(value) = form_data.get("vocab_product_categories") {
        current_settings.vocab_product_categories = value.parse().ok();
    }
    if let Some(value) = form_data.get("vocab_blog_categories") {
        current_settings.vocab_blog_categories = value.parse().ok();
    }
    if let Some(value) = form_data.get("vocab_news_categories") {
        current_settings.vocab_news_categories = value.parse().ok();
    }
    if let Some(value) = form_data.get("vocab_page_categories") {
        current_settings.vocab_page_categories = value.parse().ok();
    }
    if let Some(value) = form_data.get("vocab_tags_categories") {
        current_settings.vocab_tags_categories = value.parse().ok();
    }

    if let Some(value) = form_data.get("vocab_payment_methods") {
        current_settings.vocab_payment_methods = value.parse().ok();
    }

    if let Some(value) = form_data.get("default_contact_form") {
        current_settings.default_contact_form = value.parse().ok();
    }

    // Update default content settings
    if let Some(value) = form_data.get("default_home_content_id") {
        current_settings.default_home_content_id = value.parse().ok();
    }

    // Save settings
    match settings_service::update_settings(&state.db, current_settings).await {
        Ok(_) => {
            // Refresh settings cache
            if let Ok(new_settings_cache) =
                crate::middleware::global_context::SettingsCache::load_from_db(&state.db).await
            {
                if let Ok(mut cache) = state.settings_cache.write() {
                    *cache = new_settings_cache;
                }
            }

            Redirect::to("/admin/settings/advanced?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/advanced?error=1").into_response()
        }
    }
}

// SEO Settings Controller
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

/// SEO settings page - GET
pub async fn seo_settings(State(state): State<AppState>, session: Session) -> Response {
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

    let config = crate::config::get_config();
    let supported_languages = &config.supported_languages;

    // Prepare template settings - multi-language SEO fields
    let mut template_settings = serde_json::Map::new();
    let mut lang_data = serde_json::Map::new();

    // Create nested object for each language
    for (lang_code, _lang_name) in supported_languages {
        let mut lang_fields = serde_json::Map::new();
        lang_fields.insert(
            "seo_title".to_string(),
            serde_json::Value::String(
                current_settings
                    .get_lang_value("seo_title", lang_code)
                    .unwrap_or_default(),
            ),
        );
        lang_fields.insert(
            "seo_description".to_string(),
            serde_json::Value::String(
                current_settings
                    .get_lang_value("seo_description", lang_code)
                    .unwrap_or_default(),
            ),
        );
        lang_fields.insert(
            "seo_image".to_string(),
            serde_json::Value::String(
                current_settings
                    .get_lang_value("seo_image", lang_code)
                    .unwrap_or_default(),
            ),
        );

        lang_data.insert(lang_code.clone(), serde_json::Value::Object(lang_fields));
    }
    template_settings.insert("langs".to_string(), serde_json::Value::Object(lang_data));
    template_settings.insert(
        "robots".to_string(),
        serde_json::Value::String(
            current_settings
                .robots
                .unwrap_or_else(|| "User-agent: *\nAllow: /".to_string()),
        ),
    );

    let mut context = Context::new();
    context.insert("settings", &serde_json::Value::Object(template_settings));

    match super::render_settings_page(
        &state,
        "seo",
        "SEO Ayarları",
        "admin/settings/sections/seo.html",
        context,
        None,
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// SEO settings page - POST
pub async fn update_seo_settings(
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

    // Update multi-language SEO fields
    let config = crate::config::get_config();
    let supported_languages = &config.supported_languages;

    for (lang_code, _) in supported_languages {
        if let Some(value) = form_data.get(&format!("seo_title_{}", lang_code)) {
            current_settings.set_lang_value("seo_title", lang_code, value);
        }
        if let Some(value) = form_data.get(&format!("seo_description_{}", lang_code)) {
            current_settings.set_lang_value("seo_description", lang_code, value);
        }
        if let Some(value) = form_data.get(&format!("seo_image_{}", lang_code)) {
            current_settings.set_lang_value("seo_image", lang_code, value);
        }
    }

    // Update robots field
    if let Some(value) = form_data.get("robots") {
        current_settings.robots = Some(value.clone());
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

            Redirect::to("/admin/settings/seo?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/seo?error=1").into_response()
        }
    }
}

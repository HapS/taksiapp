// General Settings Controller
use crate::app_state::AppState;
use crate::modules::admin::models::settings::SettingsData;
use crate::modules::admin::services::settings_service;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::Multipart;
use std::collections::HashMap;
use std::path::Path;
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// General settings page - GET
pub async fn general_settings(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
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

    // Prepare template settings - multi-language fields with nested structure
    let mut template_settings = serde_json::Map::new();
    let mut lang_data = serde_json::Map::new();

    // Create nested object for each language
    for (lang_code, _lang_name) in supported_languages {
        let mut lang_fields = serde_json::Map::new();
        lang_fields.insert(
            "site_name".to_string(),
            serde_json::Value::String(
                current_settings
                    .get_lang_value("site_name", lang_code)
                    .unwrap_or_default(),
            ),
        );
        lang_fields.insert(
            "site_description".to_string(),
            serde_json::Value::String(
                current_settings
                    .get_lang_value("site_description", lang_code)
                    .unwrap_or_default(),
            ),
        );
        lang_fields.insert(
            "site_keywords".to_string(),
            serde_json::Value::String(
                current_settings
                    .get_lang_value("site_keywords", lang_code)
                    .unwrap_or_default(),
            ),
        );

        lang_data.insert(lang_code.clone(), serde_json::Value::Object(lang_fields));
    }

    template_settings.insert("langs".to_string(), serde_json::Value::Object(lang_data));
    let mut context = Context::new();
    context.insert("settings", &serde_json::Value::Object(template_settings));

    match super::render_settings_page(
        &state,
        "general",
        "Genel Ayarlar",
        "admin/settings/sections/general.html",
        context,
        Some(params),
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// General settings page - POST
pub async fn update_general_settings(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    let mut form_data = std::collections::HashMap::new();
    let mut uploaded_files = std::collections::HashMap::new();

    // Parse multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "site_logo" || name == "site_favicon" || name == "site_logo_dark" {
            // Handle file uploads
            if let Some(filename) = field.file_name() {
                let filename = filename.to_string(); // Clone filename to avoid borrow issues
                if !filename.is_empty() {
                    if let Ok(data) = field.bytes().await {
                        if let Ok(path) = upload_settings_file(&name, &filename, &data).await {
                            uploaded_files.insert(name, path);
                        }
                    }
                }
            }
        } else {
            // Handle text fields
            if let Ok(value) = field.text().await {
                form_data.insert(name, value);
            }
        }
    }

    // Get current settings
    let mut current_settings = match settings_service::get_settings(&state.db).await {
        Ok(settings) => settings,
        Err(_) => SettingsData::default(),
    };

    // Update settings with form data
    let config = crate::config::get_config();
    let supported_languages = &config.supported_languages;

    // Update multi-language fields
    for (lang_code, _) in supported_languages {
        if let Some(value) = form_data.get(&format!("site_name_{}", lang_code)) {
            current_settings.set_lang_value("site_name", lang_code, value);
        }
        if let Some(value) = form_data.get(&format!("site_description_{}", lang_code)) {
            current_settings.set_lang_value("site_description", lang_code, value);
        }
        if let Some(value) = form_data.get(&format!("site_keywords_{}", lang_code)) {
            current_settings.set_lang_value("site_keywords", lang_code, value);
        }
    }

    // Update single language fields
    if let Some(value) = form_data.get("contact_email") {
        current_settings.contact_email = Some(value.clone());
    }
    if let Some(value) = form_data.get("contact_phone") {
        current_settings.contact_phone = Some(value.clone());
    }
    if let Some(value) = form_data.get("contact_address") {
        current_settings.contact_address = Some(value.clone());
    }

    // Handle maintenance mode checkbox
    current_settings.maintenance_mode = Some(form_data.contains_key("maintenance_mode"));

    // Update uploaded files
    if let Some(path) = uploaded_files.get("site_logo") {
        current_settings.site_logo = Some(path.clone());
    }

    if let Some(path) = uploaded_files.get("site_logo_dark") {
        current_settings.site_logo_dark = Some(path.clone());
    }

    if let Some(path) = uploaded_files.get("site_favicon") {
        current_settings.site_favicon = Some(path.clone());
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

            Redirect::to("/admin/settings/general?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/general?error=1").into_response()
        }
    }
}

/// Upload settings file helper
async fn upload_settings_file(
    field_name: &str,
    filename: &str,
    data: &[u8],
) -> Result<String, Box<dyn std::error::Error>> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Generate unique filename
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("jpg");
    let safe_filename = format!("{}_{}.{}", field_name, timestamp, extension);

    // Create directory
    let upload_dir = Path::new("media/uploads/settings");
    if !upload_dir.exists() {
        std::fs::create_dir_all(upload_dir)?;
    }

    // Write file
    let file_path = upload_dir.join(&safe_filename);
    tokio::fs::write(&file_path, data).await?;

    // Return web path
    Ok(format!("/media/uploads/settings/{}", safe_filename))
}

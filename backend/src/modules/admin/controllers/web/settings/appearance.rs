// Appearance Settings Controller
use crate::app_state::AppState;
use crate::modules::admin::models::settings::SettingsData;
use crate::modules::admin::services::settings_service;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::Multipart;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tera::Context;
use tower_sessions::Session;
// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
}

/// Appearance settings page - GET
pub async fn appearance_settings(State(state): State<AppState>, session: Session) -> Response {
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

    // Get available themes - only show frontend theme (for single theme deployment)
    let available_themes = get_available_themes(&current_settings);

    let mut template_settings = serde_json::Map::new();

    let mut context = Context::new();

    context.insert(
        "frontend_theme",
        &current_settings
            .frontend_theme
            .unwrap_or_else(|| "base".to_string()),
    );
    context.insert("available_themes", &available_themes);
    context.insert(
        "custom_css",
        &current_settings.custom_css.unwrap_or_default(),
    );
    context.insert("custom_js", &current_settings.custom_js.unwrap_or_default());
    context.insert(
        "analytics_code",
        &current_settings.analytics_code.unwrap_or_default(),
    );
    template_settings.insert(
        "debug_logs".to_string(),
        serde_json::Value::Bool(current_settings.debug_logs.unwrap_or_default()),
    );

    template_settings.insert(
        "site_logo".to_string(),
        serde_json::Value::String(current_settings.site_logo.unwrap_or_default()),
    );
    template_settings.insert(
        "site_logo_dark".to_string(),
        serde_json::Value::String(current_settings.site_logo_dark.unwrap_or_default()),
    );
    template_settings.insert(
        "site_favicon".to_string(),
        serde_json::Value::String(current_settings.site_favicon.unwrap_or_default()),
    );

    context.insert("settings", &serde_json::Value::Object(template_settings));

    match super::render_settings_page(
        &state,
        "appearance",
        "Görünüm Ayarları",
        "admin/settings/sections/appearance.html",
        context,
        None,
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Appearance settings page - POST
pub async fn update_appearance_settings(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    let mut form_data = std::collections::HashMap::new();

    // multipart form data kara deliği
    // while let Ok(Some(field)) = multipart.next_field().await {
    //     let name = field.name().unwrap_or("").to_string();
    //     if let Ok(value) = field.text().await {
    //         form_data.insert(name, value);
    //     }
    // }

    let mut uploaded_files = std::collections::HashMap::new();

    // Parse multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        println!("Processing field: {}", name);

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

    // Update appearance settings
    if let Some(value) = form_data.get("frontend_theme") {
        current_settings.frontend_theme = Some(value.clone());
    }
    if let Some(value) = form_data.get("debug_logs") {
        current_settings.debug_logs = Some(value.parse::<bool>().unwrap_or(false));
    }
    if let Some(value) = form_data.get("custom_css") {
        current_settings.custom_css = Some(value.clone());
    }
    if let Some(value) = form_data.get("custom_js") {
        current_settings.custom_js = Some(value.clone());
    }
    if let Some(value) = form_data.get("analytics_code") {
        current_settings.analytics_code = Some(value.clone());
    }

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

            Redirect::to("/admin/settings/appearance?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/appearance?error=1").into_response()
        }
    }
}

/// Get available themes from filesystem - shows all themes in templates folder
fn get_available_themes(_settings: &SettingsData) -> Vec<ThemeInfo> {
    let mut themes = Vec::new();

    // Scan templates/ directory
    if let Ok(entries) = std::fs::read_dir("templates") {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Some(dir_name) = entry.file_name().to_str() {
                        // Skip admin directory (embedded)
                        if dir_name == "admin" {
                            continue;
                        }

                        // Check if it's a theme directory (has base.html and mailer/)
                        let base_html_path = format!("templates/{}/base.html", dir_name);
                        let mailer_dir_path = format!("templates/{}/mailer", dir_name);

                        if std::path::Path::new(&base_html_path).exists()
                            && std::path::Path::new(&mailer_dir_path).exists()
                        {
                            // Dynamically format theme name
                            let display_name = format_theme_name(dir_name);

                            themes.push(ThemeInfo {
                                name: dir_name.to_string(),
                                display_name,
                                description: "Otomatik algılandı".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort themes by name
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}

/// Format theme directory name to display name
fn format_theme_name(name: &str) -> String {
    let mut result = String::new();
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            result.push(c.to_uppercase().next().unwrap_or(c));
        } else if c == '_' || c == '-' {
            result.push(' ');
        } else {
            let prev_char = name.chars().nth(i - 1);
            if prev_char == Some('_') || prev_char == Some('-') || prev_char == Some(' ') {
                result.push(c.to_uppercase().next().unwrap_or(c));
            } else {
                result.push(c);
            }
        }
    }
    result
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

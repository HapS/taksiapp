// Security Settings Controller
use crate::app_state::AppState;
use crate::modules::admin::services::settings_service;
use crate::modules::admin::models::settings::SettingsData;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::Multipart;
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// Security settings page - GET
pub async fn security_settings(
    State(state): State<AppState>,
    session: Session,
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

    let mut context = Context::new();
    context.insert("maintenance_mode", &current_settings.maintenance_mode.unwrap_or(false));

    match super::render_settings_page(
        &state,
        "security",
        "Güvenlik Ayarları",
        "admin/settings/sections/security.html",
        context,
        None,
    ).await {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Security settings page - POST
pub async fn update_security_settings(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    let mut form_data = std::collections::HashMap::new();
    let mut processed_fields = std::collections::HashSet::new();

    // Parse multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        processed_fields.insert(name.clone());
        if let Ok(value) = field.text().await {
            form_data.insert(name, value);
        }
    }

    // Get current settings
    let mut current_settings = match settings_service::get_settings(&state.db).await {
        Ok(settings) => settings,
        Err(_) => SettingsData::default(),
    };

    // Handle maintenance mode checkbox
    if let Some(value) = form_data.get("maintenance_mode") {
        current_settings.maintenance_mode = Some(value == "on" || value == "1" || value == "true");
    } else if !processed_fields.contains("maintenance_mode") {
        current_settings.maintenance_mode = Some(false);
    }

    // Save settings
    match settings_service::update_settings(&state.db, current_settings).await {
        Ok(_) => {
            // Refresh settings cache
            if let Ok(new_settings_cache) = crate::middleware::global_context::SettingsCache::load_from_db(&state.db).await {
                if let Ok(mut cache) = state.settings_cache.write() {
                    *cache = new_settings_cache;
                }
            }
            
            Redirect::to("/admin/settings/security?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/security?error=1").into_response()
        }
    }
}
// Mail Settings Controller
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

/// Mail settings page - GET
pub async fn mail_settings(
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
    context.insert("smtp_host", &current_settings.smtp_host.unwrap_or_default());
    context.insert("smtp_port", &current_settings.smtp_port.unwrap_or(587));
    context.insert("smtp_username", &current_settings.smtp_username.unwrap_or_default());
    context.insert("smtp_password", &current_settings.smtp_password.unwrap_or_default());
    context.insert("smtp_from_email", &current_settings.smtp_from_email.unwrap_or_default());
    context.insert("smtp_from_name", &current_settings.smtp_from_name.unwrap_or_default());
    context.insert("smtp_encryption", &current_settings.smtp_encryption.unwrap_or_else(|| "tls".to_string()));

    match super::render_settings_page(
        &state,
        "mail",
        "Mail Ayarları",
        "admin/settings/sections/mail.html",
        context,
        None,
    ).await {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Mail settings page - POST
pub async fn update_mail_settings(
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

    // Update SMTP settings
    if let Some(value) = form_data.get("smtp_host") {
        current_settings.smtp_host = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("smtp_port") {
        current_settings.smtp_port = value.parse().ok();
    }
    if let Some(value) = form_data.get("smtp_username") {
        current_settings.smtp_username = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("smtp_password") {
        current_settings.smtp_password = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("smtp_from_email") {
        current_settings.smtp_from_email = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("smtp_from_name") {
        current_settings.smtp_from_name = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("smtp_encryption") {
        current_settings.smtp_encryption = if value.is_empty() { None } else { Some(value.clone()) };
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
            
            Redirect::to("/admin/settings/mail?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/mail?error=1").into_response()
        }
    }
}
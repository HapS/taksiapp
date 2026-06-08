// Social Media Settings Controller
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

/// Social media settings page - GET
pub async fn social_settings(
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
    context.insert("social_facebook", &current_settings.social_facebook.unwrap_or_default());
    context.insert("social_twitter", &current_settings.social_twitter.unwrap_or_default());
    context.insert("social_instagram", &current_settings.social_instagram.unwrap_or_default());
    context.insert("social_linkedin", &current_settings.social_linkedin.unwrap_or_default());
    context.insert("social_youtube", &current_settings.social_youtube.unwrap_or_default());
    context.insert("contact_email", &current_settings.contact_email.unwrap_or_default());
    context.insert("contact_phone", &current_settings.contact_phone.unwrap_or_default());
    context.insert("contact_address", &current_settings.contact_address.unwrap_or_default());
    context.insert("contact_map_embed", &current_settings.contact_map_embed.unwrap_or_default());

    match super::render_settings_page(
        &state,
        "social",
        "Sosyal Medya & İletişim",
        "admin/settings/sections/social.html",
        context,
        None,
    ).await {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Social media settings page - POST
pub async fn update_social_settings(
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

    // Update social media settings
    if let Some(value) = form_data.get("social_facebook") {
        current_settings.social_facebook = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("social_twitter") {
        current_settings.social_twitter = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("social_instagram") {
        current_settings.social_instagram = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("social_linkedin") {
        current_settings.social_linkedin = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("social_youtube") {
        current_settings.social_youtube = if value.is_empty() { None } else { Some(value.clone()) };
    }

    // Update contact settings
    if let Some(value) = form_data.get("contact_email") {
        current_settings.contact_email = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("contact_phone") {
        current_settings.contact_phone = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("contact_address") {
        current_settings.contact_address = if value.is_empty() { None } else { Some(value.clone()) };
    }

    if let Some(value) = form_data.get("contact_map_embed") {
        current_settings.contact_map_embed = if value.is_empty() { None } else { Some(value.clone()) };
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
            
            Redirect::to("/admin/settings/social?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/social?error=1").into_response()
        }
    }
}
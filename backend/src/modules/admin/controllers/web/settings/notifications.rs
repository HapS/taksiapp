// Notifications Settings Controller
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

/// Notifications settings page - GET
pub async fn notification_settings(State(state): State<AppState>, session: Session) -> Response {
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

    // Admin
    context.insert(
        "admin_notification_mail",
        &current_settings.admin_notification_mail.unwrap_or_default(),
    );
    context.insert(
        "admin_notification_phone",
        &current_settings
            .admin_notification_phone
            .unwrap_or_default(),
    );

    // Accounting
    context.insert(
        "accounting_notification_mail",
        &current_settings
            .accounting_notification_mail
            .unwrap_or_default(),
    );
    context.insert(
        "accounting_notification_phone",
        &current_settings
            .accounting_notification_phone
            .unwrap_or_default(),
    );

    // Warehouse
    context.insert(
        "warehouse_notification_mail",
        &current_settings
            .warehouse_notification_mail
            .unwrap_or_default(),
    );
    context.insert(
        "warehouse_notification_phone",
        &current_settings
            .warehouse_notification_phone
            .unwrap_or_default(),
    );

    // Purchasing
    context.insert(
        "purchasing_notification_mail",
        &current_settings
            .purchasing_notification_mail
            .unwrap_or_default(),
    );
    context.insert(
        "purchasing_notification_phone",
        &current_settings
            .purchasing_notification_phone
            .unwrap_or_default(),
    );

    // Return
    context.insert(
        "return_notification_mail",
        &current_settings
            .return_notification_mail
            .unwrap_or_default(),
    );
    context.insert(
        "return_notification_phone",
        &current_settings
            .return_notification_phone
            .unwrap_or_default(),
    );

    // Service
    context.insert(
        "service_notification_mail",
        &current_settings
            .service_notification_mail
            .unwrap_or_default(),
    );
    context.insert(
        "service_notification_phone",
        &current_settings
            .service_notification_phone
            .unwrap_or_default(),
    );

    match super::render_settings_page(
        &state,
        "notifications",
        "Bildirim Ayarları",
        "admin/settings/sections/notifications.html",
        context,
        None,
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Notifications settings page - POST
pub async fn update_notification_settings(
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

    // Helper closure to update fields
    let update_field = |val: Option<&String>| val.filter(|v| !v.is_empty()).cloned();

    // Update Notification settings
    current_settings.admin_notification_mail =
        update_field(form_data.get("admin_notification_mail"));
    current_settings.admin_notification_phone =
        update_field(form_data.get("admin_notification_phone"));

    current_settings.accounting_notification_mail =
        update_field(form_data.get("accounting_notification_mail"));
    current_settings.accounting_notification_phone =
        update_field(form_data.get("accounting_notification_phone"));

    current_settings.warehouse_notification_mail =
        update_field(form_data.get("warehouse_notification_mail"));
    current_settings.warehouse_notification_phone =
        update_field(form_data.get("warehouse_notification_phone"));

    current_settings.purchasing_notification_mail =
        update_field(form_data.get("purchasing_notification_mail"));
    current_settings.purchasing_notification_phone =
        update_field(form_data.get("purchasing_notification_phone"));

    current_settings.return_notification_mail =
        update_field(form_data.get("return_notification_mail"));
    current_settings.return_notification_phone =
        update_field(form_data.get("return_notification_phone"));

    current_settings.service_notification_mail =
        update_field(form_data.get("service_notification_mail"));
    current_settings.service_notification_phone =
        update_field(form_data.get("service_notification_phone"));

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

            Redirect::to("/admin/settings/notifications?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/notifications?error=1").into_response()
        }
    }
}

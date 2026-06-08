// Payment Settings Controller
use crate::app_state::AppState;
use crate::modules::admin::services::settings_service;
use crate::modules::admin::models::settings::SettingsData;
use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::Multipart;
// use rand::rand_core::le;
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// Payment settings page - GET
pub async fn payment_settings(
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

    // Payment providers JSON
    let payment_providers = current_settings.payment_providers.unwrap_or_else(|| serde_json::json!({
        "iyzico": {
            "provider_type": "iyzico",
            "enabled": false,
            "test_mode": true,
            "config": {
                "api_key": "",
                "secret_key": "",
                "base_url": "https://sandbox-api.iyzipay.com"
            }
        },
        "garanti": {
            "provider_type": "garanti",
            "enabled": false,
            "test_mode": true,
            "config": {
                "terminal_id": "",
                "merchant_id": "",
                "user_id": "",
                "password": "",
                "base_url": "https://sanalposprovtest.garantibbva.com.tr"
            }
        },

        "paytr": {
            "provider_type": "paytr",
            "enabled": false,
            "test_mode": true,
            "config": {
                "merchant_id": "",
                "merchant_key": "",
                "merchant_salt": "",
                "base_url": "https://www.paytr.com/odeme/api"
            }
        }
    }));

    let mut context = Context::new();
    context.insert("default_payment_provider", &current_settings.default_payment_provider.unwrap_or_else(|| "iyzico".to_string()));
    context.insert("payment_providers", &payment_providers);
    context.insert("bank1_name", &current_settings.bank1_name.unwrap_or_default());
    context.insert("bank1_account_holder", &current_settings.bank1_account_holder.unwrap_or_default());
    context.insert("bank1_iban", &current_settings.bank1_iban.unwrap_or_default());
    context.insert("bank1_branch_code", &current_settings.bank1_branch_code.unwrap_or_default());
    context.insert("bank2_name", &current_settings.bank2_name.unwrap_or_default());
    context.insert("bank2_account_holder", &current_settings.bank2_account_holder.unwrap_or_default());
    context.insert("bank2_iban", &current_settings.bank2_iban.unwrap_or_default());
    context.insert("bank2_branch_code", &current_settings.bank2_branch_code.unwrap_or_default());

    match super::render_settings_page(
        &state,
        "payment",
        "Ödeme Ayarları",
        "admin/settings/sections/payment.html",
        context,
        None,
    ).await {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Payment settings page - POST
pub async fn update_payment_settings(
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

    // Update default payment provider
    if let Some(value) = form_data.get("default_payment_provider") {
        current_settings.default_payment_provider = if value.is_empty() { None } else { Some(value.clone()) };
    }

    // Update İyzico settings
    if let Some(value) = form_data.get("iyzico_enabled") {
        current_settings.set_payment_provider_enabled("iyzico", value == "on" || value == "1" || value == "true");
    } else if !processed_fields.contains("iyzico_enabled") {
        current_settings.set_payment_provider_enabled("iyzico", false);
    }

    if let Some(value) = form_data.get("iyzico_test_mode") {
        current_settings.set_payment_provider_test_mode("iyzico", value == "on" || value == "1" || value == "true");
    } else if !processed_fields.contains("iyzico_test_mode") {
        current_settings.set_payment_provider_test_mode("iyzico", false);
    }

    if let Some(value) = form_data.get("iyzico_api_key") {
        current_settings.set_payment_provider_config("iyzico", "api_key", value);
    }
    if let Some(value) = form_data.get("iyzico_secret_key") {
        current_settings.set_payment_provider_config("iyzico", "secret_key", value);
    }
    if let Some(value) = form_data.get("iyzico_base_url") {
        current_settings.set_payment_provider_config("iyzico", "base_url", value);
    }

    // Update Garanti settings
    if let Some(value) = form_data.get("garanti_enabled") {
        current_settings.set_payment_provider_enabled("garanti", value == "on" || value == "1" || value == "true");
    } else if !processed_fields.contains("garanti_enabled") {
        current_settings.set_payment_provider_enabled("garanti", false);
    }

    if let Some(value) = form_data.get("garanti_test_mode") {
        current_settings.set_payment_provider_test_mode("garanti", value == "on" || value == "1" || value == "true");
    } else if !processed_fields.contains("garanti_test_mode") {
        current_settings.set_payment_provider_test_mode("garanti", false);
    }

    if let Some(value) = form_data.get("garanti_terminal_id") {
        current_settings.set_payment_provider_config("garanti", "terminal_id", value);
    }
    if let Some(value) = form_data.get("garanti_merchant_id") {
        current_settings.set_payment_provider_config("garanti", "merchant_id", value);
    }
    if let Some(value) = form_data.get("garanti_user_id") {
        current_settings.set_payment_provider_config("garanti", "user_id", value);
    }
    if let Some(value) = form_data.get("garanti_password") {
        current_settings.set_payment_provider_config("garanti", "password", value);
    }
    if let Some(value) = form_data.get("garanti_store_key") {
        current_settings.set_payment_provider_config("garanti", "store_key", value);
    }
    if let Some(value) = form_data.get("garanti_base_url") {
        current_settings.set_payment_provider_config("garanti", "base_url", value);
    }

    // Update PayTR settings
    if let Some(value) = form_data.get("paytr_enabled") {
        current_settings.set_payment_provider_enabled("paytr", value == "on" || value == "1" || value == "true");
    } else if !processed_fields.contains("paytr_enabled") {
        current_settings.set_payment_provider_enabled("paytr", false);
    }

    if let Some(value) = form_data.get("paytr_test_mode") {
        current_settings.set_payment_provider_test_mode("paytr", value == "on" || value == "1" || value == "true");
    } else if !processed_fields.contains("paytr_test_mode") {
        current_settings.set_payment_provider_test_mode("paytr", false);
    }

    if let Some(value) = form_data.get("paytr_merchant_key") {
        current_settings.set_payment_provider_config("paytr", "merchant_key", value);
    }
    if let Some(value) = form_data.get("paytr_merchant_salt") {
        current_settings.set_payment_provider_config("paytr", "merchant_salt", value);
    }

    if let Some(value) = form_data.get("paytr_merchant_id") {
        current_settings.set_payment_provider_config("paytr", "merchant_id", value);
    }

    if let Some(value) = form_data.get("paytr_base_url") {
        current_settings.set_payment_provider_config("paytr", "base_url", value);
    }

    // Update bank settings
    if let Some(value) = form_data.get("bank1_name") {
        current_settings.bank1_name = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("bank1_account_holder") {
        current_settings.bank1_account_holder = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("bank1_iban") {
        current_settings.bank1_iban = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("bank1_branch_code") {
        current_settings.bank1_branch_code = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("bank2_name") {
        current_settings.bank2_name = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("bank2_account_holder") {
        current_settings.bank2_account_holder = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("bank2_iban") {
        current_settings.bank2_iban = if value.is_empty() { None } else { Some(value.clone()) };
    }
    if let Some(value) = form_data.get("bank2_branch_code") {
        current_settings.bank2_branch_code = if value.is_empty() { None } else { Some(value.clone()) };
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
            
            Redirect::to("/admin/settings/payment?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/payment?error=1").into_response()
        }
    }
}
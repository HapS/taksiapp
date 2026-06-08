// Payment Settings Controller
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

/// Payment settings page - GET
pub async fn payment_methods_settings(State(state): State<AppState>, session: Session) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/").into_response();
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
    let payment_methods = current_settings.payment_methods.unwrap_or_else(|| {
        serde_json::json!({
            "credit_card": {
                "keyword": "kredi-karti",
                "icon": "credit-card-2-front",
                "order_id": 1,
                "langs": {
                    "en": {
                        "title": "Credit Card",
                        "description": "Pay with your credit card"
                    },
                    "tr": {
                        "title": "Kredi Kartı",
                        "description": "Kredi kartı ile ödeme yapın"
                    }
                },
                "b2b_available": true,
                "b2c_available": true
            },
            "bank_transfer": {
                "keyword": "banka-transferi",
                "icon": "bank",
                "order_id": 2,
                "langs": {
                    "en": {
                        "title": "Bank Transfer",
                        "description": "Pay via bank transfer"
                    },
                    "tr": {
                        "title": "Banka Transferi",
                        "description": "Banka transferi ile ödeme yapın"
                    }
                },
                "b2b_available": true,
                "b2c_available": true
            },
            "cash_on_delivery": {
                "keyword": "kapida-odeme",
                "icon": "cash-coin",
                "order_id": 3,
                "langs": {
                    "en": {
                        "title": "Cash on Delivery",
                        "description": "Pay with cash upon delivery"
                    },
                    "tr": {
                        "title": "Kapıda Ödeme",
                        "description": "Kapıda nakit ödeme yapın"
                    }
                },
                "b2b_available": false,
                "b2c_available": true
            },
            "b2b_credit": {
                "keyword": "b2b-kredi",
                "icon": "person-badge",
                "order_id": 4,
                "langs": {
                    "en": {
                        "title": "B2B Credit",
                        "description": "Pay with B2B credit account"
                    },
                    "tr": {
                        "title": "B2B Kredi",
                        "description": "B2B kredi hesabı ile ödeme yapın"
                    }
                },
                "b2b_available": true,
                "b2c_available": false
            }
        })
    });

    let mut context = Context::new();

    context.insert("payment_methods", &payment_methods);

    match super::render_settings_page(
        &state,
        "payment-methods",
        "Ödeme Yöntemleri",
        "admin/settings/sections/payment_methods.html",
        context,
        None,
    )
    .await
    {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Payment settings page - POST
pub async fn update_payment_methods_settings(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/").into_response();
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

    let payment_method_keys = [
        "credit_card",
        "bank_transfer",
        "cash_on_delivery",
        "b2b_credit",
    ];
    let supported_langs = ["tr", "en"];

    for method_key in payment_method_keys {
        for lang in supported_langs {
            let title_key = format!("{}_title_{}", method_key, lang);
            let description_key = format!("{}_description_{}", method_key, lang);

            let title = form_data.get(&title_key).map(|s| s.as_str());
            let description = form_data.get(&description_key).map(|s| s.as_str());

            let b2b_key = format!("{}_b2b_available", method_key);
            let b2c_key = format!("{}_b2c_available", method_key);

            let b2b_available = if processed_fields.contains(&b2b_key) {
                form_data
                    .get(&b2b_key)
                    .map(|v| v == "on" || v == "1" || v == "true")
            } else {
                Some(false)
            };

            let b2c_available = if processed_fields.contains(&b2c_key) {
                form_data
                    .get(&b2c_key)
                    .map(|v| v == "on" || v == "1" || v == "true")
            } else {
                Some(false)
            };

            current_settings.update_payment_method(
                method_key,
                lang,
                title,
                description,
                b2b_available,
                b2c_available,
                None,
                None,
            );
        }

        let icon_key = format!("{}_icon", method_key);
        let order_key = format!("{}_order", method_key);

        if let Some(icon) = form_data.get(&icon_key) {
            current_settings.update_payment_method(
                method_key,
                "tr",
                None,
                None,
                None,
                None,
                Some(icon.as_str()),
                None,
            );
        }

        if let Some(order_str) = form_data.get(&order_key) {
            if let Ok(order) = order_str.parse::<i32>() {
                current_settings.update_payment_method(
                    method_key,
                    "tr",
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(order),
                );
            }
        }
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

            Redirect::to("/admin/settings/payment-methods?success=1").into_response()
        }
        Err(e) => {
            eprintln!("Settings update error: {:?}", e);
            Redirect::to("/admin/settings/payment-methods?error=1").into_response()
        }
    }
}

// Settings Module - Modular settings management
pub mod advanced;
pub mod appearance;
pub mod general;
pub mod mail;
pub mod notifications;
pub mod security;
pub mod seo;
pub mod social;
// pub mod update;

use crate::app_state::AppState;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use std::collections::HashMap;
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// Settings main page - redirect to general
pub async fn settings_index(State(state): State<AppState>, session: Session) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    // Redirect to general settings
    Redirect::to("/admin/settings/general").into_response()
}

/// Common function to render settings layout with sidebar
pub async fn render_settings_page(
    state: &AppState,
    section: &str,
    title: &str,
    template: &str,
    mut context: Context,
    query_params: Option<HashMap<String, String>>,
) -> Result<Html<String>, Response> {
    // Add common settings context
    context.insert("title", title);
    context.insert("current_section", section);
    context.insert("sidebar_items", &get_sidebar_items());
    context.insert("current_path", &format!("/admin/settings/{}", section));

    // Handle success/error messages from query parameters
    if let Some(params) = query_params {
        if params.contains_key("success") {
            context.insert("success", &true);
        }
        if params.contains_key("error") {
            context.insert("error", &true);
        }
    }

    // Get supported languages
    let config = crate::config::get_config();
    context.insert(
        "supported_languages",
        &config.supported_languages.keys().collect::<Vec<_>>(),
    );

    match state.render_template(template, &context) {
        Ok(html) => Ok(Html(html)),
        Err(e) => {
            // In debug mode show the detailed Tera error page (including raw Tera output).
            // In production return a generic 500 response for security.
            return Err(
                crate::middleware::error_handler::handle_template_error_with_context(
                    &e,
                    config.is_debug(),
                    false,
                    Some(state),
                ),
            );
        }
    }
}

/// Get sidebar menu items
fn get_sidebar_items() -> Vec<SidebarItem> {
    vec![
        SidebarItem {
            id: "general".to_string(),
            title: "Genel Ayarlar".to_string(),
            icon: "bi-gear".to_string(),
            url: "/admin/settings/general".to_string(),
        },
        SidebarItem {
            id: "appearance".to_string(),
            title: "Görünüm".to_string(),
            icon: "bi-palette".to_string(),
            url: "/admin/settings/appearance".to_string(),
        },
        SidebarItem {
            id: "seo".to_string(),
            title: "SEO Ayarları".to_string(),
            icon: "bi-search".to_string(),
            url: "/admin/settings/seo".to_string(),
        },
        SidebarItem {
            id: "social".to_string(),
            title: "Sosyal Medya".to_string(),
            icon: "bi-share".to_string(),
            url: "/admin/settings/social".to_string(),
        },
        SidebarItem {
            id: "notifications".to_string(),
            title: "Bildirim Ayarları".to_string(),
            icon: "bi-bell".to_string(),
            url: "/admin/settings/notifications".to_string(),
        },
        SidebarItem {
            id: "mail".to_string(),
            title: "Mail Ayarları".to_string(),
            icon: "bi-envelope".to_string(),
            url: "/admin/settings/mail".to_string(),
        },
        SidebarItem {
            id: "security".to_string(),
            title: "Güvenlik".to_string(),
            icon: "bi-shield-check".to_string(),
            url: "/admin/settings/security".to_string(),
        },
        SidebarItem {
            id: "advanced".to_string(),
            title: "Gelişmiş".to_string(),
            icon: "bi-tools".to_string(),
            url: "/admin/settings/advanced".to_string(),
        },
        // SidebarItem {
        //     id: "update".to_string(),
        //     title: "Güncelleme".to_string(),
        //     icon: "bi-arrow-up-circle".to_string(),
        //     url: "/admin/settings/update".to_string(),
        // },
    ]
}

#[derive(serde::Serialize)]
pub struct SidebarItem {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub url: String,
}

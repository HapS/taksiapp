use crate::app_state::AppState;
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use tera::Context;
use tower_sessions::Session;

#[derive(Debug, Serialize, Deserialize)]
pub struct ThemeInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
}

pub async fn locales_manager(State(state): State<AppState>, session: Session) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Dil Dosyası Yönetimi");
    context.insert("current_path", "/admin/locales");

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    // Config'deki dilleri de gönderelim
    let config = crate::config::get_config();
    context.insert("supported_languages", &config.supported_languages);
    context.insert("default_language", &config.default_language);

    // Get available themes dynamically
    let available_themes = get_available_themes();
    context.insert("available_themes", &available_themes);

    match state.render_template("admin/locales/index.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// Get available themes from filesystem
fn get_available_themes() -> Vec<ThemeInfo> {
    let mut themes = Vec::new();

    // Always add admin as first option
    themes.push(ThemeInfo {
        name: "admin".to_string(),
        display_name: "Admin".to_string(),
        description: "Admin panel".to_string(),
    });

    // Scan templates/ directory
    if let Ok(entries) = std::fs::read_dir("templates") {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Some(dir_name) = entry.file_name().to_str() {
                        // Skip admin directory (already added)
                        if dir_name == "admin" {
                            continue;
                        }

                        // Check if it's a theme directory (has base.html)
                        let base_html_path = format!("templates/{}/base.html", dir_name);

                        if std::path::Path::new(&base_html_path).exists() {
                            // Dynamically format theme name (kebab-case to Title Case)
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

    themes
}

/// Format theme directory name to display name
/// Example: "balmumcu" -> "Balmumcu", "grey_wolf" -> "Grey Wolf"
fn format_theme_name(name: &str) -> String {
    let mut result = String::new();
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            result.push(c.to_uppercase().next().unwrap_or(c));
        } else if c == '_' || c == '-' {
            result.push(' ');
        } else {
            // Check if previous char was _ or - or if it's uppercase
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

// Admin Page Web Controllers - HTML views
use crate::app_state::AppState;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

// Helper: Admin kontrolü
// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

// Admin sayfa listesi
pub async fn admin_content_list(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Sayfa Yönetimi");

    // Query string'li current_path oluştur
    let mut current_path = "/admin/contents".to_string();
    let mut active_menu = String::from("contents:all");
    if !params.is_empty() {
        let query_string: Vec<String> =
            params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        current_path = format!("{}?{}", current_path, query_string.join("&"));

        // Eğer content_type varsa active_menu olarak ayarla (örn. "product", "news", "blog")
        if let Some(ct) = params.get("content_type") {
            active_menu = format!("contents:{}", ct);
        }
    }
    context.insert("current_path", &current_path);
    context.insert("active_menu", &active_menu);

    //yeni xxx buton title
    // context.insert("new_button_title", "Yeni Sayfa Oluştur");

    match params.get("content_type") {
        Some(ct) if ct == "product" => {
            context.insert("new_button_title", "Yeni Ürün");
        }
        Some(ct) if ct == "news" => {
            context.insert("new_button_title", "Yeni Haber");
        }
        Some(ct) if ct == "blog" => {
            context.insert("new_button_title", "Yeni Blog");
        }
        Some(ct) if ct == "form" => {
            context.insert("new_button_title", "Yeni Form");
        }
        Some(ct) if ct == "page" => {
            context.insert("new_button_title", "Yeni Sayfa");
        }
        _ => {
            context.insert("new_button_title", "Yeni İçerik");
        }
    }

    // User bilgilerini ekle
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/contents/content_list.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode, otherwise return generic 500
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

// Admin sayfa oluşturma formu
pub async fn admin_content_create(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Yeni Sayfa Oluştur");

    // Default current_path and active menu
    let mut current_path = "/admin/contents/new".to_string();
    let mut active_menu = String::from("contents:all");

    if !params.is_empty() {
        let query_string: Vec<String> =
            params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        current_path = format!("{}?{}", current_path, query_string.join("&"));

        if let Some(ct) = params.get("content_type") {
            active_menu = format!("contents:{}", ct);
        }
    }

    context.insert("current_path", &current_path);
    context.insert("active_menu", &active_menu);

    // User bilgilerini ekle
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/contents/content_add_edit.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode, otherwise return generic 500
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

// Admin sayfa detay sayfası
pub async fn admin_content_detail(
    State(state): State<AppState>,
    session: Session,
    Path((content_type, id)): Path<(String, i64)>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    // if params.get("content_type").is_none() {
    //     return (StatusCode::BAD_REQUEST, "content_type parametresi gerekli").into_response();
    // }

    let mut current_path = format!("/admin/contents/{}/{}", content_type, id);
    // Varsayılan aktif menüyü path'teki content_type üzerinden ayarla (düzenleme sayfasında doğru görünüm için)
    let mut active_menu = format!("contents:{}", &content_type);
    if !params.is_empty() {
        let query_string: Vec<String> =
            params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        current_path = format!("{}?{}", current_path, query_string.join("&"));

        // Eğer query param olarak content_type varsa üzerine yaz
        if let Some(ct) = params.get("content_type") {
            active_menu = format!("contents:{}", ct);
        }
    }

    let mut context = Context::new();
    context.insert("page_id", &id);
    context.insert("title", "Sayfa Düzenle");
    context.insert("current_path", &current_path);
    context.insert("active_menu", &active_menu);
    context.insert("content_type", &content_type);

    // User bilgilerini ekle
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/contents/content_add_edit.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode, otherwise return generic 500
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

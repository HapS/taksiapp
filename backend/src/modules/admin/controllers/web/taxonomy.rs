// Taxonomy Web Controllers - HTML pages
use crate::app_state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// Taxonomy Manager - Admin vocabulary/term management page
pub async fn taxonomy_manager(State(state): State<AppState>, session: Session) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Kategori Grupları Yönetimi");
    context.insert("current_path", "/admin/taxonomy");
    context.insert("vocabulary_type", ""); // Varsayılan boş değer
    context.insert("vocabulary_id", &None::<Option<i64>>);

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/taxonomy/vocabulary_manager.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

pub async fn vocabulary_list_by_type(
    State(state): State<AppState>,
    session: Session,
    Path(vocabulary_type): Path<String>,
) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    //title vovabulary type
    let title = match vocabulary_type.as_str() {
        "category" => "Kategori Grupları",
        "menu" => "Menü Grupları",
        "tag" => "Etiket Grupları",
        "product_attributes" => "Ürün Özellik Grupları",
        _ => "Tüm Gruplar",
    };

    let mut context = Context::new();
    context.insert("title", &title);
    context.insert(
        "current_path",
        format!("/admin/taxonomy/{}", vocabulary_type).as_str(),
    );
    context.insert("vocabulary_type", &vocabulary_type);
    context.insert("vocabulary_id", &None::<Option<i64>>);

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/taxonomy/vocabulary_manager.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// Term Manager - Admin term management page for specific vocabulary
pub async fn term_manager(
    State(state): State<AppState>,
    session: Session,
    Path(vocabulary_id): Path<i64>,
) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    // Get vocabulary data
    use crate::modules::taxonomy::helpers::vocabulary_helper::VocabularyExtensions;
    use crate::modules::taxonomy::models::vocabulary::Entity as VocabularyEntity;
    use sea_orm::*;

    let vocabulary = match VocabularyEntity::find_by_id(vocabulary_id)
        .one(&state.db)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => return (StatusCode::NOT_FOUND, "Vocabulary not found").into_response(),
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Convert to response with proper language data
    let config = crate::config::get_config();
    let vocabulary_response = vocabulary.to_response(&config.default_language);

    let mut context = Context::new();
    context.insert("title", "Term Yönetimi");
    context.insert(
        "current_path",
        format!("/admin/taxonomy/vocabulary/{}/term", vocabulary_id).as_str(),
    );
    context.insert("vocabulary_id", &vocabulary_id);
    context.insert("vocabulary", &vocabulary_response);

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/taxonomy/term_manager.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// Term Add - Create new term page
pub async fn term_add(
    State(state): State<AppState>,
    session: Session,
    Path(vocabulary_id): Path<i64>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let parent_id = query.get("parent_id").and_then(|s| s.parse::<i64>().ok());

    // Get vocabulary data
    use crate::modules::taxonomy::helpers::VocabularyExtensions;
    use crate::modules::taxonomy::models::vocabulary::Entity as VocabularyEntity;
    use sea_orm::*;

    let vocabulary = match VocabularyEntity::find_by_id(vocabulary_id)
        .one(&state.db)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => return (StatusCode::NOT_FOUND, "Vocabulary not found").into_response(),
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    let mut context = Context::new();
    context.insert("title", "Yeni Term Ekle");
    context.insert("vocabulary", &vocabulary.to_response("tr"));
    context.insert("parent_id", &parent_id);
    context.insert(
        "current_path",
        &format!("/admin/taxonomy/vocabularies/{}/terms/new", vocabulary_id),
    );

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    // Select template based on vocabulary type
    let template_name = match vocabulary.vocabulary_type.as_str() {
        "menu" => "admin/taxonomy/term_add_edit.html",
        "category" => "admin/taxonomy/term_add_edit.html",
        _ => "admin/taxonomy/term_add_edit.html",
    };

    match state.render_template(template_name, &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// Term Edit - Edit existing term page
pub async fn term_edit(
    State(state): State<AppState>,
    session: Session,
    Path((vocabulary_id, term_id)): Path<(i64, i64)>,
) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    // Get vocabulary data
    use crate::modules::taxonomy::helpers::VocabularyExtensions;
    use crate::modules::taxonomy::models::vocabulary::Entity as VocabularyEntity;
    use sea_orm::*;

    let vocabulary = match VocabularyEntity::find_by_id(vocabulary_id)
        .one(&state.db)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => return (StatusCode::NOT_FOUND, "Vocabulary not found").into_response(),
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    let mut context = Context::new();
    context.insert("title", "Term Düzenle");
    context.insert("vocabulary", &vocabulary.to_response("tr"));
    context.insert("term_id", &term_id);
    context.insert(
        "current_path",
        &format!(
            "/admin/taxonomy/vocabularies/{}/terms/{}",
            vocabulary_id, term_id
        ),
    );

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    // Select template based on vocabulary type
    let template_name = match vocabulary.vocabulary_type.as_str() {
        "menu" => "admin/taxonomy/term_add_edit.html",
        "category" => "admin/taxonomy/term_add_edit.html",
        _ => "admin/taxonomy/term_add_edit.html",
    };

    match state.render_template(template_name, &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

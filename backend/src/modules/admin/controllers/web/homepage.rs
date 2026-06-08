// Admin Page Web Controllers - HTML views
use crate::app_state::AppState;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use slug::slugify;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tera::Context;
use tower_sessions::Session;

// Homepage cache - kendi cache'i
lazy_static::lazy_static! {
    static ref HOMEPAGE_RENDER_CACHE: Arc<RwLock<HashMap<String, HomepageRenderResponse>>> = Arc::new(RwLock::new(HashMap::new()));
}

// Helper: Admin kontrolü
// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

// Content model import
use crate::modules::content::models::content::{
    Column as ContentColumn, Entity as Content, Model as ContentModel,
};
use crate::modules::taxonomy::helpers::term_helper::get_string_from_json as get_term_string;
use crate::modules::taxonomy::models::term::{Column as TermColumn, Entity as Term};
use crate::modules::taxonomy::models::vocabulary;

// Homepage Model - Tek row, tüm sections JSON'da
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "homepage")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub data: serde_json::Value, // Tüm sections burada
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Homepage sections'ları yükle ve cache'e al
async fn load_homepage_sections(db: &DatabaseConnection) -> Result<JsonValue, DbErr> {
    // İlk (ve tek) homepage kaydını getir
    match Entity::find().one(db).await {
        Ok(Some(homepage)) => {
            let mut sections = homepage.data.clone();

            // Eğer sections array ise, her section'ı kontrol et
            if let Some(sections_array) = sections.as_array_mut() {
                for section in sections_array {
                    // Vocabulary source olan section'lar için termlerini yükle
                    if let Some(source) = section.get("source") {
                        if source.get("type").and_then(|v| v.as_str()) == Some("vocabulary") {
                            if let Some(vocabulary_id) =
                                source.get("vocabulary_id").and_then(|v| v.as_i64())
                            {
                                // Bu vocabulary'e ait termleri çek
                                match Term::find()
                                    .filter(TermColumn::VocabularyId.eq(vocabulary_id))
                                    .filter(TermColumn::Publish.eq(true))
                                    .order_by_asc(TermColumn::OrderId)
                                    .all(db)
                                    .await
                                {
                                    Ok(terms) => {
                                        // Termleri JSON formatına çevir
                                        let terms_json: Vec<serde_json::Value> = terms
                                            .into_iter()
                                            .map(|term| {
                                                let title =
                                                    get_term_string(&term.data, "title", "tr")
                                                        .or_else(|| {
                                                            get_term_string(
                                                                &term.data, "title", "en",
                                                            )
                                                        })
                                                        .unwrap_or_else(|| {
                                                            format!("Term {}", term.id)
                                                        });

                                                serde_json::json!({
                                                    "id": term.id,
                                                    "title": title,
                                                    "data": term.data,
                                                    "vocabulary_id": term.vocabulary_id,
                                                    "order_id": term.order_id,
                                                    "publish": term.publish,
                                                    "created_at": term.created_at
                                                })
                                            })
                                            .collect();

                                        // Section'a terms field'ını ekle
                                        if let Some(section_obj) = section.as_object_mut() {
                                            section_obj.insert(
                                                "terms".to_string(),
                                                serde_json::Value::Array(terms_json),
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Terms loading error for vocabulary {}: {}",
                                            vocabulary_id, e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok(sections)
        }
        Ok(None) => {
            // Eğer kayıt yoksa boş array döndür
            Ok(serde_json::json!([]))
        }
        Err(e) => Err(e),
    }
}

// Cache'den homepage render'ını getir - KOMPLE CACHE
async fn get_cached_homepage_render(
    state: &AppState,
    lang: &str,
) -> Result<HomepageRenderResponse, Box<dyn std::error::Error>> {
    let cache_key = format!("homepage_render_{}", lang);

    // Önce render cache'den kontrol et
    if let Ok(cache_guard) = HOMEPAGE_RENDER_CACHE.read() {
        if let Some(cached_render) = cache_guard.get(&cache_key) {
            eprintln!(
                "Homepage RENDER loaded from CACHE (lang: {})",
                lang
            );
            return Ok(cached_render.clone());
        }
    }

    eprintln!(
        "🔄 Homepage RENDER loading from DATABASE (cache miss, lang: {})",
        lang
    );

    // Cache'de yoksa database'den yükle ve render et
    let sections = load_homepage_sections(&state.db).await?;

    // Sections'ları parse et
    let sections_array = match sections.as_array() {
        Some(arr) => arr,
        None => {
            return Ok(HomepageRenderResponse {
                sections: vec![],
                language: lang.to_string(),
                total_sections: 0,
            });
        }
    };

    let mut rendered_sections = Vec::new();

    // Her section'ı render et
    for section in sections_array {
        // Sadece aktif section'ları render et
        if section
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
        {
            match render_section(&state.db, section, lang).await {
                Ok(rendered_section) => rendered_sections.push(rendered_section),
                Err(e) => {
                    eprintln!(
                        "Section render error for section {:?}: {}",
                        section
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown"),
                        e
                    );
                    // Hata durumunda section'ı atla, devam et
                }
            }
        }
    }

    // Order'a göre sırala
    rendered_sections.sort_by_key(|s| s.order);

    let render_response = HomepageRenderResponse {
        sections: rendered_sections,
        language: lang.to_string(),
        total_sections: sections_array.len(),
    };

    // Render'ı cache'e kaydet
    if let Ok(mut cache_guard) = HOMEPAGE_RENDER_CACHE.write() {
        cache_guard.insert(cache_key, render_response.clone());
        eprintln!(
            "📦 Homepage RENDER CACHED successfully (lang: {})",
            lang
        );
    }

    Ok(render_response)
}

// Homepage cache'ini temizle - Render cache
pub fn clear_homepage_cache(_state: &AppState) {
    // Render cache'ini temizle (tüm diller)
    if let Ok(mut cache_guard) = HOMEPAGE_RENDER_CACHE.write() {
        cache_guard.clear();
        eprintln!("🗑️ Homepage render cache CLEARED (all languages)");
    }
}

// Request DTOs
#[derive(Deserialize)]
pub struct UpdateHomepageRequest {
    pub sections: serde_json::Value,
}

#[derive(Deserialize)]
pub struct RenderQuery {
    pub lang: Option<String>,
}

// Response DTOs - Tek unified response
#[derive(Serialize, Clone)]
pub struct HomepageContentResponse {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub excerpt: Option<String>,
    pub price: Option<String>,
    pub image: Option<String>,
    pub url: Option<String>,
    pub content_type: String,
    pub tags: Vec<TagInfo>,
    pub created_at: Option<String>,
    pub data: serde_json::Value, // Manipüle edilmiş JSON data
}

// Re-use TagInfo from page_helper
pub use crate::modules::content::helpers::page_helper::TagInfo;

#[derive(Serialize, Clone)]
pub struct RenderedSection {
    pub id: String, // Frontend'den gelen ID
    pub title: String,
    pub description: String,
    pub template: String,
    pub background_color: Option<String>,
    pub settings: serde_json::Value,
    pub items: Vec<serde_json::Value>, // Unified HomepageContentResponse
    pub active: bool,
    pub order: i32,
}

#[derive(Serialize, Clone)]
pub struct HomepageRenderResponse {
    pub sections: Vec<RenderedSection>,
    pub language: String,
    pub total_sections: usize,
}

// Home Page Builder Yönetici Görünümü
pub async fn home_page_builder(
    State(state): State<AppState>,
    session: Session,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Ana Sayfa Düzenleyici - Admin Panel");

    let languages = crate::supported_languages_map!();

    println!("supportedt languages hashmap {:?}", languages);

    context.insert("supported_languages", &languages);

    println!("CONTEXT : {:?}", &context);

    // Query string'li current_path oluştur
    let mut current_path = "/admin/home-page-builder".to_string();

    if !params.is_empty() {
        let query_string: Vec<String> =
            params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        current_path = format!("{}?{}", current_path, query_string.join("&"));
    }
    context.insert("current_path", &current_path);

    match state.render_template("admin/home_page_builder.html", &context) {
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

// API: Homepage sections getir (Admin - Cache YOK)
pub async fn api_get_homepage_sections(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Unauthorized"})),
        )
            .into_response();
    }

    // Admin'de cache yok - her zaman fresh data
    match Entity::find().one(&state.db).await {
        Ok(Some(homepage)) => {
            let mut sections = homepage.data.clone();

            // Eğer sections array ise, her section'ı kontrol et
            if let Some(sections_array) = sections.as_array_mut() {
                for section in sections_array {
                    // Vocabulary source olan section'lar için termlerini yükle
                    if let Some(source) = section.get("source") {
                        if source.get("type").and_then(|v| v.as_str()) == Some("vocabulary") {
                            if let Some(vocabulary_id) =
                                source.get("vocabulary_id").and_then(|v| v.as_i64())
                            {
                                // Bu vocabulary'e ait termleri çek
                                match Term::find()
                                    .filter(TermColumn::VocabularyId.eq(vocabulary_id))
                                    // .filter(TermColumn::Publish.eq(true))
                                    .order_by_asc(TermColumn::OrderId)
                                    .all(&state.db)
                                    .await
                                {
                                    Ok(terms) => {
                                        // Termleri JSON formatına çevir
                                        let terms_json: Vec<serde_json::Value> = terms
                                            .into_iter()
                                            .map(|term| {
                                                let title =
                                                    get_term_string(&term.data, "title", "tr")
                                                        .or_else(|| {
                                                            get_term_string(
                                                                &term.data, "title", "en",
                                                            )
                                                        })
                                                        .unwrap_or_else(|| {
                                                            format!("Term {}", term.id)
                                                        });

                                                serde_json::json!({
                                                    "id": term.id,
                                                    "title": title,
                                                    "data": term.data,
                                                    "vocabulary_id": term.vocabulary_id,
                                                    "order_id": term.order_id,
                                                    "publish": term.publish,
                                                    "created_at": term.created_at
                                                })
                                            })
                                            .collect();

                                        // Section'a terms field'ını ekle
                                        if let Some(section_obj) = section.as_object_mut() {
                                            section_obj.insert(
                                                "terms".to_string(),
                                                serde_json::Value::Array(terms_json),
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Terms loading error for vocabulary {}: {}",
                                            vocabulary_id, e
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sections": sections
                })),
            )
                .into_response()
        }
        Ok(None) => {
            // Eğer kayıt yoksa boş array döndür
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sections": []
                })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            )
                .into_response()
        }
    }
}

// API: Homepage sections güncelle
pub async fn api_update_homepage_sections(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<UpdateHomepageRequest>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Unauthorized"})),
        )
            .into_response();
    }

    // İlk homepage kaydını bul veya oluştur
    let homepage = match Entity::find().one(&state.db).await {
        Ok(Some(existing)) => existing,
        Ok(None) => {
            // Kayıt yoksa oluştur
            let new_homepage = ActiveModel {
                data: Set(serde_json::json!([])),
                created_at: Set(Some(chrono::Utc::now())),
                updated_at: Set(Some(chrono::Utc::now())),
                ..Default::default()
            };

            match new_homepage.insert(&state.db).await {
                Ok(created) => created,
                Err(e) => {
                    eprintln!("Database error: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": "Database error"})),
                    )
                        .into_response();
                }
            }
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            )
                .into_response();
        }
    };

    // Sections'ı güncelle
    let mut homepage_active: ActiveModel = homepage.into();
    homepage_active.data = Set(payload.sections);
    homepage_active.updated_at = Set(Some(chrono::Utc::now()));

    match homepage_active.update(&state.db).await {
        Ok(updated_homepage) => {
            // Cache'i temizle
            clear_homepage_cache(&state);

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "message": "Homepage updated successfully",
                    "sections": updated_homepage.data
                })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            )
                .into_response()
        }
    }
}

// Helper functions
use crate::modules::content::helpers::page_helper::get_string_from_json;
use crate::modules::content::helpers::page_helper::fetch_tags_for_contents;

async fn render_section(
    db: &DatabaseConnection,
    section: &serde_json::Value,
    lang: &str,
) -> Result<RenderedSection, Box<dyn std::error::Error>> {
    // println!("SECTIONS : {}", &section);

    let section_id = section
        .get("id")
        .and_then(|v| v.as_f64())
        .map(|f| f.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // let title = section
    //     .get("langs")
    //     .and_then(|v| v.as_object())
    //     .and_then(|langs| langs.get(lang))
    //     .and_then(|v| v.as_str())
    //     .unwrap_or("Untitled Section")
    //     .to_string();
    //
    //

    //bunu ezebere yazana bir milyon dolar
    let title = section
        .get("langs")
        .and_then(|v| v.as_object())
        .and_then(|langs| langs.get(lang))
        .and_then(|lang_obj| lang_obj.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let description = section
        .get("langs")
        .and_then(|v| v.as_object())
        .and_then(|langs| langs.get(lang))
        .and_then(|lang_obj| lang_obj.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let template = section
        .get("template")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let settings = section
        .get("settings")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let active = section
        .get("active")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let order = section.get("order").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    let source = section.get("source").ok_or("Missing source")?;
    let source_type = source
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("automatic");

    // eprintln!("🔍 Section '{}': source_type = {}", title, source_type);

    let mut items = Vec::new();

    match source_type {
        "vocabulary" => {
            // Vocabulary source - term'leri çek
            let vocabulary_id = source
                .get("vocabulary_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(1);

            let selection_type = source
                .get("selection_type")
                .and_then(|v| v.as_str())
                .unwrap_or("automatic");

            // Vocabulary ID'sini kontrol et
            if vocabulary_id <= 0 {
                eprintln!("Invalid vocabulary_id: {}", vocabulary_id);
                return Ok(RenderedSection {
                    id: section_id,
                    title,
                    description,
                    template,
                    background_color: None,
                    settings,
                    items: vec![],
                    active,
                    order,
                });
            }

            match selection_type {
                "manual" => {
                    // Manuel seçim - selected_terms array'inden term'leri çek
                    if let Some(selected_terms) =
                        source.get("selected_terms").and_then(|v| v.as_array())
                    {
                        let term_ids: Vec<i64> =
                            selected_terms.iter().filter_map(|v| v.as_i64()).collect();

                        if !term_ids.is_empty() {
                            match Term::find()
                                .filter(TermColumn::Id.is_in(term_ids.clone()))
                                // .filter(TermColumn::Publish.eq(true))
                                .all(db)
                                .await
                            {
                                Ok(terms) => {
                                    // Term'leri selected_terms sırasına göre işle (kullanıcının belirlediği sıra)
                                    for term_id in &term_ids {
                                        if let Some(term) = terms.iter().find(|t| t.id == *term_id)
                                        {
                                            match render_term_item(term, lang) {
                                                Ok(rendered_item) => items.push(rendered_item),
                                                Err(e) => {
                                                    eprintln!(
                                                        "Error rendering term {}: {}",
                                                        term.id, e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Error fetching manual terms for vocabulary {}: {}",
                                        vocabulary_id, e
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Otomatik seçim (default)
                    let limit = source.get("limit").and_then(|v| v.as_i64()).unwrap_or(10) as u64;

                    let order_by = source
                        .get("order_by")
                        .and_then(|v| v.as_str())
                        .unwrap_or("order_id_asc");

                    let mut query = Term::find()
                        .filter(TermColumn::VocabularyId.eq(vocabulary_id))
                        // .filter(TermColumn::Publish.eq(true))
                        .limit(limit);

                    // Sıralama
                    match order_by {
                        "order_id_asc" => query = query.order_by_asc(TermColumn::OrderId),
                        "order_id_desc" => query = query.order_by_desc(TermColumn::OrderId),
                        "created_at_asc" => query = query.order_by_asc(TermColumn::CreatedAt),
                        "created_at_desc" => query = query.order_by_desc(TermColumn::CreatedAt),
                        _ => query = query.order_by_asc(TermColumn::OrderId), // default
                    }

                    match query.all(db).await {
                        Ok(terms) => {
                            // Her term'i render et
                            for term in terms {
                                match render_term_item(&term, lang) {
                                    Ok(rendered_item) => items.push(rendered_item),
                                    Err(e) => {
                                        eprintln!("Error rendering term {}: {}", term.id, e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "Error fetching terms for vocabulary {}: {}",
                                vocabulary_id, e
                            );
                        }
                    }
                }
            }
        }
        "manual" => {
            // Manuel seçim
            // eprintln!("🔍 Manual section: processing selected_items");

            if let Some(selected_items) = source.get("selected_items").and_then(|v| v.as_array()) {
                let content_ids: Vec<i64> =
                    selected_items.iter().filter_map(|v| v.as_i64()).collect();

                // eprintln!("🔍 Selected content IDs: {:?}", content_ids);

                if !content_ids.is_empty() {
                    // İçerikleri çek
                    // eprintln!("🔍 Fetching contents from database...");
                    let contents = Content::find()
                        .filter(ContentColumn::Id.is_in(content_ids.clone()))
                        // .filter(ContentColumn::Publish.eq(true)) // publish olmayan sayfalarıda ana sayfada kullanabiliriz
                        .filter(ContentColumn::DeletedAt.is_null())
                        .all(db)
                        .await?;

                    // eprintln!("🔍 Found {} contents", contents.len());

                    // Tag'ları batch olarak çek
                    let tags_map = fetch_tags_for_contents(db, &content_ids, lang).await?;

                    // eprintln!("🔍 Processing contents...");
                    // Content'leri selected_items sırasına göre işle (kullanıcının belirlediği sıra)
                    for content_id in &content_ids {
                        if let Some(content) = contents.iter().find(|c| c.id == *content_id) {
                            let tags = tags_map.get(&content.id).cloned().unwrap_or_default();
                            let rendered_item =
                                render_content_item(content, lang, &tags)
                                    .await?;
                            items.push(rendered_item);
                        }
                    }
                    // eprintln!("🔍 Manual section processing completed");
                }
            }
        }
        _ => {
            // Automatic content source (default)
            let content_type = source
                .get("content_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let limit = source.get("limit").and_then(|v| v.as_i64()).unwrap_or(6) as u64;

            let order_by = source
                .get("order_by")
                .and_then(|v| v.as_str())
                .unwrap_or("created_at_desc");

            let mut query = Content::find()
                .filter(ContentColumn::Publish.eq(true))
                .filter(ContentColumn::DeletedAt.is_null())
                .limit(limit);

            // Content type filtresi
            if !content_type.is_empty() {
                query = query.filter(ContentColumn::ContentType.eq(content_type));
            }

            // Sıralama
            match order_by {
                "created_at_asc" => query = query.order_by_asc(ContentColumn::CreatedAt),
                "title_asc" => query = query.order_by_asc(ContentColumn::Id), // JSON'da title'a göre sıralama zor
                "title_desc" => query = query.order_by_desc(ContentColumn::Id),
                _ => query = query.order_by_desc(ContentColumn::CreatedAt), // created_at_desc default
            }

            let contents = query.all(db).await?;
            let content_ids: Vec<i64> = contents.iter().map(|c| c.id).collect();

            // Tag'ları batch olarak çek
            let tags_map = fetch_tags_for_contents(db, &content_ids, lang).await?;

            // Her content'i render et
            for content in contents {
                let tags = tags_map.get(&content.id).cloned().unwrap_or_default();
                let rendered_item =
                    render_content_item(&content, lang, &tags).await?;
                items.push(rendered_item);
            }
        }
    }

    Ok(RenderedSection {
        id: section_id,
        title,
        description,
        template,
        background_color: section
            .get("background_color")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        settings,
        items,
        active,
        order,
    })
}

// Data manipülasyon fonksiyonu - content type'a göre data'yı işle
fn process_content_data(
    content: &ContentModel,
    lang: &str,
) -> serde_json::Value {
    let mut data = content.data.clone();

    // Apply media fallbacks for the current language
    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut data, lang);

    data
}

fn render_term_item(
    term: &crate::modules::taxonomy::models::term::Model,
    lang: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut data = term.data.clone();

    // Apply media fallbacks for terms too
    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut data, lang);

    let title = get_term_string(&data, "title", lang)
        .or_else(|| get_term_string(&data, "title", "tr"))
        .unwrap_or_else(|| format!("Term {}", term.id));

    let slug = get_term_string(&data, "slug", lang)
        .or_else(|| get_term_string(&data, "slug", "tr"))
        .unwrap_or_else(|| format!("{}-{}", slugify(&title), term.id));

    let description = get_term_string(&data, "description", lang)
        .or_else(|| get_term_string(&data, "description", "tr"));

    let image =
        get_term_string(&data, "image", lang).or_else(|| get_term_string(&data, "image", "tr"));

    // URL oluştur - term için
    // let url = format!("/{}/products/category/{}-{}", lang, slug, term.id);

    let created_at = term.created_at.map(|dt| dt.format("%d.%m.%Y").to_string());

    // Term response
    let response = HomepageContentResponse {
        id: term.id,
        title,
        slug,
        description,
        excerpt: None, // Term'lerde excerpt yok
        price: None,   // Term'lerde price yok
        image,
        url: None,                        //term için ulr ye gerek yok çok karmaşık zaten
        content_type: "term".to_string(), // Özel type
        tags: Vec::new(),                 // Term'lerde tag yok (kendisi zaten term)
        created_at,
        data, // İşlenmiş JSON data
    };

    Ok(serde_json::to_value(response)?)
}

async fn render_content_item(
    content: &ContentModel,
    lang: &str,
    tags: &[TagInfo],
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let title = get_string_from_json(&content.data, lang, "title")
        .unwrap_or_else(|| format!("Content {}", content.id));

    let slug = get_string_from_json(&content.data, lang, "slug")
        .unwrap_or_else(|| format!("content-{}", content.id));

    let description = get_string_from_json(&content.data, lang, "description");
    let excerpt = get_string_from_json(&content.data, lang, "excerpt");
    let image = get_string_from_json(&content.data, lang, "image");
    // URL oluştur
    let url = match content.content_type.as_str() {
        "page" => format!("/{}/page/{}-{}", lang, slug, content.id),
        "blog" => format!("/{}/blog/{}-{}", lang, slug, content.id),
        "news" => format!("/{}/news/{}-{}", lang, slug, content.id),
        "product" => format!("/{}/product/{}-{}", lang, slug, content.id),
        _ => format!("/{}/{}/{}-{}", lang, content.content_type, slug, content.id),
    };

    let created_at = content
        .created_at
        .map(|dt| dt.format("%d.%m.%Y").to_string());

    // Data'yı işle
    let processed_data = process_content_data(content, lang);

    let price = get_string_from_json(&content.data, lang, "price");

    // Unified response
    let response = HomepageContentResponse {
        id: content.id,
        title,
        slug,
        description,
        excerpt,
        price,
        image,
        url: Some(url),
        content_type: content.content_type.clone(),
        tags: tags.to_vec(),
        created_at,
        data: processed_data,
    };

    Ok(serde_json::to_value(response)?)
}

// API: Vocabulary listesi (frontend için)
pub async fn api_get_vocabularies(State(state): State<AppState>, session: Session) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Unauthorized"})),
        )
            .into_response();
    }

    match vocabulary::Entity::find()
        .order_by_asc(vocabulary::Column::OrderId)
        .all(&state.db)
        .await
    {
        Ok(vocabularies) => {
            let vocab_list: Vec<serde_json::Value> = vocabularies
                .into_iter()
                .map(|vocab| {
                    let title = get_term_string(&vocab.data, "title", "tr")
                        .or_else(|| get_term_string(&vocab.data, "title", "en"))
                        .unwrap_or_else(|| format!("Vocabulary {}", vocab.id));

                    serde_json::json!({
                        "id": vocab.id,
                        "title": title,
                        "vocabulary_type": vocab.vocabulary_type,
                        "data": vocab.data
                    })
                })
                .collect();

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "vocabularies": vocab_list
                })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            )
                .into_response()
        }
    }
}

// Helper: Homepage render (content modülü için) - Cache ile
pub async fn get_homepage_render_cached(
    state: &AppState,
    lang: &str,
) -> Result<HomepageRenderResponse, Box<dyn std::error::Error>> {
    // Direkt cache'li render fonksiyonunu kullan
    get_cached_homepage_render(state, lang).await
}

// API: Homepage render (önizleme için) - Cache ile

pub async fn api_render_homepage(
    State(state): State<AppState>,
    Query(params): Query<RenderQuery>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
) -> Response {
    // Sadece admin kullanıcılar erişebilir
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let lang = params.lang.unwrap_or_else(|| "tr".to_string());

    // Cache'den render'ı getir (admin preview - sale_currency kullanır)
    match get_cached_homepage_render(&state, &lang).await {
        Ok(render_response) => (StatusCode::OK, Json(render_response)).into_response(),
        Err(e) => {
            eprintln!("Homepage render error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Homepage render error"})),
            )
                .into_response()
        }
    }
}

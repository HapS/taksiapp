// Page Helper - JSON parsing utilities
use crate::config::get_config;
use crate::modules::content::models::content::Column as ContentColumn;
use sea_orm::*;
use serde::Serialize;
use serde_json::Value;
// use tera::{Function as TeraFunction, Result as TeraResult};
// use std::collections::HashMap;
// use std::sync::Arc;
// use sea_orm::{DatabaseConnection};

/// Load content navigation: parent + all siblings (children of the same parent).
/// Returns Vec<PageResponse> — her nav item tam bir PageResponse olarak döner.
///
/// Logic:
/// - If the current content has a parent_id → find parent, then load parent's children.
///   Nav = [parent] + [children of parent], current page marked is_active.
/// - If the current content has NO parent_id → check if it has children.
///   If yes, nav = [self as parent] + [children], self marked is_active.
///   If no children, nav = empty (no navigation needed).
///
/// This way:
///   A has children B, C, D
///   Viewing A → nav: A(active, parent), B, C, D
///   Viewing C → nav: A(parent), B, C(active), D
pub async fn load_content_nav(
    db: &sea_orm::DatabaseConnection,
    content_id: i64,
    parent_id: Option<i64>,
    language: &str,
) -> Vec<PageResponse> {
    use crate::modules::content::models::Content;

    let content_types = vec!["page", "news", "blog"];

    // Determine the "root" of the nav group
    let root_id = if let Some(pid) = parent_id {
        pid
    } else {
        // No parent — this content IS the root. Check if it has children.
        content_id
    };

    // Load root content (parent)
    let root_content = match Content::find_by_id(root_id)
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null())
        .one(db)
        .await
    {
        Ok(Some(c)) => c,
        _ => return vec![],
    };

    // Load children of root
    let children = match Content::find()
        .filter(ContentColumn::ParentId.eq(root_id))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null())
        .filter(ContentColumn::ContentType.is_in(content_types))
        .order_by_asc(ContentColumn::OrderId)
        .all(db)
        .await
    {
        Ok(items) => items,
        Err(_) => return vec![],
    };

    // If this content is the root and has no children → no nav needed
    if parent_id.is_none() && children.is_empty() {
        return vec![];
    }

    // Tüm content'leri (root + children) bir araya getir
    let mut all_contents = Vec::with_capacity(children.len() + 1);
    all_contents.push(root_content);
    all_contents.extend(children.into_iter().filter(|c| {
        let title = get_string_from_json(&c.data, language, "title").unwrap_or_default();
        !title.is_empty()
    }));

    // Batch tag fetch
    let content_ids: Vec<i64> = all_contents.iter().map(|c| c.id).collect();
    let tags_map = if !content_ids.is_empty() {
        fetch_tags_for_contents(db, &content_ids, language)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    // Her content için PageResponse oluştur, is_active ve is_parent set et
    let mut nav: Vec<PageResponse> = Vec::with_capacity(all_contents.len());
    for (i, content) in all_contents.iter().enumerate() {
        let mut page = to_page_response_with_tags(content, language, &tags_map).await;
        page.is_active = content.id == content_id;
        page.is_parent = i == 0; // İlk item her zaman parent/root
        nav.push(page);
    }

    nav
}

async fn load_subpages(
    db: &sea_orm::DatabaseConnection,
    parent_id: i64,
    language: &str,
) -> Vec<serde_json::Value> {
    use crate::modules::content::models::Content;

    let content_types = vec!["page", "news", "blog"];

    match Content::find()
        .filter(ContentColumn::ParentId.eq(parent_id))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null())
        .filter(ContentColumn::ContentType.is_in(content_types))
        .order_by_asc(ContentColumn::OrderId)
        .all(db)
        .await
    {
        Ok(contents) => contents
            .into_iter()
            .map(|c| {
                let mut map = serde_json::Map::new();
                map.insert("id".to_string(), serde_json::Value::Number(c.id.into()));
                map.insert(
                    "content_type".to_string(),
                    serde_json::Value::String(c.content_type),
                );
                map.insert(
                    "title".to_string(),
                    serde_json::Value::String(
                        get_string_from_json(&c.data, language, "title").unwrap_or_default(),
                    ),
                );
                map.insert(
                    "description".to_string(),
                    serde_json::Value::String(
                        get_string_from_json(&c.data, language, "description").unwrap_or_default(),
                    ),
                );

                map.insert(
                    "slug".to_string(),
                    serde_json::Value::String(
                        get_string_from_json(&c.data, language, "slug").unwrap_or_default(),
                    ),
                );
                map.insert("data".to_string(), c.data);
                serde_json::Value::Object(map)
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Tag for frontend display
#[derive(Serialize, Clone)]
pub struct TagInfo {
    pub id: i64,
    pub title: String,
    pub slug: String,
}

/// PageResponse for frontend templates
#[derive(Serialize, Clone)]
pub struct PageResponse {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub order_id: Option<i32>,
    pub data: Value,
    // pub product: Option<Value>,
    pub publish: bool,
    pub language: String,
    pub content_type: Option<String>,
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub body: String,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub template: Option<String>,
    pub available_languages: Vec<String>,
    pub tags: Vec<TagInfo>,
    pub sub_contents: std::collections::HashMap<String, serde_json::Value>,
    pub subpages: Vec<serde_json::Value>, // Alt sayfalar (page/news/blog)
    pub is_active: bool,                  // content_nav içinde: şu an bakılan sayfa mı?
    pub is_parent: bool,                  // content_nav içinde: parent sayfa mı?

    pub get_absolute_url: Option<String>, // Absolute URL for links

    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Extract string from JSON data based on language and field
/// Supports formats: data.langs.tr.field OR data.tr.field
pub fn get_string_from_json(data: &Value, lang: &str, field: &str) -> Option<String> {
    // Try data.langs.tr.field format
    if let Some(langs) = data.get("langs") {
        if let Some(lang_data) = langs.get(lang) {
            if let Some(value) = lang_data.get(field) {
                return value.as_str().map(|s| s.to_string());
            }
        }
    }

    // Try data.tr.field format (alternative)
    if let Some(lang_data) = data.get(lang) {
        if let Some(value) = lang_data.get(field) {
            return value.as_str().map(|s| s.to_string());
        }
    }

    None
}

/// Get available languages from JSON data
#[allow(dead_code)]
pub fn get_available_languages(data: &Value) -> Vec<String> {
    let config = get_config();

    // Try data.langs format
    if let Some(langs) = data.get("langs") {
        if let Some(obj) = langs.as_object() {
            return obj.keys().cloned().collect();
        }
    }

    // Alternative: check root keys against supported languages
    if let Some(obj) = data.as_object() {
        return obj
            .keys()
            .filter(|k| config.supported_languages.contains_key(*k))
            .cloned()
            .collect();
    }

    vec![]
}

/// Check if content exists in given language
pub fn has_content_in_language(data: &Value, language: &str) -> bool {
    let title =
        get_string_from_json(data, language, "title").unwrap_or_else(|| "Başlıksız".to_string());
    !title.is_empty() && title != "Başlıksız"
}

/// Batch fetch tags for multiple contents (N+1 query fix)
pub async fn fetch_tags_for_contents(
    db: &sea_orm::DatabaseConnection,
    content_ids: &[i64],
    language: &str,
) -> Result<std::collections::HashMap<i64, Vec<TagInfo>>, sea_orm::DbErr> {
    use crate::modules::content::models::{content_terms, ContentTerm};
    use crate::modules::taxonomy::models::{term, Term};
    use sea_orm::*;

    if content_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    // Get all content_terms for these contents
    let content_terms = ContentTerm::find()
        .filter(content_terms::Column::ContentId.is_in(content_ids.to_vec()))
        .all(db)
        .await?;

    if content_terms.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    // Group by content_id
    let mut content_term_map: std::collections::HashMap<i64, Vec<i64>> =
        std::collections::HashMap::new();
    for ct in &content_terms {
        content_term_map
            .entry(ct.content_id)
            .or_insert_with(Vec::new)
            .push(ct.term_id);
    }

    // Get all term IDs
    let term_ids: Vec<i64> = content_terms.iter().map(|ct| ct.term_id).collect();

    // Settings'ten sayfa kategorileri vocabulary ID'sini al (tags için)
    let vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(db, "tags_categories")
            .await
            .unwrap_or(4); // Fallback olarak 4 kullan

    // Fetch all terms in one query
    let terms = Term::find()
        .filter(term::Column::Id.is_in(term_ids))
        .filter(term::Column::VocabularyId.eq(vocab_id)) // Tags vocabulary
        .filter(term::Column::Publish.eq(true))
        .all(db)
        .await?;

    // Create term_id -> TagInfo map
    let term_info_map: std::collections::HashMap<i64, TagInfo> = terms
        .into_iter()
        .map(|term| {
            let title = get_string_from_json(&term.data, language, "title")
                .unwrap_or_else(|| format!("Tag {}", term.id));
            let slug = get_string_from_json(&term.data, language, "slug")
                .unwrap_or_else(|| format!("tag-{}", term.id));

            (
                term.id,
                TagInfo {
                    id: term.id,
                    title,
                    slug,
                },
            )
        })
        .collect();

    // Build final map: content_id -> Vec<TagInfo>
    let mut result: std::collections::HashMap<i64, Vec<TagInfo>> = std::collections::HashMap::new();
    for (content_id, term_ids) in content_term_map {
        let tags: Vec<TagInfo> = term_ids
            .iter()
            .filter_map(|tid| term_info_map.get(tid).cloned())
            .collect();
        result.insert(content_id, tags);
    }

    Ok(result)
}

/// Get tags for content
async fn get_content_tags(
    db: &sea_orm::DatabaseConnection,
    content_id: i64,
    content_type: &str,
    language: &str,
) -> Vec<TagInfo> {
    use crate::modules::content::models::{content_terms, ContentTerm};
    use crate::modules::taxonomy::models::{term, Term};
    use sea_orm::*;

    // Get term IDs for this content
    let content_terms = match ContentTerm::find()
        .filter(content_terms::Column::ContentId.eq(content_id))
        .filter(content_terms::Column::ContentType.eq(content_type))
        .all(db)
        .await
    {
        Ok(terms) => terms,
        Err(_) => return vec![],
    };

    let term_ids: Vec<i64> = content_terms.iter().map(|ct| ct.term_id).collect();
    if term_ids.is_empty() {
        return vec![];
    }

    // Settings'ten tags kategorileri vocabulary ID'sini al
    let vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(db, "tags_categories")
            .await
            .unwrap_or(7); // Fallback olarak 7 kullan

    // Get terms (tags from vocabulary_id = 4)
    let terms = match Term::find()
        .filter(term::Column::Id.is_in(term_ids))
        .filter(term::Column::VocabularyId.eq(vocab_id)) // Tags vocabulary
        .filter(term::Column::Publish.eq(true))
        .all(db)
        .await
    {
        Ok(terms) => terms,
        Err(_) => return vec![],
    };

    // Convert to TagInfo
    let taglar: Vec<TagInfo> = terms
        .iter()
        .map(|term| {
            let title = get_string_from_json(&term.data, language, "title")
                .unwrap_or_else(|| format!("Tag {}", term.id));
            let slug = get_string_from_json(&term.data, language, "slug")
                .unwrap_or_else(|| format!("tag-{}", term.id));

            TagInfo {
                id: term.id,
                title,
                slug,
            }
        })
        .collect();

    // println!("Found {} tags for content {}", taglar.len(), content_id);
    // for tag in &taglar {
    //     println!(" - Tag: {} (slug: {})", tag.title, tag.slug);
    // }
    taglar
}

/// Convert ContentModel to PageResponse for a specific language
pub async fn to_page_response(
    content: &crate::modules::content::models::ContentModel,
    language: &str,
    db: &sea_orm::DatabaseConnection,
) -> PageResponse {
    let config = get_config();
    let lang = config.get_language_or_default(Some(language));

    let template = content
        .data
        .get("template")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Get tags for this content
    let tags = get_content_tags(db, content.id, &content.content_type, &lang).await;

    // Load sub contents for this content
    let sub_contents = crate::modules::content::helpers::sub_content_helper::load_sub_contents(
        db,
        content.id,
        Some(&lang),
    )
    .await
    .unwrap_or_default();

    // Load subpages: parent_id = content.id olan page/news/blog içerikleri
    let subpages = load_subpages(db, content.id, language).await;

    // Generate absolute URL
    let get_absolute_url = content.get_absolute_url(&lang);

    let mut data = content.data.clone();
    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut data, &lang);

    PageResponse {
        id: content.id,
        parent_id: content.parent_id,
        order_id: content.order_id,
        data: data,
        // product: content.data.get("product").cloned(),
        publish: content.publish,
        language: lang.clone(),
        content_type: Some(content.content_type.to_string()),

        title: get_string_from_json(&content.data, &lang, "title")
            .unwrap_or_else(|| "Başlıksız".to_string()),
        slug: get_string_from_json(&content.data, &lang, "slug").unwrap_or_default(),
        description: get_string_from_json(&content.data, &lang, "description"),
        body: get_string_from_json(&content.data, &lang, "body").unwrap_or_default(),
        meta_title: get_string_from_json(&content.data, &lang, "meta_title"),
        meta_description: get_string_from_json(&content.data, &lang, "meta_description"),
        template,
        available_languages: get_available_languages(&content.data),
        tags,
        sub_contents,
        subpages,
        is_active: false,
        is_parent: false,
        get_absolute_url,

        created_at: content.created_at.map(|dt| dt.naive_utc().and_utc()),
        updated_at: content.updated_at.map(|dt| dt.naive_utc().and_utc()),
    }
}
pub async fn to_page_response_with_tags(
    content: &crate::modules::content::models::ContentModel,
    language: &str,
    tags_map: &std::collections::HashMap<i64, Vec<TagInfo>>,
) -> PageResponse {
    let config = get_config();
    let lang = config.get_language_or_default(Some(language));

    let template = content
        .data
        .get("template")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Get tags from pre-fetched map
    let tags = tags_map.get(&content.id).cloned().unwrap_or_default();

    // For batch operations, don't load sub contents (performance)
    let sub_contents = std::collections::HashMap::new();
    let subpages = vec![]; // Batch işlemlerinde yüklenmiyor
                           // Generate absolute URL
    let get_absolute_url = content.get_absolute_url(&lang);

    let mut data = content.data.clone();
    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut data, &lang);

    PageResponse {
        id: content.id,
        parent_id: content.parent_id,
        order_id: content.order_id,
        data: data,
        // product: content.data.get("product").cloned(),
        publish: content.publish,
        language: lang.clone(),
        content_type: Some(content.content_type.to_string()),

        title: get_string_from_json(&content.data, &lang, "title")
            .unwrap_or_else(|| "Başlıksız".to_string()),
        slug: get_string_from_json(&content.data, &lang, "slug").unwrap_or_default(),
        description: get_string_from_json(&content.data, &lang, "description"),
        body: get_string_from_json(&content.data, &lang, "body").unwrap_or_default(),
        meta_title: get_string_from_json(&content.data, &lang, "meta_title"),
        meta_description: get_string_from_json(&content.data, &lang, "meta_description"),
        template,
        available_languages: get_available_languages(&content.data),
        tags,
        sub_contents,
        subpages,
        is_active: false,
        is_parent: false,
        get_absolute_url,

        created_at: content.created_at.map(|dt| dt.naive_utc().and_utc()),
        updated_at: content.updated_at.map(|dt| dt.naive_utc().and_utc()),
    }
}

// Admin Page API Controllers - JSON responses for AJAX calls
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use sea_orm::*;
use serde::{Deserialize, Serialize};

use super::super::dto::{ContentExtensions, CreateContentRequest, UpdateContentRequest};
use crate::app_state::AppState;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::content::models::content::Column as ContentColumn;
use crate::modules::content::models::{Content, ContentActiveModel};
use crate::modules::media::services::media_service;
use crate::modules::timeline::helpers::TimelineHelper;
use crate::modules::utils::terms_utils::{build_term_hierarchy, NestedTerm};
// ============ API QUERY/RESPONSE MODELS ============

#[derive(Deserialize)]
pub struct ContentQueryParams {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
    pub parent_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub content_type: Option<String>,
    // Sorting support
    pub sort_by: Option<String>,
    // Expected values: "asc" or "desc"
    pub sort_order: Option<String>,
    // Homepage builder için yeni parametreler
    pub for_homepage_builder: Option<bool>, // Homepage builder için basitleştirilmiş response
    pub simple_format: Option<bool>,        // Sadece id, title, type döndür
}

#[derive(Serialize)]
pub struct ContentListResponse {
    pub id: i64,
    pub content_type: String,
    pub data: serde_json::Value,
    pub publish: bool,
    pub parent_id: Option<i64>,
    pub parent_title: Option<String>,
    pub children_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct TermQueryParams {
    pub ghost: Option<bool>,
}

// Homepage builder için basitleştirilmiş response
#[derive(Serialize)]
pub struct SimpleContentResponse {
    pub id: i64,
    pub title: String,
    pub content_type: String,
}

#[derive(Serialize)]
pub struct PaginatedResponse {
    pub data: Vec<ContentListResponse>,
    pub meta: PaginationMeta,
}

#[derive(Serialize)]
pub struct PaginationMeta {
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub total_pages: i64,
    pub breadcrumbs: Vec<BreadcrumbItem>,
}

//content type enum, page,blog,news,product,form
// enum contentTypeEnum {
//     Page(String),
//     Blog(String),
//     News(String),
//     Product(String),
//     Form(String),
//     Slider(String),
// }

use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentType {
    Page,
    Blog,
    News,
    Product,
    Form,
    Slider,
}

impl FromStr for ContentType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "page" => Ok(ContentType::Page),
            "blog" => Ok(ContentType::Blog),
            "news" => Ok(ContentType::News),
            "product" => Ok(ContentType::Product),
            "form" => Ok(ContentType::Form),
            "slider" => Ok(ContentType::Slider),
            _ => Err(()),
        }
    }
}

impl ContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentType::Page => "page",
            ContentType::Blog => "blog",
            ContentType::News => "news",
            ContentType::Product => "product",
            ContentType::Form => "form",
            ContentType::Slider => "slider",
        }
    }
}

// BreadcrumbItem artık DTO'da tanımlı
pub use super::super::dto::content::BreadcrumbItem;

// ============ API ENDPOINTS ============

// API: Admin sayfa listesi (gelişmiş filtreleme ile)
pub async fn admin_api_list_contents(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Query(query): Query<ContentQueryParams>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    // Base query
    let mut select = Content::find().filter(ContentColumn::DeletedAt.is_null());

    // Content type filtresi - boş değilse filtrele, boşsa tüm tipleri getir
    if let Some(content_type) = &query.content_type {
        if !content_type.is_empty() && content_type != "all" {
            select = select.filter(ContentColumn::ContentType.eq(content_type.as_str()));
        }
    }

    // Parent ID filtresi
    if let Some(parent_id_str) = &query.parent_id {
        if parent_id_str == "null" {
            select = select.filter(ContentColumn::ParentId.is_null());
        } else if let Ok(parent_id) = parent_id_str.parse::<i64>() {
            select = select.filter(ContentColumn::ParentId.eq(parent_id));
        }
    }

    // Arama filtresi (JSON içinde arama)
    if let Some(search) = &query.search {
        if !search.is_empty() {
            // PostgreSQL JSONB'de text arama için ::text cast gerekli
            let search_pattern = format!("%{}%", search.to_lowercase());

            // SeaORM'un custom expression'ı ile güvenli arama
            use sea_orm::sea_query::{Expr, ExprTrait, Func};

            // LOWER(data::text) LIKE '%search%' şeklinde SQL oluşturur
            select = select.filter(
                Func::lower(Func::cast_as(
                    Expr::col(ContentColumn::Data),
                    sea_orm::sea_query::Alias::new("text"),
                ))
                .like(search_pattern),
            );
        }
    }

    // Tarih filtreleri
    if let Some(start_date) = &query.start_date {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
            let datetime = date.and_hms_opt(0, 0, 0).unwrap();
            select = select.filter(ContentColumn::CreatedAt.gte(datetime));
        }
    }

    if let Some(end_date) = &query.end_date {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
            let datetime = date.and_hms_opt(23, 59, 59).unwrap();
            select = select.filter(ContentColumn::CreatedAt.lte(datetime));
        }
    }

    // Toplam sayı
    let total = match select.clone().count(&state.db).await {
        Ok(count) => count as i64,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Veri çekme - sort_by & sort_order destekleniyor (id, content_type, publish/status, created_at)
    let mut sort_applied = false;
    if let Some(sb) = &query.sort_by {
        if !sb.is_empty() {
            let order = query
                .sort_order
                .clone()
                .unwrap_or_else(|| "desc".to_string())
                .to_lowercase();
            match sb.as_str() {
                "id" => {
                    if order == "asc" {
                        select = select.order_by_asc(ContentColumn::Id);
                    } else {
                        select = select.order_by_desc(ContentColumn::Id);
                    }
                    sort_applied = true;
                }
                "content_type" | "type" => {
                    if order == "asc" {
                        select = select.order_by_asc(ContentColumn::ContentType);
                    } else {
                        select = select.order_by_desc(ContentColumn::ContentType);
                    }
                    sort_applied = true;
                }
                "publish" | "status" => {
                    if order == "asc" {
                        select = select.order_by_asc(ContentColumn::Publish);
                    } else {
                        select = select.order_by_desc(ContentColumn::Publish);
                    }
                    sort_applied = true;
                }
                "created_at" | "date" => {
                    if order == "asc" {
                        select = select.order_by_asc(ContentColumn::CreatedAt);
                    } else {
                        select = select.order_by_desc(ContentColumn::CreatedAt);
                    }
                    sort_applied = true;
                }
                "order_id" => {
                    if order == "asc" {
                        select = select.order_by_asc(ContentColumn::OrderId);
                    } else {
                        select = select.order_by_desc(ContentColumn::OrderId);
                    }
                    sort_applied = true;
                }
                _ => {
                    // bilinen bir alan değilse yoksay ve varsayılan uygulanacak
                }
            }
        }
    }

    // Eğer sıralama belirtilmemişse varsayılan: order_id asc, sonra created_at desc
    if !sort_applied {
        select = select
            .order_by_asc(ContentColumn::OrderId)
            .order_by_desc(ContentColumn::CreatedAt);
    }

    let pages = match select
        .offset(offset as u64)
        .limit(limit as u64)
        .all(&state.db)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Batch queries for N+1 fix
    // 1. Get all parent IDs and fetch in one query
    let parent_ids: Vec<i64> = pages.iter().filter_map(|p| p.parent_id).collect();

    let parents_map: std::collections::HashMap<i64, String> = if !parent_ids.is_empty() {
        Content::find()
            .filter(ContentColumn::Id.is_in(parent_ids))
            .all(&state.db)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| {
                let title = p
                    .as_content_data()
                    .ok()
                    .and_then(|data| data.langs.get("tr").map(|l| l.title.clone()));
                title.map(|t| (p.id, t))
            })
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // 2. Get children counts in one query
    let page_ids: Vec<i64> = pages.iter().map(|p| p.id).collect();
    let children_counts: std::collections::HashMap<i64, i64> = if !page_ids.is_empty() {
        // GROUP BY parent_id and COUNT
        let counts = Content::find()
            .filter(ContentColumn::ParentId.is_in(page_ids.clone()))
            .filter(ContentColumn::DeletedAt.is_null())
            .all(&state.db)
            .await
            .unwrap_or_default();

        let mut map = std::collections::HashMap::new();
        for content in counts {
            if let Some(parent_id) = content.parent_id {
                *map.entry(parent_id).or_insert(0) += 1;
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let total_pages = (total as u64 + limit as u64 - 1) / limit as u64;

    // Homepage builder için basitleştirilmiş response
    if query.simple_format.unwrap_or(false) || query.for_homepage_builder.unwrap_or(false) {
        let simple_data: Vec<SimpleContentResponse> = pages
            .into_iter()
            .filter_map(|page_model| {
                // JSON data'dan title'ı çıkar
                let title = page_model
                    .as_content_data()
                    .ok()
                    .and_then(|data| {
                        // Önce Türkçe title'ı dene, yoksa ilk dili al
                        data.langs
                            .get("tr")
                            .or_else(|| data.langs.values().next())
                            .map(|lang| lang.title.clone())
                    })
                    .unwrap_or_else(|| format!("İçerik {}", page_model.id));

                Some(SimpleContentResponse {
                    id: page_model.id,
                    title,
                    content_type: page_model.content_type,
                })
            })
            .collect();

        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "data": simple_data,
                "meta": {
                    "total": total,
                    "page": page,
                    "limit": limit,
                    "total_pages": total_pages as i64
                }
            })),
        )
            .into_response();
    }

    // Normal response formatı
    let mut response_data = Vec::new();
    for page_model in pages {
        // Get parent title from pre-fetched map
        let parent_title = page_model
            .parent_id
            .and_then(|pid| parents_map.get(&pid).cloned());

        // Get children count from pre-fetched map
        let children_count = children_counts.get(&page_model.id).copied().unwrap_or(0);

        response_data.push(ContentListResponse {
            id: page_model.id,
            content_type: page_model.content_type.clone(),
            data: page_model.data.clone(),
            publish: page_model.publish,
            parent_id: page_model.parent_id,
            parent_title,
            children_count,
            created_at: page_model
                .created_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default(),
            updated_at: page_model
                .updated_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default(),
        });
    }

    // Build breadcrumbs if parent_id is set
    let breadcrumbs = if let Some(parent_id_str) = &query.parent_id {
        if parent_id_str != "null" {
            if let Ok(parent_id) = parent_id_str.parse::<i64>() {
                build_page_breadcrumbs(&state.db, parent_id)
                    .await
                    .unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    (
        StatusCode::OK,
        Json(PaginatedResponse {
            data: response_data,
            meta: PaginationMeta {
                total,
                page,
                limit,
                total_pages: total_pages as i64,
                breadcrumbs,
            },
        }),
    )
        .into_response()
}

// API: Admin sayfa oluştur
pub async fn admin_api_create_content(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Json(json): Json<CreateContentRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let ct_str = json.content_type.as_deref().unwrap_or("page");
    let ct = match ContentType::from_str(ct_str) {
        Ok(x) => x,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_content_type",
                    "message": "Invalid content_type",
                    "allowed": ["page", "blog", "news", "product", "form", "slider"]
                })),
            )
                .into_response();
        }
    };
    let content_type = ct.as_str().to_string();

    // Get max order_id for this content_type
    let max_order_id = Content::find()
        .filter(ContentColumn::ContentType.eq(content_type.as_str()))
        .filter(ContentColumn::DeletedAt.is_null())
        .order_by_desc(ContentColumn::OrderId)
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|c| c.order_id)
        .unwrap_or(0);

    let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();

    // Content type'a göre data'yı temizle
    let mut cleaned_data = json.data.clone();

    // Form tipinde ise product field'ını kaldır
    if content_type == "form" {
        if let Some(data_obj) = cleaned_data.as_object_mut() {
            data_obj.remove("product");
            data_obj.remove("term_master_id");
        }
    }

    if content_type == "page" || content_type == "blog" || content_type == "news" {
        if let Some(data_obj) = cleaned_data.as_object_mut() {
            data_obj.remove("product");
        }
    }

    let active_model = ContentActiveModel {
        data: Set(cleaned_data), // Temizlenmiş JSON'ı kaydet
        content_type: Set(content_type.to_string()),
        publish: Set(json.publish.unwrap_or(false)),
        gcx: Set(json.gcx.unwrap_or(false)),
        parent_id: Set(json.parent_id),
        order_id: Set(Some(max_order_id + 1)),
        user_id: Set(Some(auth_user.id)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    };

    let page = match active_model.insert(&state.db).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Term'leri ekle (many-to-many)
    if let Some(term_ids) = &json.term_ids {
        if !term_ids.is_empty() {
            if let Err(e) =
                save_content_terms(&state.db, page.id, term_ids, &page.content_type).await
            {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        }
    }

    // Tag'leri ekle (many-to-many)
    if let Some(tag_ids) = &json.tag_ids {
        if !tag_ids.is_empty() {
            if let Err(e) =
                save_content_terms(&state.db, page.id, tag_ids, &page.content_type).await
            {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        }
    }

    // Timeline event oluştur - content oluşturuldu
    let admin_user_id = auth_user.id;
    let action = if page.publish {
        "published" // Direkt yayınlandı
    } else {
        "created" // Oluşturuldu ama henüz yayınlanmadı
    };

    // Metadata hazırla
    let metadata = serde_json::json!({
        "content_type": page.content_type,
        "publish_status": page.publish,
        "has_parent": page.parent_id.is_some(),
        "term_count": json.term_ids.as_ref().map(|t| t.len()).unwrap_or(0) +
                     json.tag_ids.as_ref().map(|t| t.len()).unwrap_or(0)
    });

    // Timeline event oluştur
    if let Err(e) = TimelineHelper::create_content_event(
        &state.db,
        page.id,
        &page.content_type,
        action,
        Some(admin_user_id),
        Some(metadata),
    )
    .await
    {
        eprintln!("Timeline event oluşturma hatası: {}", e);
        // Timeline hatası ana işlemi etkilemesin, sadece log'la
    }

    // Global context cache'ini yenile (her content değişikliğinde)
    // Çünkü gcx=false olan bir content gcx=true'ya çevrilebilir
    if let Err(e) =
        crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
            &state.db,
            &state.global_context_cache,
        )
        .await
    {
        eprintln!("Global context cache yenileme hatası: {}", e);
        // Cache hatası ana işlemi etkilemesin
    }

    // Homepage render cache'ini temizle
    crate::modules::admin::controllers::web::homepage::clear_homepage_cache(&state);

    (StatusCode::CREATED, Json(page.get_admin_content_response())).into_response()
}

// API: Admin sayfa getir
pub async fn admin_api_get_content(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = match Content::find_by_id(id)
        // .filter(ContentColumn::ContentType.eq("page"))
        .filter(ContentColumn::DeletedAt.is_null())
        .one(&state.db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Page not found").into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Term ID'lerini al (hem category hem tag)
    let all_term_ids = match get_content_term_ids(&state.db, id, &page.content_type).await {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Breadcrumb'ları oluştur
    let breadcrumbs = if let Some(parent_id) = page.parent_id {
        build_page_breadcrumbs(&state.db, parent_id)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut response = page.get_admin_content_response();
    // Şimdilik tüm term_ids'i hem term_ids hem tag_ids'e koy
    // Frontend'de vocabulary'ye göre filtrelenecek
    response.term_ids = all_term_ids.clone();
    response.tag_ids = all_term_ids;
    response.breadcrumbs = breadcrumbs;

    (StatusCode::OK, Json(response)).into_response()
}

// API: Admin sayfa güncelle
pub async fn admin_api_update_content(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(id): Path<i64>,
    Json(json): Json<UpdateContentRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = match Content::find_by_id(id)
        // .filter(ContentColumn::ContentType.eq("page"))
        .filter(ContentColumn::DeletedAt.is_null())
        .one(&state.db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Page not found").into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    let mut active_model: ContentActiveModel = page.clone().into();

    // Data güncelleme - mevcut data'yı al ve birleştir
    if let Some(new_data) = &json.data {
        // println!("Updating page data: {:?}", new_data);

        // Mevcut data'yı al
        let mut current_data = page.data.clone();

        // Eğer current_data bir object ise, yeni data'yı birleştir
        if let Some(current_obj) = current_data.as_object_mut() {
            if let Some(new_obj) = new_data.as_object() {
                // Yeni alanları mevcut data'ya ekle/güncelle
                for (key, value) in new_obj {
                    current_obj.insert(key.clone(), value.clone());
                }
            }
        } else {
            // Eğer mevcut data object değilse, direkt yeni data'yı kullan
            current_data = new_data.clone();
        }

        active_model.data = Set(current_data);
    }

    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    if let Some(publish) = json.publish {
        active_model.publish = Set(publish);
    }

    if let Some(gcx) = json.gcx {
        active_model.gcx = Set(gcx);
    }

    let ct_str = json.content_type.as_deref().unwrap_or("page");
    let ct = match ContentType::from_str(ct_str) {
        Ok(x) => x,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_content_type",
                    "message": "Invalid content_type",
                    "allowed": ["page", "blog", "news", "product", "form", "slider"]
                })),
            )
                .into_response();
        }
    };
    let content_type = ct.as_str().to_string();

    // Content type değişikliğini kontrol et
    let content_type_changed = if let Some(new_content_type) = Some(&content_type) {
        let changed = page.content_type != *new_content_type;
        if changed {
            eprintln!(
                "Content type changed: {} -> {}",
                page.content_type, new_content_type
            );
        }
        active_model.content_type = Set(new_content_type.clone());
        changed
    } else {
        false
    };

    // Form tipinde ise product field'ını kaldır
    let final_content_type = json.content_type.as_ref().unwrap_or(&page.content_type);
    if final_content_type == "form" {
        if let Set(ref mut data) = active_model.data {
            if let Some(data_obj) = data.as_object_mut() {
                data_obj.remove("product");
                data_obj.remove("term_master_id");
            }
        }
    }

    // Parent ID güncelleme - None olabilir (üst sayfayı kaldırma)
    active_model.parent_id = Set(json.parent_id);

    let updated_page = match active_model.update(&state.db).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Term'leri ve Tag'leri güncelle (önce hepsini sil, sonra ekle)
    // Content type değişmişse veya term_ids/tag_ids güncelleniyorsa, eski term'leri sil
    if content_type_changed || json.term_ids.is_some() || json.tag_ids.is_some() {
        eprintln!(
            "Deleting old terms for content_id={} (content_type_changed: {})",
            id, content_type_changed
        );
        if let Err(e) = delete_content_terms(&state.db, id, &updated_page.content_type).await {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }

        // Tüm term_ids ve tag_ids'i birleştir ve unique yap
        let mut all_term_ids = Vec::new();

        if let Some(term_ids) = &json.term_ids {
            all_term_ids.extend(term_ids);
        }

        if let Some(tag_ids) = &json.tag_ids {
            all_term_ids.extend(tag_ids);
        }

        // Duplicate'leri kaldır
        all_term_ids.sort();
        all_term_ids.dedup();

        // Yeni term'leri ekle
        if !all_term_ids.is_empty() {
            eprintln!(
                "Saving {} terms for content_id={} with content_type={}: {:?}",
                all_term_ids.len(),
                id,
                updated_page.content_type,
                all_term_ids
            );
            if let Err(e) =
                save_content_terms(&state.db, id, &all_term_ids, &updated_page.content_type).await
            {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
            eprintln!(
                "Terms saved successfully with content_type={}",
                updated_page.content_type
            );
        } else {
            eprintln!("No terms to save for content_id={}", id);
        }
    }

    // Timeline event oluştur - content güncellendi
    let admin_user_id = auth_user.id;
    let action = if json.publish == Some(true) && !page.publish {
        "published" // Yayınlandı
    } else if json.publish == Some(false) && page.publish {
        "unpublished" // Yayından kaldırıldı
    } else {
        "updated" // Güncellendi
    };

    // Metadata hazırla
    let metadata = serde_json::json!({
        "content_type": updated_page.content_type,
        "publish_status": updated_page.publish,
        "updated_fields": {
            "data_updated": json.data.is_some(),
            "publish_changed": json.publish.is_some(),
            "content_type_changed": content_type_changed,
            "parent_changed": json.parent_id != page.parent_id,
            "terms_updated": json.term_ids.is_some() || json.tag_ids.is_some()
        }
    });

    // Timeline event oluştur
    if let Err(e) = TimelineHelper::create_content_event(
        &state.db,
        updated_page.id,
        &updated_page.content_type,
        action,
        Some(admin_user_id),
        Some(metadata),
    )
    .await
    {
        eprintln!("Timeline event oluşturma hatası: {}", e);
        // Timeline hatası ana işlemi etkilemesin, sadece log'la
    }

    // Global context cache'ini yenile (her content değişikliğinde)
    // Çünkü gcx=false olan bir content gcx=true'ya çevrilebilir veya
    // gcx=true olan bir content güncellenmiş olabilir
    if let Err(e) =
        crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
            &state.db,
            &state.global_context_cache,
        )
        .await
    {
        eprintln!("Global context cache yenileme hatası: {}", e);
        // Cache hatası ana işlemi etkilemesin
    }

    // Homepage render cache'ini temizle
    crate::modules::admin::controllers::web::homepage::clear_homepage_cache(&state);

    (
        StatusCode::OK,
        Json(updated_page.get_admin_content_response()),
    )
        .into_response()
}

// API: Admin sayfa sıralamasını güncelle
#[derive(Deserialize)]
pub struct ContentOrderItem {
    pub id: i64,
    pub order_id: i32,
}

#[derive(Deserialize)]
pub struct UpdateContentOrderRequest {
    pub orders: Vec<ContentOrderItem>,
}

pub async fn admin_api_update_content_order(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Json(json): Json<UpdateContentOrderRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Update each page's order_id
    for order_item in json.orders {
        let page = match Content::find_by_id(order_item.id)
            .filter(ContentColumn::DeletedAt.is_null())
            .one(&state.db)
            .await
        {
            Ok(Some(p)) => p,
            Ok(None) => continue, // Skip if not found
            Err(e) => {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        };

        let mut active_model: ContentActiveModel = page.into();
        active_model.order_id = Set(Some(order_item.order_id));

        if let Err(e) = active_model.update(&state.db).await {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    }

    if let Err(e) =
        crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
            &state.db,
            &state.global_context_cache,
        )
        .await
    {
        eprintln!("Global context cache yenileme hatası: {}", e);
        // Cache hatası ana işlemi etkilemesin
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Page order updated successfully"
        })),
    )
        .into_response()
}

// API: Admin sayfa sil
pub async fn admin_api_delete_content(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    // Timeline için önce content bilgilerini al
    let content = match Content::find_by_id(id)
        .filter(ContentColumn::DeletedAt.is_null())
        .one(&state.db)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Content not found").into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // // datayı fonksiyona gönder ve dil içindeki tüm media ID'lerini topla
    let mut media_ids: Vec<i64> = Vec::new();
    collect_media_ids_from_value(&content.data, &mut media_ids);
    media_ids.sort();
    media_ids.dedup();

    // Delete content
    if let Err(e) = Content::delete_by_id(id).exec(&state.db).await {
        eprintln!("Veritabanı hatası: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
    }

    // İlişkili media dosyalarını sil
    if !media_ids.is_empty() {
        let upload_root = state.config.media_upload_root().to_string();
        for mid in media_ids {
            match media_service::delete_media_and_file(&state.db, mid, &upload_root).await {
                Ok(_) => eprintln!("Deleted media id={} associated with content id={}", mid, id),
                Err(e) => eprintln!(
                    "Failed to delete media id={} for content id={} : {}",
                    mid, id, e
                ),
            }
        }
    }

    // Timeline event oluştur - content silindi
    let admin_user_id = auth_user.id;
    // Metadata hazırla
    let metadata = serde_json::json!({
        "content_type": content.content_type,
        "was_published": content.publish,
        "had_parent": content.parent_id.is_some(),
        "deleted_at": chrono::Utc::now().to_rfc3339()
    });

    // Timeline event oluştur
    if let Err(e) = TimelineHelper::create_content_event(
        &state.db,
        content.id,
        &content.content_type,
        "deleted",
        Some(admin_user_id),
        Some(metadata),
    )
    .await
    {
        eprintln!("Timeline event oluşturma hatası: {}", e);
        // Timeline hatası ana işlemi etkilemesin, sadece log'la
    }

    // Global context cache'ini yenile (her content silindiğinde)
    // Çünkü silinen content gcx=true olabilir
    if let Err(e) =
        crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
            &state.db,
            &state.global_context_cache,
        )
        .await
    {
        eprintln!("Global context cache yenileme hatası: {}", e);
        // Cache hatası ana işlemi etkilemesin
    }

    // Homepage render cache'ini temizle
    crate::modules::admin::controllers::web::homepage::clear_homepage_cache(&state);

    StatusCode::NO_CONTENT.into_response()
}

// API: Desteklenen dilleri listele
pub async fn admin_api_list_languages(
    _auth_user: crate::middleware::auth::AuthenticatedUser,
) -> Response {
    // Admin access kontrolü - dil listesi için gerekli değil, sadece authentication yeterli
    // Ama tutarlılık için ekleyelim
    // Not: State parametresi yok, bu endpoint için admin kontrolü yapmayalım
    let config = crate::config::get_config();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "supported_languages": config.supported_languages,
            "default_language": config.default_language
        })),
    )
        .into_response()
}

// API: Temadaki sayfa şablonlarını (templates) listele
pub async fn admin_api_list_templates(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let theme = state.get_frontend_theme();
    let pages_dir = format!("templates/{}/pages", theme);

    let mut templates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&pages_dir) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                // Sadece .html dosyalarını ve içinde "list" geçmeyenleri al
                if file_name.ends_with(".html") && !file_name.to_lowercase().contains("list") {
                    templates.push(file_name.to_string());
                }
            }
        }
    }

    // Alfabetik sırala
    templates.sort();

    (StatusCode::OK, Json(templates)).into_response()
}

// API: Temadaki ana sayfa bölüm şablonlarını (sections) listele
pub async fn admin_api_list_section_templates(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let theme = state.get_frontend_theme();
    let sections_dir = format!("templates/{}/home/sections", theme);

    let mut templates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&sections_dir) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                // Sadece .html dosyalarını al
                if file_name.ends_with(".html") {
                    templates.push(file_name.to_string());
                }
            }
        }
    }

    // Alfabetik sırala
    templates.sort();

    (StatusCode::OK, Json(templates)).into_response()
}

// API: Content type'a göre term'leri getir (recursive)
pub async fn admin_api_get_terms_by_content_type(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(content_type): Path<String>,
    Query(query): Query<TermQueryParams>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Content type'a göre vocabulary ID'lerini belirle
    // TODO: Bu mapping'i config'den veya database'den al

    let page_vocab_id = crate::modules::admin::services::settings_service::get_vocab_id(
        &state.db,
        "page_categories",
    )
    .await
    .unwrap_or(2); // Fallback olarak 2 kullan
    let blog_vocab_id = crate::modules::admin::services::settings_service::get_vocab_id(
        &state.db,
        "blog_categories",
    )
    .await
    .unwrap_or(2);

    let product_vocab_id = crate::modules::admin::services::settings_service::get_vocab_id(
        &state.db,
        "product_categories",
    )
    .await
    .unwrap_or(2);

    let news_vocab_id = crate::modules::admin::services::settings_service::get_vocab_id(
        &state.db,
        "news_categories",
    )
    .await
    .unwrap_or(2);

    let vocabulary_ids: Vec<i64> = match content_type.as_str() {
        "page" => vec![page_vocab_id],       // Sayfa kategorileri
        "blog" => vec![blog_vocab_id],       // Blog kategorileri
        "product" => vec![product_vocab_id], // Ürün kategorileri
        "news" => vec![news_vocab_id],       // Haber kategorileri
        _ => vec![],
    };

    if vocabulary_ids.is_empty() {
        return (StatusCode::OK, Json(Vec::<NestedTerm>::new())).into_response();
    }

    use crate::modules::taxonomy::models::{term, Term};

    // Tüm term'leri çek

    //parametre ghost true ise all term publish false olanlar da çekilecek

    println!("ghost parametresi: {:?}", query.ghost);

    if query.ghost.unwrap_or(false) {
        let all_terms = match Term::find()
            .filter(term::Column::VocabularyId.is_in(vocabulary_ids))
            .order_by_asc(term::Column::OrderId)
            .order_by_asc(term::Column::ParentId)
            .all(&state.db)
            .await
        {
            Ok(terms) => terms,
            Err(e) => {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        };

        // Hiyerarşik yapı oluştur - TermExtensions kullanarak doğru title'ları al
        let nested_terms = build_term_hierarchy(&all_terms, "tr", None);

        return (StatusCode::OK, Json(nested_terms)).into_response();
    }

    let all_terms = match Term::find()
        .filter(term::Column::VocabularyId.is_in(vocabulary_ids))
        .filter(term::Column::Publish.eq(true))
        .order_by_asc(term::Column::OrderId)
        .order_by_asc(term::Column::ParentId)
        .all(&state.db)
        .await
    {
        Ok(terms) => terms,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Hiyerarşik yapı oluştur - TermExtensions kullanarak doğru title'ları al
    let nested_terms = build_term_hierarchy(&all_terms, "tr", None);

    (StatusCode::OK, Json(nested_terms)).into_response()
}

// Content-Term ilişkilerini kaydet
async fn save_content_terms(
    db: &DatabaseConnection,
    content_id: i64,
    term_ids: &[i64],
    content_type: &str,
) -> Result<(), DbErr> {
    use crate::modules::content::models::ContentTermActiveModel;

    eprintln!(
        "save_content_terms: content_id={}, term_ids={:?}, content_type={}",
        content_id, term_ids, content_type
    );

    for term_id in term_ids {
        // Entity kullanarak güvenli insert
        let content_term = ContentTermActiveModel {
            content_id: Set(content_id),
            term_id: Set(*term_id),
            content_type: Set(content_type.to_string()),
            created_at: Set(Some(chrono::Utc::now().into())),
        };

        // ON CONFLICT DO NOTHING için insert_many kullanabiliriz
        // veya tek tek insert edip hatayı ignore ederiz
        match content_term.insert(db).await {
            Ok(_) => {
                eprintln!("  ✓ Inserted term_id={}", term_id);
            }
            Err(DbErr::RecordNotInserted) => {
                eprintln!("  ⚠ RecordNotInserted for term_id={}", term_id);
            } // Already exists, ignore
            Err(DbErr::Query(e)) if e.to_string().contains("duplicate key") => {
                // Duplicate key hatası, ignore et
                eprintln!(
                    "  ⚠ Duplicate term relationship ignored: content_id={}, term_id={}",
                    content_id, term_id
                );
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

// Content-Term ilişkilerini sil
// ednpoint değil   , yardımcı fonksiyon
async fn delete_content_terms(
    db: &DatabaseConnection,
    content_id: i64,
    _content_type: &str, // Artık kullanılmıyor ama API uyumluluğu için bırakıldı
) -> Result<(), DbErr> {
    use crate::modules::content::models::{content_terms, ContentTerm};

    // Entity kullanarak güvenli delete - content_id yeterli, content_type'a gerek yok
    ContentTerm::delete_many()
        .filter(content_terms::Column::ContentId.eq(content_id))
        .exec(db)
        .await?;
    Ok(())
}

// Content'e ait term ID'lerini getir
// endpoint değil   , yardımcı fonksiyon
async fn get_content_term_ids(
    db: &DatabaseConnection,
    content_id: i64,
    _content_type: &str, // Artık kullanılmıyor ama API uyumluluğu için bırakıldı
) -> Result<Vec<i64>, DbErr> {
    use crate::modules::content::models::{content_terms, ContentTerm};

    // Entity kullanarak güvenli query - content_id yeterli, content_type'a gerek yok
    let results = ContentTerm::find()
        .filter(content_terms::Column::ContentId.eq(content_id))
        .order_by_asc(content_terms::Column::TermId)
        .all(db)
        .await?;

    let term_ids: Vec<i64> = results.iter().map(|ct| ct.term_id).collect();

    eprintln!(
        "get_content_term_ids: content_id={}, found {} terms: {:?}",
        content_id,
        term_ids.len(),
        term_ids
    );

    Ok(term_ids)
}

// content.data langs, product variants
fn collect_media_ids_from_value(value: &serde_json::Value, ids: &mut Vec<i64>) {
    match value {
        serde_json::Value::Object(map) => {
            // If object has a "media" key, inspect known media arrays
            if let Some(media_val) = map.get("media") {
                if let Some(media_obj) = media_val.as_object() {
                    for key in &["icon", "cover", "video", "gallery", "document"] {
                        if let Some(arr) = media_obj.get(*key).and_then(|v| v.as_array()) {
                            for item in arr {
                                if let Some(id) = item.as_i64() {
                                    ids.push(id);
                                } else if let Some(id) = item.get("id").and_then(|x| x.as_i64()) {
                                    ids.push(id);
                                }
                            }
                        }
                    }
                }
            }

            if let Some(variants_val) = map.get("variants") {
                if let Some(arr) = variants_val.as_array() {
                    for variant in arr {
                        collect_media_ids_from_value(variant, ids);
                    }
                }
            }

            // Recurse into child objects/arrays
            for (_k, v) in map {
                collect_media_ids_from_value(v, ids);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_media_ids_from_value(item, ids);
            }
        }
        _ => {}
    }
}

// Build breadcrumbs for page hierarchy
async fn build_page_breadcrumbs(
    db: &DatabaseConnection,
    page_id: i64,
) -> Result<Vec<BreadcrumbItem>, DbErr> {
    let mut breadcrumbs = Vec::new();
    let mut current_id = Some(page_id);

    while let Some(id) = current_id {
        if let Some(page) = Content::find_by_id(id).one(db).await? {
            // Get title from page data
            let title = if let Ok(page_data) = page.as_content_data() {
                page_data
                    .langs
                    .get("tr")
                    .map(|lang| lang.title.clone())
                    .unwrap_or_else(|| format!("Page {}", id))
            } else {
                format!("Page {}", id)
            };

            // Count children
            let children_count = Content::find()
                .filter(ContentColumn::ParentId.eq(id))
                .filter(ContentColumn::DeletedAt.is_null())
                .count(db)
                .await? as i64;

            breadcrumbs.push(BreadcrumbItem {
                id,
                title,
                url: format!("/admin/contents?parent_id={}", id),
                has_children: children_count > 0,
                children_count,
            });

            current_id = page.parent_id;
        } else {
            break;
        }
    }

    breadcrumbs.reverse();
    Ok(breadcrumbs)
}

// ============ PRODUCT ATTRIBUTES API ============

#[derive(Deserialize)]
pub struct CategoryAttributesRequest {
    pub category_ids: Vec<i64>,
    #[allow(dead_code)]
    pub content_type: String,
}

#[derive(Serialize)]
pub struct AttributeValue {
    pub id: i64,
    pub value: String,
    pub display_value: String,
}

#[derive(Serialize)]
pub struct ProductAttribute {
    pub id: i64,
    pub name: String,
    pub title: String,
    pub description: String,
    pub attribute_type: String,
    pub required: bool,
    pub values: Vec<AttributeValue>,
    pub applicable_categories: Vec<i64>,
}

// API: Kategorilere göre ürün attribute'larını getir
// YENİ: Kategorilere göre ilişkili attribute vocabulary'leri getir
pub async fn get_categories_attributes(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Json(request): Json<CategoryAttributesRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }
    if request.category_ids.is_empty() {
        return Json(serde_json::json!({
            "success": true,
            "attributes": []
        }))
        .into_response();
    }

    use crate::modules::taxonomy::models::{term, Term};

    // İlk önce kategorilere bağlı vocabulary ID'lerini al
    let vocab_ids_query = "
        SELECT DISTINCT vc.vocabulary_id
        FROM vocabulary_categories vc
        JOIN vocabularies v ON vc.vocabulary_id = v.id
        WHERE vc.category_term_id = ANY($1)
        AND v.vocabulary_type = 'product_attributes'
    ";

    let vocab_ids = match sqlx::query_scalar::<_, i64>(vocab_ids_query)
        .bind(&request.category_ids)
        .fetch_all(state.db.get_postgres_connection_pool())
        .await
    {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("Error loading vocabulary IDs: {}", e);
            return Json(serde_json::json!({"error": "Vocabulary ID'leri yüklenemedi"}))
                .into_response();
        }
    };

    if vocab_ids.is_empty() {
        return Json(serde_json::json!({
            "success": true,
            "attributes": [],
            "category_count": request.category_ids.len()
        }))
        .into_response();
    }

    // Şimdi vocabulary'leri SeaORM ile al
    use crate::modules::taxonomy::models::{vocabulary, Vocabulary};

    let attributes_vocab = match Vocabulary::find()
        .filter(vocabulary::Column::Id.is_in(vocab_ids))
        .filter(vocabulary::Column::VocabularyType.eq("product_attributes"))
        .order_by_asc(vocabulary::Column::OrderId)
        .all(&state.db)
        .await
    {
        Ok(vocabs) => vocabs,
        Err(e) => {
            eprintln!("Error loading vocabularies: {}", e);
            return Json(serde_json::json!({"error": "Vocabulary'ler yüklenemedi"}))
                .into_response();
        }
    };

    // Process vocabulary data
    let mut applicable_attributes = Vec::new();

    for vocab in attributes_vocab {
        let attr_data = &vocab.data;

        // Bu attribute'ın değerlerini al
        let values = match Term::find()
            .filter(term::Column::VocabularyId.eq(vocab.id))
            .filter(term::Column::Publish.eq(true))
            .order_by_asc(term::Column::OrderId)
            .all(&state.db)
            .await
        {
            Ok(terms) => terms,
            Err(e) => {
                eprintln!(
                    "Error loading attribute values for vocab {}: {}",
                    vocab.id, e
                );
                continue;
            }
        };

        // Attribute bilgilerini hazırla
        let attr_name = attr_data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let attr_type = attr_data
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        let attr_required = attr_data
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let attr_title = attr_data
            .get("langs")
            .and_then(|langs| langs.get("tr"))
            .and_then(|tr| tr.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or(attr_name);

        let attr_description = attr_data
            .get("langs")
            .and_then(|langs| langs.get("tr"))
            .and_then(|tr| tr.get("description"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Değerleri hazırla
        let attribute_values: Vec<AttributeValue> = values
            .iter()
            .map(|v| {
                let value = v
                    .data
                    .get("value")
                    .and_then(|val| val.as_str())
                    .unwrap_or("");

                // Önce çok dilli formattan title'ı almaya çalış
                let display_value = v
                    .data
                    .get("langs")
                    .and_then(|langs| langs.get("tr"))
                    .and_then(|tr| tr.get("title"))
                    .and_then(|val| val.as_str())
                    .or_else(|| {
                        // Fallback: display_value alanı
                        v.data.get("display_value").and_then(|val| val.as_str())
                    })
                    .unwrap_or(value);

                AttributeValue {
                    id: v.id,
                    value: value.to_string(),
                    display_value: display_value.to_string(),
                }
            })
            .collect();

        applicable_attributes.push(ProductAttribute {
            id: vocab.id,
            name: attr_name.to_string(),
            title: attr_title.to_string(),
            description: attr_description.to_string(),
            attribute_type: attr_type.to_string(),
            required: attr_required,
            values: attribute_values,
            applicable_categories: request.category_ids.clone(),
        });
    }

    // Attribute'ları isme göre sırala
    applicable_attributes.sort_by(|a, b| a.title.cmp(&b.title));

    Json(serde_json::json!({
        "success": true,
        "attributes": applicable_attributes,
        "category_count": request.category_ids.len()
    }))
    .into_response()
}

// ============ VOCABULARY-CATEGORY RELATIONSHIPS API ============

#[derive(Deserialize)]
pub struct VocabularyCategoriesRequest {
    pub category_ids: Vec<i64>,
}

#[derive(Serialize)]
pub struct VocabularyCategoriesResponse {
    pub category_ids: Vec<i64>,
}

// API: Get vocabulary-category relationships
pub async fn get_vocabulary_categories(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(vocabulary_id): Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }
    use crate::modules::taxonomy::models::Vocabulary;
    use sea_orm::EntityTrait;

    // Get vocabulary from database
    let vocabulary = match Vocabulary::find_by_id(vocabulary_id).one(&state.db).await {
        Ok(Some(vocab)) => vocab,
        Ok(None) => {
            return Json(serde_json::json!({"error": "Vocabulary not found"})).into_response();
        }
        Err(e) => {
            eprintln!("Error loading vocabulary: {}", e);
            return Json(serde_json::json!({"error": "Vocabulary yüklenemedi"})).into_response();
        }
    };

    // Extract applicable_categories from JSON data
    let category_ids = vocabulary
        .data
        .get("applicable_categories")
        .and_then(|cats| cats.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect::<Vec<i64>>())
        .unwrap_or_default();

    Json(VocabularyCategoriesResponse { category_ids }).into_response()
}

// API: Update vocabulary-category relationships
pub async fn update_vocabulary_categories(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(vocabulary_id): Path<i64>,
    Json(request): Json<VocabularyCategoriesRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }
    use crate::modules::taxonomy::models::vocabulary::{
        ActiveModel as VocabularyActiveModel, Entity as Vocabulary,
    };
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};

    // Get vocabulary from database
    let vocabulary = match Vocabulary::find_by_id(vocabulary_id).one(&state.db).await {
        Ok(Some(vocab)) => vocab,
        Ok(None) => {
            return Json(serde_json::json!({"error": "Vocabulary not found"})).into_response();
        }
        Err(e) => {
            eprintln!("Error loading vocabulary: {}", e);
            return Json(serde_json::json!({"error": "Vocabulary yüklenemedi"})).into_response();
        }
    };

    // Update JSON data with new applicable_categories
    let mut updated_data = vocabulary.data.clone();
    let category_ids_json: Vec<serde_json::Value> = request
        .category_ids
        .iter()
        .map(|id| serde_json::Value::Number(serde_json::Number::from(*id)))
        .collect();

    updated_data["applicable_categories"] = serde_json::Value::Array(category_ids_json);

    // Update vocabulary in database
    let mut vocabulary_active: VocabularyActiveModel = vocabulary.into();
    vocabulary_active.data = Set(updated_data);
    vocabulary_active.updated_at = Set(Some(chrono::Utc::now().into()));

    match vocabulary_active.update(&state.db).await {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": "Kategori ilişkileri güncellendi"
        }))
        .into_response(),
        Err(e) => {
            eprintln!("Error updating vocabulary: {}", e);
            Json(serde_json::json!({"error": "Vocabulary güncellenemedi"})).into_response()
        }
    }
}

//admin_api_toggle_publish_content

pub async fn admin_api_toggle_publish_content(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Mevcut sayfayı al
    let page = match Content::find_by_id(id)
        .filter(ContentColumn::DeletedAt.is_null())
        .one(&state.db)
        .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Content not found").into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    let mut active_model: ContentActiveModel = page.clone().into();

    // Yayın durumunu tersine çevir
    active_model.publish = Set(!page.publish);
    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    // Güncellenmiş sayfayı kaydet
    let updated_page = match active_model.update(&state.db).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Timeline event oluştur - yayın durumu değişti
    let admin_user_id = auth_user.id;
    let action = if updated_page.publish {
        "published"
    } else {
        "unpublished"
    };

    let metadata = serde_json::json!({
        "content_type": updated_page.content_type,
        "publish_status": updated_page.publish,
        "previous_publish_status": page.publish,
    });

    if let Err(e) = TimelineHelper::create_content_event(
        &state.db,
        updated_page.id,
        &updated_page.content_type,
        action,
        Some(admin_user_id),
        Some(metadata),
    )
    .await
    {
        eprintln!("Timeline event oluşturma hatası: {}", e);
        // Timeline hatası ana işlemi etkilemesin
    }

    // Global context cache ve homepage cache yenile
    if let Err(e) =
        crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
            &state.db,
            &state.global_context_cache,
        )
        .await
    {
        eprintln!("Global context cache yenileme hatası: {}", e);
    }

    crate::modules::admin::controllers::web::homepage::clear_homepage_cache(&state);

    (
        StatusCode::OK,
        Json(updated_page.get_admin_content_response()),
    )
        .into_response()
}

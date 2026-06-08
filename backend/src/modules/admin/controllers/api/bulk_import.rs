// src/modules/content/controllers/api/bulk_import.rs
// Toplu ürün import endpoint'i

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, QueryFilter, QueryOrder, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::AppState;

// Request yapısı - Frontend'den gelen veri
#[derive(Debug, Deserialize)]
pub struct BulkImportRequest {
    pub products: Vec<ProductImportData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ProductImportData {
    pub content_type: String,
    pub status: String,
    pub langs: std::collections::HashMap<String, LangData>,
    pub product: ProductDetails,
    pub template: String,
    pub settings: serde_json::Value,
    #[serde(default)]
    pub page_aliases: Vec<serde_json::Value>,
    #[serde(default)]
    pub sub_contents: Vec<serde_json::Value>,
    #[serde(default)]
    pub form_settings: FormSettings,
    #[serde(default)]
    pub term_master_id: Option<i64>,
    #[serde(default)]
    pub term_ids: Vec<i64>,
    // Geriye dönük uyumluluk için sabit alanlar (TR ve EN)
    #[serde(default)]
    pub category_tr: Option<String>,
    #[serde(default)]
    pub category_en: Option<String>,
    #[serde(default)]
    pub categories_tr: Vec<String>,
    #[serde(default)]
    pub categories_en: Vec<String>,
    // Dinamik kategori alanları - category_{lang} ve categories_{lang}
    #[serde(flatten)]
    pub extra_categories: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct LangData {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub meta_title: String,
    #[serde(default)]
    pub meta_description: String,
    #[serde(default)]
    pub media: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProductDetails {
    pub sku: String,
    pub price: f64,
    #[serde(default)]
    pub stock: Option<i32>,
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub on_sale: bool,
    #[serde(default)]
    pub options: Vec<ProductOption>,
    #[serde(default)]
    pub variants: Vec<ProductVariant>,
    pub currency: String,
    #[serde(default)]
    pub vat_rate: Option<f64>,
    #[serde(default)]
    pub b2b_price: Option<f64>,
    #[serde(default)]
    pub old_price: Option<f64>,
    #[serde(default)]
    pub discount_percentage: Option<f64>,
    #[serde(default)]
    pub delivery_duration: Option<i32>,
    #[serde(default)]
    pub attributes: serde_json::Value,
    #[serde(default)]
    pub dimensions: Dimensions,
    #[serde(default)]
    pub dimensional_weight: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Dimensions {
    #[serde(default)]
    pub depth: Option<f64>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProductOption {
    pub name: String,
    pub values: String, // Array yerine String (örn: "Kırmızı, Yeşil, Mavi")
    pub position: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProductVariant {
    pub sku: String,
    pub price: f64,
    #[serde(default)]
    pub stock: Option<i32>,
    #[serde(default)]
    pub b2b_price: Option<f64>,
    #[serde(default)]
    pub old_price: Option<f64>,
    #[serde(default)]
    pub discount_percentage: Option<f64>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub option_values: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub option_values_display: String,
    #[serde(default)]
    pub compare_at_price: Option<f64>,
    #[serde(default)]
    pub media: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[allow(dead_code)]
pub struct FormSettings {
    #[serde(default)]
    pub fields: Vec<serde_json::Value>,
    #[serde(default)]
    pub send_email: bool,
    #[serde(default)]
    pub allow_anonymous: bool,
}

// Response yapısı
#[derive(Debug, Serialize)]
pub struct BulkImportResponse {
    pub success: bool,
    pub success_count: usize,
    pub error_count: usize,
    pub errors: Vec<String>,
    pub imported_ids: Vec<i64>,
}

/// POST /api/admin/products/bulk-import
/// Toplu ürün import endpoint'i - BATCH INSERT ile hızlandırılmış
pub async fn bulk_import_products(
    State(state): State<AppState>,
    Json(request): Json<BulkImportRequest>,
) -> impl IntoResponse {
    info!(
        "Bulk import started with BATCH INSERT. Products count: {}",
        request.products.len()
    );

    // Batch insert kullan - çok daha hızlı
    match batch_insert_products(&state, &request.products).await {
        Ok(result) => {
            info!(
                "Bulk import completed with BATCH INSERT. Success: {}, Errors: {}",
                result.success_count, result.error_count
            );
            (StatusCode::OK, Json(result))
        }
        Err(e) => {
            error!("Batch import failed: {}", e);
            let response = BulkImportResponse {
                success: false,
                success_count: 0,
                error_count: request.products.len(),
                errors: vec![format!("Batch import failed: {}", e)],
                imported_ids: vec![],
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
        }
    }
}

// Content-Term ilişkilerini sil (güncelleme için)
async fn delete_content_terms<C: sea_orm::ConnectionTrait>(
    db: &C,
    content_id: i64,
) -> Result<(), sea_orm::DbErr> {
    use crate::modules::content::models::content_terms::{
        Column as ContentTermColumn, Entity as ContentTermEntity,
    };
    use sea_orm::{EntityTrait, QueryFilter};

    info!("Deleting content terms for content_id: {}", content_id);

    ContentTermEntity::delete_many()
        .filter(ContentTermColumn::ContentId.eq(content_id))
        .exec(db)
        .await?;

    Ok(())
}

/// Kategori ağacını path string'den çöz veya oluştur (cache ile)
/// "Bilgisayar.Amd.Cpu" -> [Bilgisayar_id, Amd_id, Cpu_id] + Cpu_id (master)
/// Term data yapısı: { "langs": { "tr": { "title": "...", "slug": "..." } } }
///
/// lang parametresi "tr" veya "en" olabilir.
/// "tr" ile çağrılırsa TR title ile arar/oluşturur.
/// "en" ile çağrılırsa EN title ile arar, yoksa aynı parent'daki ilk term'ü bulup EN data'sını günceller.
async fn resolve_category_path_with_cache<C: sea_orm::ConnectionTrait>(
    db: &C,
    path: &str,
    vocabulary_id: i64,
    lang: &str,
    term_cache: &mut std::collections::HashMap<(Option<i64>, String), i64>,
) -> Result<(Option<i64>, Vec<i64>), String> {
    use crate::modules::taxonomy::helpers::term_helper::TermExtensions;
    use crate::modules::taxonomy::models::term::{Term, TermActiveModel};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
    use slug::slugify;

    let parts: Vec<&str> = path
        .split('.')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Ok((None, vec![]));
    }

    let mut term_ids: Vec<i64> = Vec::new();
    let mut current_parent_id: Option<i64> = None;

    for part in parts.iter() {
        let part_lower = part.to_lowercase();
        let cache_key = (current_parent_id, part_lower.clone());

        info!(
            "Processing category part: '{}', lang: '{}', parent_id: {:?}, cache_key: {:?}",
            part, lang, current_parent_id, cache_key
        );

        let term_id = if let Some(&id) = term_cache.get(&cache_key) {
            info!("Found in cache: '{}' -> id={}", part, id);
            id
        } else {
            // Cache'de yok, DB'den ara
            let existing = Term::find()
                .filter(
                    crate::modules::taxonomy::models::term::Column::VocabularyId.eq(vocabulary_id),
                )
                .filter(
                    crate::modules::taxonomy::models::term::Column::ParentId.eq(current_parent_id),
                )
                .all(db)
                .await
                .map_err(|e| format!("Failed to find term '{}': {}", part, e))?;

            // Önce bu dildeki title ile ara
            let found = existing
                .iter()
                .find(|t| t.get_title(lang).trim().to_lowercase() == part_lower);

            if let Some(found_term) = found {
                info!(
                    "Found in DB by {} title: '{}' -> id={}",
                    lang, part, found_term.id
                );
                term_cache.insert(cache_key, found_term.id);
                found_term.id
            } else if lang == "en" && !existing.is_empty() {
                // EN path için: TR'den oluşturulmuş term'ü bul ve EN data'sını güncelle
                // Aynı parent'daki ilk term'ü al (TR path ile oluşturulmuş olmalı)
                let first_term = &existing[0];
                info!(
                    "EN path: Found existing term (TR) id={}, updating with EN title '{}'",
                    first_term.id, part
                );

                // Term'ün mevcut datasını al ve EN title'ını güncelle
                let mut data = first_term.data.clone();
                let slug_en = slugify(part);

                if let Some(langs) = data.get_mut("langs") {
                    if let Some(en_obj) = langs.get_mut("en") {
                        en_obj["title"] = serde_json::json!(part);
                        en_obj["slug"] = serde_json::json!(slug_en);
                        en_obj["description"] = serde_json::json!(part);
                    } else {
                        langs["en"] = serde_json::json!({
                            "title": part,
                            "slug": slug_en,
                            "description": part,
                            "menu_url": "",
                            "media": {
                                "icon": [],
                                "cover": [],
                                "video": [],
                                "gallery": [],
                                "document": []
                            }
                        });
                    }
                } else {
                    data["langs"] = serde_json::json!({
                        "en": {
                            "title": part,
                            "slug": slug_en,
                            "description": part,
                            "menu_url": "",
                            "media": {
                                "icon": [],
                                "cover": [],
                                "video": [],
                                "gallery": [],
                                "document": []
                            }
                        }
                    });
                }

                let mut active_model: TermActiveModel = first_term.clone().into();
                active_model.data = Set(data);
                active_model.updated_at = Set(Some(chrono::Utc::now().into()));

                let updated = active_model
                    .update(db)
                    .await
                    .map_err(|e| format!("Failed to update term EN data: {}", e))?;

                info!(
                    "Updated term EN data: id={}, EN title='{}'",
                    updated.id, part
                );
                term_cache.insert(cache_key, updated.id);
                updated.id
            } else {
                // Yeni term oluştur - sadece bu dilde
                let slug = slugify(part);
                let data = serde_json::json!({
                    "langs": {
                        lang: {
                            "title": part,
                            "slug": slug,
                            "description": part,
                            "menu_url": "",
                            "media": {
                                "icon": [],
                                "cover": [],
                                "video": [],
                                "gallery": [],
                                "document": []
                            }
                        }
                    },
                    "term_icon": ""
                });

                let max_order = Term::find()
                    .filter(
                        crate::modules::taxonomy::models::term::Column::VocabularyId
                            .eq(vocabulary_id),
                    )
                    .filter(
                        crate::modules::taxonomy::models::term::Column::ParentId
                            .eq(current_parent_id),
                    )
                    .order_by_desc(crate::modules::taxonomy::models::term::Column::OrderId)
                    .one(db)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|t| t.order_id)
                    .unwrap_or(0);

                let now: chrono::DateTime<chrono::Utc> = chrono::Utc::now();

                info!(
                    "Creating new term: '{}', lang: '{}', parent_id: {:?}, vocabulary_id: {}",
                    part, lang, current_parent_id, vocabulary_id
                );

                let new_term = TermActiveModel {
                    vocabulary_id: Set(vocabulary_id),
                    data: Set(data),
                    parent_id: Set(current_parent_id),
                    order_id: Set(Some(max_order + 1)),
                    publish: Set(true),
                    lock: Set(false),
                    hide: Set(false),
                    created_at: Set(Some(now.into())),
                    updated_at: Set(Some(now.into())),
                    ..Default::default()
                };

                let inserted = new_term
                    .insert(db)
                    .await
                    .map_err(|e| format!("Failed to create term '{}': {}", part, e))?;

                info!(
                    "Created term: '{}' -> id={}, parent_id={:?}",
                    part, inserted.id, inserted.parent_id
                );

                // Cache'e ekle
                term_cache.insert(cache_key, inserted.id);
                inserted.id
            }
        };

        term_ids.push(term_id);
        current_parent_id = Some(term_id);
        info!("Updated current_parent_id to: {:?}", current_parent_id);
    }

    // Son term ana kategori (master kategori)
    let master_id = term_ids.last().copied();
    Ok((master_id, term_ids))
}

/// GET /api/admin/products/bulk-import/test
/// Endpoint test endpoint'i
pub async fn test_bulk_import_endpoint() -> impl IntoResponse {
    info!("Bulk import test endpoint called");

    let response = serde_json::json!({
        "status": "ok",
        "message": "Bulk import endpoint is ready",
        "endpoint": "POST /api/admin/products/bulk-import",
        "expected_format": {
            "products": [
                {
                    "content_type": "product",
                    "status": "published",
                    "langs": {
                        "tr": {
                            "title": "Ürün Adı",
                            "slug": "urun-adi",
                            "description": "Ürün açıklaması",
                            "body": "Ürün içeriği",
                            "meta_title": "Meta Başlık",
                            "meta_description": "Meta Açıklama",
                            "media": {
                                "icon": [],
                                "cover": [],
                                "video": [],
                                "gallery": [],
                                "document": []
                            }
                        }
                    },
                    "product": {
                        "sku": "SKU-001",
                        "price": 100.00,
                        "stock": 10,
                        "weight": 1.5,
                        "barcode": "123456789",
                        "on_sale": false,
                        "options": [
                            {"name": "Renk", "values": "Kırmızı, Mavi", "position": 0}
                        ],
                        "variants": [
                            {
                                "sku": "SKU-001-RED",
                                "price": 100.00,
                                "stock": 5,
                                "option_values": {"Renk": "Kırmızı"},
                                "option_values_display": "Kırmızı",
                                "is_active": true
                            }
                        ],
                        "currency": "TRY",
                        "vat_rate": 18.0,
                        "b2b_price": 80.00,
                        "old_price": 120.00,
                        "delivery_duration": 3,
                        "attributes": {},
                        "dimensions": {"depth": null, "width": null, "height": null},
                        "dimensional_weight": null
                    },
                    "template": "product_detail.html",
                    "settings": {"kapak_yazi_goster": true, "kapak_resmi_goster": true},
                    "page_aliases": [],
                    "sub_contents": [],
                    "form_settings": {
                        "fields": [],
                        "send_email": false,
                        "allow_anonymous": true
                    },
                    "term_master_id": 5,
                    "term_ids": [15, 16, 17]
                }
            ]
        }
    });

    (StatusCode::OK, Json(response))
}

/// Batch insert products using a database transaction for better performance
async fn batch_insert_products(
    state: &AppState,
    products: &[ProductImportData],
) -> Result<BulkImportResponse, String> {
    use crate::modules::content::models::content::Column as ContentColumn;
    use crate::modules::content::models::{
        Content, ContentActiveModel, ContentModel, ContentTermActiveModel,
    };
    use sea_orm::EntityTrait;
    use serde_json::json;
    use slug::slugify;
    use std::collections::HashMap;

    info!("Starting batch insert for {} products", products.len());

    // Get vocabulary_id for product categories from settings
    let vocabulary_id = crate::modules::admin::services::settings_service::get_vocab_id(
        &state.db,
        "product_categories",
    )
    .await
    .unwrap_or(1);

    info!(
        "Using vocabulary_id: {} for product categories",
        vocabulary_id
    );

    // Pre-load all existing terms for this vocabulary
    use crate::modules::taxonomy::helpers::term_helper::TermExtensions;
    use crate::modules::taxonomy::models::term::Term;

    let all_terms = Term::find()
        .filter(crate::modules::taxonomy::models::term::Column::VocabularyId.eq(vocabulary_id))
        .all(&state.db)
        .await
        .map_err(|e| format!("Failed to load terms: {}", e))?;

    info!(
        "Loaded {} existing terms from vocabulary {}",
        all_terms.len(),
        vocabulary_id
    );

    // Build term cache: (parent_id, title_lower) -> term_id
    let mut term_cache: std::collections::HashMap<(Option<i64>, String), i64> =
        std::collections::HashMap::new();
    for term in &all_terms {
        let title_lower = term.get_title("tr").trim().to_lowercase();
        term_cache.insert((term.parent_id, title_lower), term.id);
    }

    // 1. Collect all SKUs and find existing products
    let skus: Vec<String> = products
        .iter()
        .map(|p| p.product.sku.trim().to_lowercase())
        .collect();

    // Fetch all existing products of type 'product' that are not deleted
    // We'll filter by SKU in Rust code since SKU is in JSON data
    let existing_products: Vec<ContentModel> = Content::find()
        .filter(ContentColumn::ContentType.eq("product"))
        .filter(ContentColumn::DeletedAt.is_null())
        .all(&state.db)
        .await
        .map_err(|e| format!("Failed to fetch existing products: {}", e))?;

    // Build a HashMap of existing products by normalized SKU
    let existing_products_map: HashMap<String, ContentModel> = existing_products
        .into_iter()
        .filter_map(|p| {
            let sku_str = p
                .data
                .get("product")
                .and_then(|prod| prod.get("sku"))
                .and_then(|sku| sku.as_str())
                .map(|s| s.to_string());
            sku_str.map(|sku| {
                let normalized_sku = sku.trim().to_lowercase();
                (normalized_sku, p)
            })
        })
        .filter(|(normalized_sku, _)| skus.contains(normalized_sku))
        .collect();

    info!("Found {} existing products", existing_products_map.len());

    // 2. Get max order_id for new products
    let max_order_result: Option<crate::modules::content::models::ContentModel> = Content::find()
        .filter(ContentColumn::ContentType.eq("product"))
        .filter(ContentColumn::DeletedAt.is_null())
        .order_by_desc(ContentColumn::OrderId)
        .one(&state.db)
        .await
        .ok()
        .flatten();

    let max_order_id: i32 = max_order_result.and_then(|c| c.order_id).unwrap_or(0);

    // 3. Start database transaction
    let txn: sea_orm::DatabaseTransaction = state
        .db
        .begin()
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let mut success_count = 0;
    let error_count = 0;
    let errors: Vec<String> = vec![];
    let mut imported_ids: Vec<i64> = vec![];
    let mut next_order_id = max_order_id;

    // 4. Process each product in the transaction
    for product in products {
        let normalized_sku = product.product.sku.trim().to_lowercase();

        // Prepare normalized variants
        let normalized_variants: Vec<_> = product
            .product
            .variants
            .iter()
            .map(|v| {
                json!({
                    "sku": v.sku.trim().to_lowercase(),
                    "price": v.price,
                    "stock": v.stock,
                    "b2b_price": v.b2b_price,
                    "old_price": v.old_price,
                    "discount_percentage": v.discount_percentage,
                    "is_active": v.is_active,
                    "option_values": v.option_values,
                    "option_values_display": v.option_values_display,
                    "compare_at_price": v.compare_at_price,
                    "media": v.media
                })
            })
            .collect();

        // 5. Resolve category paths FIRST to get master_id
        let mut all_term_ids: Vec<i64> = product.term_ids.clone();
        let default_lang = "tr";
        let mut resolved_master_id: Option<i64> = product.term_master_id;

        // Tüm category_{lang} alanlarını topla (sabit + dinamik)
        let mut category_paths_by_lang: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        // Sabit alanlar (geriye dönük uyumluluk)
        if let Some(ref tr) = product.category_tr {
            if !tr.is_empty() {
                category_paths_by_lang.insert("tr".to_string(), tr.clone());
            }
        }
        if let Some(ref en) = product.category_en {
            if !en.is_empty() {
                category_paths_by_lang.insert("en".to_string(), en.clone());
            }
        }

        // Dinamik alanlar - category_{lang} formatında
        if let Some(obj) = product.extra_categories.as_object() {
            for (key, value) in obj {
                if key.starts_with("category_") && !key.starts_with("categories_") {
                    let lang = key.strip_prefix("category_").unwrap_or(key);
                    if let Some(path) = value.as_str() {
                        if !path.is_empty() {
                            category_paths_by_lang
                                .entry(lang.to_string())
                                .or_insert(path.to_string());
                        }
                    }
                }
            }
        }

        // Tüm categories_{lang} alanlarını topla (sabit + dinamik)
        let mut extra_paths_by_lang: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        // Sabit alanlar
        if !product.categories_tr.is_empty() {
            extra_paths_by_lang.insert("tr".to_string(), product.categories_tr.clone());
        }
        if !product.categories_en.is_empty() {
            extra_paths_by_lang.insert("en".to_string(), product.categories_en.clone());
        }

        // Dinamik alanlar - categories_{lang} formatında
        if let Some(obj) = product.extra_categories.as_object() {
            for (key, value) in obj {
                if key.starts_with("categories_") {
                    let lang = key.strip_prefix("categories_").unwrap_or(key);
                    if let Some(paths) = value.as_array() {
                        let path_strings: Vec<String> = paths
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .filter(|s| !s.is_empty())
                            .collect();
                        if !path_strings.is_empty() {
                            extra_paths_by_lang
                                .entry(lang.to_string())
                                .or_insert(path_strings);
                        }
                    }
                }
            }
        }

        // Ana kategori: Reference dil (TR) ile kategori hiyerarşisini oluştur
        if let Some(ref_path) = category_paths_by_lang.get(default_lang) {
            match resolve_category_path_with_cache(
                &txn,
                ref_path,
                vocabulary_id,
                default_lang,
                &mut term_cache,
            )
            .await
            {
                Ok((master_id, term_ids)) => {
                    // master_id'yi kaydet
                    if master_id.is_some() {
                        resolved_master_id = master_id;
                    }
                    
                    for &tid in &term_ids {
                        if !all_term_ids.contains(&tid) {
                            all_term_ids.push(tid);
                        }
                    }

                    // Diğer dillerin path segmentlerini çeviri olarak ekle
                    for (lang, path) in &category_paths_by_lang {
                        if lang != default_lang {
                            let parts: Vec<&str> = path
                                .split('.')
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .collect();
                            for (idx, &tid) in term_ids.iter().enumerate() {
                                if idx < parts.len() {
                                    let title = parts[idx];
                                    if let Ok(Some(term_model)) =
                                        crate::modules::taxonomy::models::term::Term::find_by_id(
                                            tid,
                                        )
                                        .one(&txn)
                                        .await
                                    {
                                        let mut data = term_model.data.clone();
                                        let slug_lang = slugify(title);

                                        if let Some(langs) = data.get_mut("langs") {
                                            langs[lang] = serde_json::json!({
                                                "title": title,
                                                "slug": slug_lang,
                                                "description": title,
                                                "menu_url": "",
                                                "media": { "icon": [], "cover": [], "video": [], "gallery": [], "document": [] }
                                            });
                                        }

                                        let mut active_model: crate::modules::taxonomy::models::term::TermActiveModel = term_model.into();
                                        active_model.data = Set(data);
                                        active_model.updated_at =
                                            Set(Some(chrono::Utc::now().into()));
                                        let _ = active_model.update(&txn).await;
                                    }
                                }
                            }
                        }
                    }

                    info!("Resolved main category path '{}' -> master_id: {:?}, {} terms", ref_path, resolved_master_id, term_ids.len());
                }
                Err(e) => error!("Failed to resolve main category path '{}': {}", ref_path, e),
            }
        }

        // Ek kategoriler: Reference dil (TR) ile oluştur, diğer dilleri çeviri olarak ekle
        if let Some(ref_paths) = extra_paths_by_lang.get(default_lang) {
            for (cat_idx, cat_path) in ref_paths.iter().enumerate() {
                if !cat_path.is_empty() {
                    match resolve_category_path_with_cache(
                        &txn,
                        cat_path,
                        vocabulary_id,
                        default_lang,
                        &mut term_cache,
                    )
                    .await
                    {
                        Ok((_, term_ids)) => {
                            for &tid in &term_ids {
                                if !all_term_ids.contains(&tid) {
                                    all_term_ids.push(tid);
                                }
                            }

                            // Diğer dillerin aynı index'teki path'lerini çeviri olarak ekle
                            for (lang, paths) in &extra_paths_by_lang {
                                if lang != default_lang && cat_idx < paths.len() {
                                    let other_path = &paths[cat_idx];
                                    let parts: Vec<&str> = other_path
                                        .split('.')
                                        .map(|s| s.trim())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    for (idx, &tid) in term_ids.iter().enumerate() {
                                        if idx < parts.len() {
                                            let title = parts[idx];
                                            if let Ok(Some(term_model)) = crate::modules::taxonomy::models::term::Term::find_by_id(tid).one(&txn).await {
                                                let mut data = term_model.data.clone();
                                                let slug_lang = slugify(title);

                                                if let Some(langs) = data.get_mut("langs") {
                                                    langs[lang] = serde_json::json!({
                                                        "title": title,
                                                        "slug": slug_lang,
                                                        "description": title,
                                                        "menu_url": "",
                                                        "media": { "icon": [], "cover": [], "video": [], "gallery": [], "document": [] }
                                                    });
                                                }

                                                let mut active_model: crate::modules::taxonomy::models::term::TermActiveModel = term_model.into();
                                                active_model.data = Set(data);
                                                active_model.updated_at = Set(Some(chrono::Utc::now().into()));
                                                let _ = active_model.update(&txn).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => error!("Failed to resolve extra category '{}': {}", cat_path, e),
                    }
                }
            }
        }

        // 6. Prepare data JSON with resolved master_id
        let data = json!({
            "langs": product.langs,
            "product": {
                "sku": normalized_sku,
                "price": product.product.price,
                "stock": product.product.stock,
                "weight": product.product.weight,
                "barcode": product.product.barcode,
                "on_sale": product.product.on_sale,
                "options": product.product.options,
                "variants": normalized_variants,
                "currency": product.product.currency,
                "vat_rate": product.product.vat_rate,
                "b2b_price": product.product.b2b_price,
                "old_price": product.product.old_price,
                "discount_percentage": product.product.discount_percentage,
                "delivery_duration": product.product.delivery_duration,
                "attributes": product.product.attributes,
                "dimensions": product.product.dimensions,
                "dimensional_weight": product.product.dimensional_weight
            },
            "template": product.template,
            "settings": product.settings,
            "page_aliases": product.page_aliases,
            "sub_contents": product.sub_contents,
            "form_settings": product.form_settings,
            "term_master_id": resolved_master_id
        });

        let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();

        // Insert or update product
        let content_id = match existing_products_map.get(&normalized_sku) {
            Some(existing) => {
                // Update existing product
                info!(
                    "Updating existing product ID: {} with SKU: {}, master_id: {:?}",
                    existing.id, product.product.sku, resolved_master_id
                );

                // Deep merge: Preserve existing media, translations, and configurations from the database
                let mut data_to_save = data.clone();

                // 1. Preserve langs (translations and their media)
                if let Some(existing_langs) = existing.data.get("langs").and_then(|l| l.as_object()) {
                    if let Some(new_langs) = data_to_save.get_mut("langs").and_then(|l| l.as_object_mut()) {
                        for (lang_code, existing_lang_val) in existing_langs {
                            if let Some(new_lang_val) = new_langs.get_mut(lang_code) {
                                if let Some(existing_media) = existing_lang_val.get("media") {
                                    if !existing_media.is_null() {
                                        if let Some(new_lang_obj) = new_lang_val.as_object_mut() {
                                            new_lang_obj.insert("media".to_string(), existing_media.clone());
                                        }
                                    }
                                }
                            } else {
                                // Keep language translation completely if missing in the new import
                                new_langs.insert(lang_code.clone(), existing_lang_val.clone());
                            }
                        }
                    }
                }

                // 2. Preserve page_aliases if empty in new import but exists in DB
                if let Some(existing_aliases) = existing.data.get("page_aliases").and_then(|a| a.as_array()) {
                    if !existing_aliases.is_empty() {
                        if let Some(new_aliases) = data_to_save.get("page_aliases").and_then(|a| a.as_array()) {
                            if new_aliases.is_empty() {
                                data_to_save["page_aliases"] = serde_json::Value::Array(existing_aliases.clone());
                            }
                        }
                    }
                }

                // 3. Preserve sub_contents if empty in new import but exists in DB
                if let Some(existing_sub) = existing.data.get("sub_contents").and_then(|s| s.as_array()) {
                    if !existing_sub.is_empty() {
                        if let Some(new_sub) = data_to_save.get("sub_contents").and_then(|s| s.as_array()) {
                            if new_sub.is_empty() {
                                data_to_save["sub_contents"] = serde_json::Value::Array(existing_sub.clone());
                            }
                        }
                    }
                }

                // 4. Preserve settings if empty/null in new import but exists in DB
                if let Some(existing_settings) = existing.data.get("settings") {
                    if !existing_settings.is_null() && existing_settings.is_object() {
                        if let Some(new_settings) = data_to_save.get("settings") {
                            if new_settings.is_null() || (new_settings.is_object() && new_settings.as_object().unwrap().is_empty()) {
                                data_to_save["settings"] = existing_settings.clone();
                            }
                        }
                    }
                }

                // 5. Preserve form_settings if empty/null in new import but exists in DB
                if let Some(existing_form) = existing.data.get("form_settings") {
                    if !existing_form.is_null() && existing_form.is_object() {
                        if let Some(new_form) = data_to_save.get("form_settings") {
                            if new_form.is_null() || (new_form.is_object() && new_form.as_object().unwrap().is_empty()) {
                                data_to_save["form_settings"] = existing_form.clone();
                            }
                        }
                    }
                }

                let mut active_model: ContentActiveModel = existing.clone().into();
                active_model.data = Set(data_to_save);
                active_model.content_type = Set(product.content_type.clone());
                active_model.publish = Set(product.status == "published");
                active_model.updated_at = Set(Some(now));

                let updated: crate::modules::content::models::ContentModel = active_model
                    .update(&txn)
                    .await
                    .map_err(|e| format!("Update error for SKU {}: {}", product.product.sku, e))?;

                // Delete existing content terms for update
                delete_content_terms(&txn, updated.id).await.map_err(|e| {
                    format!(
                        "Failed to delete old content terms for {}: {}",
                        product.product.sku, e
                    )
                })?;

                updated.id
            }
            None => {
                // Insert new product
                info!("Inserting new product with SKU: {}, master_id: {:?}", product.product.sku, resolved_master_id);

                next_order_id += 1;

                let active_model = ContentActiveModel {
                    data: Set(data),
                    content_type: Set(product.content_type.clone()),
                    publish: Set(product.status == "published"),
                    gcx: Set(false),
                    parent_id: Set(None),
                    order_id: Set(Some(next_order_id)),
                    created_at: Set(Some(now)),
                    updated_at: Set(Some(now)),
                    deleted_at: Set(None),
                    ..Default::default()
                };

                let inserted = active_model
                    .insert(&txn)
                    .await
                    .map_err(|e| format!("Insert error for SKU {}: {}", product.product.sku, e))?;

                inserted.id
            }
        };

        // 7. Insert content terms
        if !all_term_ids.is_empty() {
            for term_id in &all_term_ids {
                let content_term = ContentTermActiveModel {
                    content_id: Set(content_id),
                    term_id: Set(*term_id),
                    content_type: Set(product.content_type.clone()),
                    created_at: Set(Some(chrono::Utc::now().into())),
                };

                if let Err(e) = content_term.insert(&txn).await {
                    let err_str = e.to_string();
                    if !err_str.contains("duplicate key") {
                        error!(
                            "Failed to insert content_term for content_id={}, term_id={}: {}",
                            content_id, term_id, e
                        );
                    }
                }
            }
        }

        imported_ids.push(content_id);
        success_count += 1;
    }

    // 7. Commit the transaction at the end
    txn.commit()
        .await
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    info!(
        "Batch insert completed. Success: {}, Errors: {}",
        success_count, error_count
    );

    Ok(BulkImportResponse {
        success: error_count == 0,
        success_count,
        error_count,
        errors,
        imported_ids,
    })
}

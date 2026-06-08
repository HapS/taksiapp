// API Controllers - JSON endpoints
use crate::config;
use crate::middleware::global_context::GlobalContext;
use crate::modules::content::helpers::ProductResponse;
use crate::{app_state::AppState, modules::content::services::product_service};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    Extension,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub results: Option<T>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub lang: Option<String>,
}

/// API: List all published pages
pub async fn product_list(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    Query(query): Query<PageQuery>,
) -> Response {
    let config = config::get_config();
    let language = config.get_language_or_default(query.lang.as_deref());
    let display_currency = Some(global_ctx.display_currency.as_str());

    match product_service::list_products(&state.db, &language, None, display_currency).await {
        Ok(products) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                error: None,
                results: Some(products),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<ProductResponse>> {
                success: false,
                error: Some(format!("Failed to fetch pages: {:?}", e)),
                results: None,
            }),
        )
            .into_response(),
    }
}

/// API: Get page by ID
pub async fn get_by_id(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    Path(id): Path<i64>,
    Query(query): Query<PageQuery>,
) -> Response {
    let config = config::get_config();
    let language = config.get_language_or_default(query.lang.as_deref());
    let display_currency = Some(global_ctx.display_currency.as_str());

    match product_service::get_product(&state.db, &language, None, Some(id), display_currency).await
    {
        Ok(product) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                results: Some(product),
                error: None,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<ProductResponse> {
                success: false,
                results: None,
                error: Some("Page not found".to_string()),
            }),
        )
            .into_response(),
    }
}

pub async fn get_categories(
    State(state): State<AppState>,
    Query(query): Query<PageQuery>,
) -> Response {
    let config = config::get_config();
    let language = config.get_language_or_default(query.lang.as_deref());

    // 3 id product category vocabulary id
    match product_service::get_producs_all_categories(&state.db, &language, Some(1)).await {
        Ok(categories) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                error: None,
                results: Some(categories),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<serde_json::Value>> {
                success: false,
                error: Some(format!("Failed to fetch categories: {:?}", e)),
                results: None,
            }),
        )
            .into_response(),
    }
}

/// API: Get attributes (vocabularies) for a specific category
pub async fn get_category_attributes(
    State(state): State<AppState>,
    Path(category_id): Path<i64>,
    Query(query): Query<PageQuery>,
) -> Response {
    use crate::modules::taxonomy::helpers::vocabulary_helper::VocabularyExtensions;
    use crate::modules::taxonomy::models::Vocabulary;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let config = config::get_config();
    let language = config.get_language_or_default(query.lang.as_deref());

    // Get all product_attributes vocabularies
    let all_vocabularies = match Vocabulary::find()
        .filter(
            crate::modules::taxonomy::models::vocabulary::Column::VocabularyType
                .eq("product_attributes"),
        )
        .all(&state.db)
        .await
    {
        Ok(vocabs) => vocabs,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Vec<serde_json::Value>> {
                    success: false,
                    error: Some("Veritabanı hatası".to_string()),
                    results: None,
                }),
            )
                .into_response();
        }
    };

    // Filter vocabularies by applicable_categories
    let vocabularies: Vec<_> = all_vocabularies
        .into_iter()
        .filter(|vocab| {
            let vocab_name = vocab
                .data
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("unknown");

            if let Some(applicable_categories) = vocab.data.get("applicable_categories") {
                if let Some(categories_array) = applicable_categories.as_array() {
                    let has_category = categories_array
                        .iter()
                        .any(|cat| cat.as_i64() == Some(category_id));

                    println!(
                        "Vocabulary '{}': applicable_categories={:?}, category_id={}, matches={}",
                        vocab_name, categories_array, category_id, has_category
                    );

                    return has_category;
                }
            }

            println!(
                "Vocabulary '{}': no applicable_categories found",
                vocab_name
            );
            false
        })
        .collect();

    // Convert to response format with terms
    let mut response_vocabularies = Vec::new();

    for vocabulary in vocabularies {
        // Get terms for this vocabulary
        let terms = match crate::modules::taxonomy::services::term_service::get_terms_by_vocabulary(
            &state.db,
            vocabulary.id,
            &language,
        )
        .await
        {
            Ok(terms) => terms,
            Err(_) => continue,
        };

        // Extract name from vocabulary data
        let vocab_name = vocabulary
            .data
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let vocab_response = serde_json::json!({
            "id": vocabulary.id,
            "name": vocab_name,
            "title": vocabulary.get_title(&language),
            "description": vocabulary.get_description(&language),
            "data": vocabulary.data,
            "terms": terms
        });

        response_vocabularies.push(vocab_response);
    }

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            error: None,
            results: Some(response_vocabularies),
        }),
    )
        .into_response()
}

// Debug API: Create a product without admin auth (development only)
// POST /api/debug/products - accepts CreateContentRequest-style payload (similar to admin API)

//dead code
#[allow(dead_code)]
pub async fn debug_create_product(
    State(state): State<AppState>,
    axum::Json(json): axum::Json<crate::modules::admin::controllers::dto::CreateContentRequest>,
) -> Response {
    // Sadece debug modda aktif olsun
    if !crate::config::get_config().is_debug() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<serde_json::Value> {
                success: false,
                results: None,
                error: Some("debug_api_disabled".to_string()),
            }),
        )
            .into_response();
    }

    // Yalnızca ürün (product) tipi için izin ver (veya default olarak product kabul et)
    if let Some(ct) = json.content_type.as_deref() {
        if ct != "product" {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<serde_json::Value> {
                    success: false,
                    results: None,
                    error: Some("only_product_allowed".to_string()),
                }),
            )
                .into_response();
        }
    }

    let content_type = "product".to_string();

    // Max order_id al
    use crate::modules::content::models::content::Column as ContentColumn;
    use crate::modules::content::models::{Content, ContentActiveModel};
    use sea_orm::Set;

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

    // Data'yı olduğu gibi kaydediyoruz (admin UI ile uyumlu JSON bekleniyor)
    let cleaned_data = json.data.clone();

    let active_model = ContentActiveModel {
        data: Set(cleaned_data),
        content_type: Set(content_type.to_string()),
        publish: Set(json.publish.unwrap_or(false)),
        gcx: Set(json.gcx.unwrap_or(false)),
        parent_id: Set(json.parent_id),
        order_id: Set(Some(max_order_id + 1)),
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

    // Term'leri kaydet (kategori + tag'ler)
    if let Some(term_ids) = &json.term_ids {
        if !term_ids.is_empty() {
            if let Err(e) =
                save_content_terms_local(&state.db, page.id, term_ids, &page.content_type).await
            {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        }
    }

    if let Some(tag_ids) = &json.tag_ids {
        if !tag_ids.is_empty() {
            if let Err(e) =
                save_content_terms_local(&state.db, page.id, tag_ids, &page.content_type).await
            {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        }
    }

    // Timeline event oluştur
    let action = if page.publish { "published" } else { "created" };

    let metadata = serde_json::json!({
        "content_type": page.content_type,
        "publish_status": page.publish,
        "has_parent": page.parent_id.is_some(),
        "term_count": json.term_ids.as_ref().map(|t| t.len()).unwrap_or(0) +
                     json.tag_ids.as_ref().map(|t| t.len()).unwrap_or(0)
    });

    if let Err(e) = crate::modules::timeline::helpers::TimelineHelper::create_content_event(
        &state.db,
        page.id,
        &page.content_type,
        action,
        None, // admin_user_id yok (debug)
        Some(metadata),
    )
    .await
    {
        eprintln!("Timeline event oluşturma hatası: {}", e);
        // Timeline hatası ana işlemi etkilemesin
    }

    // Cache / global context yenilemeleri
    if let Err(e) =
        crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
            &state.db,
            &state.global_context_cache,
        )
        .await
    {
        eprintln!("Global context cache yenileme hatası: {}", e);
    }

    // Homepage cache temizle (admin helper kullanılıyor)
    crate::modules::admin::controllers::web::homepage::clear_homepage_cache(&state);

    use crate::modules::admin::controllers::dto::content::ContentExtensions;

    // Admin tarafında beklenen response formatını döndürelim (AdminContentResponse)
    (
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            error: None,
            results: Some(page.get_admin_content_response()),
        }),
    )
        .into_response()
}

/// Local helper: Content-Term ilişkilerini kaydet (admin-api'dekinin küçük bir duplicate'ı)
async fn save_content_terms_local(
    db: &sea_orm::DatabaseConnection,
    content_id: i64,
    term_ids: &[i64],
    content_type: &str,
) -> Result<(), sea_orm::DbErr> {
    use crate::modules::content::models::ContentTermActiveModel;
    use sea_orm::Set;

    for term_id in term_ids {
        let content_term = ContentTermActiveModel {
            content_id: Set(content_id),
            term_id: Set(*term_id),
            content_type: Set(content_type.to_string()),
            created_at: Set(Some(chrono::Utc::now().into())),
        };

        match content_term.insert(db).await {
            Ok(_) => {
                // inserted
            }
            Err(sea_orm::DbErr::RecordNotInserted) => {
                // ignore
            }
            Err(sea_orm::DbErr::Query(e)) if e.to_string().contains("duplicate key") => {
                // ignore duplicate
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}

/// Kullanımı kolaylaştırmak için opsiyonel: bu fonksiyonu content/routes.rs içinde merge edin
/// örn:
///   .merge(crate::modules::content::controllers::api::products::debug_routes())
/// veya
///   .route("/api/debug/products", post(api_controllers::products::debug_create_product))
///
/// Ayrıca multipart desteği için (dosya + payload) ayrı bir endpoint de eklenmiştir:
///   .route("/api/debug/products/media", post(api_controllers::products::debug_create_product_multipart))
#[allow(dead_code)]
pub async fn debug_create_product_multipart(
    State(state): State<AppState>,
    mut multipart: axum_extra::extract::Multipart,
) -> Response {
    // Sadece debug modda aktif olsun
    let config = config::get_config();
    if !config.is_debug() {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<serde_json::Value> {
                success: false,
                results: None,
                error: Some("debug_api_disabled".to_string()),
            }),
        )
            .into_response();
    }

    // Multipart içinde:
    // - payload (text) alanı JSON string (CreateContentRequest formatında)
    // - dosya alanları name formatı: media_<lang>_<slot>  (örn: media_tr_cover, media_en_gallery)
    //   aynı name ile birden fazla dosya gönderilebilir (örneğin cover için birkaç dosya)
    //
    // Dosyalar önce diske yazılır (aynı upload path mantığı ile). İçerik oluşunca media kayıtları
    // page.id ile oluşturulur ve içerik verisinin ilgili media dizilerine (data.langs.<lang>.media.<slot>)
    // eklenir. Böylece dosya + payload aynı request ile kaydedilmiş olur.

    // read multipart
    let mut payload_text: Option<String> = None;

    struct SavedFile {
        lang: String,
        slot: String,
        filename: String,
        mime: String,
        upload_path: std::path::PathBuf,
        size: usize,
    }

    let mut saved_files: Vec<SavedFile> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        // payload field (JSON)
        if name == "payload" {
            if let Ok(text) = field.text().await {
                payload_text = Some(text);
            }
            continue;
        }

        // Dosya alanı: beklenen format media_<lang>_<slot>
        let mut lang = "tr".to_string();
        let mut slot = "cover".to_string();
        if name.starts_with("media_") {
            let parts: Vec<&str> = name.splitn(3, '_').collect();
            if parts.len() >= 3 {
                lang = parts[1].to_string();
                slot = parts[2].to_string();
            }
        }

        let filename = field.file_name().unwrap_or("file").to_string();
        let mime = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        // read bytes
        if let Ok(bytes) = field.bytes().await {
            if bytes.len() as u64 > config.media_max_file_size() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error":"file_too_large","file": filename})),
                )
                    .into_response();
            }

            if !config.is_allowed_mime_type(&mime) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error":"invalid_mime_type","file": filename, "mime": mime})),
                )
                .into_response();
            }

            // write to disk using media_service helper
            let upload_path = crate::modules::media::services::media_service::generate_upload_path(
                config.media_upload_root(),
                &filename,
            );

            if let Some(parent) = upload_path.parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    eprintln!("Failed to create directory: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "FS error").into_response();
                }
            }
            if let Err(e) = tokio::fs::write(&upload_path, &bytes).await {
                eprintln!("Failed to write file: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "FS error").into_response();
            }

            saved_files.push(SavedFile {
                lang,
                slot,
                filename,
                mime,
                upload_path,
                size: bytes.len(),
            });
        }
    }

    // payload zorunlu
    // Debug: log saved files (if any) for easier troubleshooting
    println!("Multipart upload: saved {} files", saved_files.len());
    for sf in &saved_files {
        println!(
            "  - saved file: lang={} slot={} filename={} mime={} path={} size={}",
            sf.lang,
            sf.slot,
            sf.filename,
            sf.mime,
            sf.upload_path.display(),
            sf.size
        );
    }

    let payload_text = match payload_text {
        Some(t) => t,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"missing_payload"})),
            )
                .into_response();
        }
    };

    let create_req: crate::modules::admin::controllers::dto::CreateContentRequest =
        match serde_json::from_str(&payload_text) {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error":"invalid_payload","details": e.to_string()})),
                )
                    .into_response();
            }
        };

    // İçerik oluştur (aynı debug_create_product mantığı)
    let content_type = create_req.content_type.as_deref().unwrap_or("product");
    if content_type != "product" {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<serde_json::Value> {
                success: false,
                results: None,
                error: Some("Only 'product' content_type is allowed for this endpoint".to_string()),
            }),
        )
            .into_response();
    }

    use crate::modules::content::models::content::Column as ContentColumn;
    use crate::modules::content::models::{Content, ContentActiveModel};
    use sea_orm::Set;

    let max_order_id = Content::find()
        .filter(ContentColumn::ContentType.eq(content_type))
        .filter(ContentColumn::DeletedAt.is_null())
        .order_by_desc(ContentColumn::OrderId)
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|c| c.order_id)
        .unwrap_or(0);

    let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();

    let initial_active = ContentActiveModel {
        data: Set(create_req.data.clone()),
        content_type: Set(content_type.to_string()),
        publish: Set(create_req.publish.unwrap_or(false)),
        gcx: Set(create_req.gcx.unwrap_or(false)),
        parent_id: Set(create_req.parent_id),
        order_id: Set(Some(max_order_id + 1)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    };

    let page = match initial_active.insert(&state.db).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Helper: push media objesini uygun yere ekle (data.langs.<lang>.media.<slot>)
    fn push_media_to_data(
        data: &mut serde_json::Value,
        lang: &str,
        slot: &str,
        media_json: serde_json::Value,
    ) {
        // Try data.langs[lang]
        if let Some(map) = data.as_object_mut() {
            if let Some(langs_val) = map.get_mut("langs") {
                if let Some(langs_map) = langs_val.as_object_mut() {
                    if let Some(lang_val) = langs_map.get_mut(lang) {
                        let lang_map = lang_val.as_object_mut().unwrap();
                        let media_val = lang_map
                            .entry("media")
                            .or_insert(serde_json::Value::Object(serde_json::Map::new()));
                        let media_map = media_val.as_object_mut().unwrap();
                        let slot_val = media_map
                            .entry(slot)
                            .or_insert(serde_json::Value::Array(vec![]));
                        if let Some(arr) = slot_val.as_array_mut() {
                            arr.push(media_json);
                            return;
                        }
                    } else {
                        // create lang with media
                        langs_map.insert(
                            lang.to_string(),
                            serde_json::json!({"media": { slot: [media_json] }}),
                        );
                        return;
                    }
                }
            }

            // Fallback: data[lang] (some admin flows use data.<lang> directly)
            if let Some(lang_val) = map.get_mut(lang) {
                let lang_map = lang_val.as_object_mut().unwrap();
                let media_val = lang_map
                    .entry("media")
                    .or_insert(serde_json::Value::Object(serde_json::Map::new()));
                let media_map = media_val.as_object_mut().unwrap();
                let slot_val = media_map
                    .entry(slot)
                    .or_insert(serde_json::Value::Array(vec![]));
                if let Some(arr) = slot_val.as_array_mut() {
                    arr.push(media_json);
                    return;
                }
            }

            // Otherwise create langs structure
            map.insert(
                "langs".to_string(),
                serde_json::json!({ lang: { "media": { slot: [media_json] } } }),
            );
        }
    }

    // Media kayıtlarını oluştur (her savelenmiş dosya için)
    let mut updated_data = page.data.clone(); // start merging media into this
                                              // collect created media info to return in debug response
    let mut created_media: Vec<serde_json::Value> = Vec::new();

    for f in saved_files {
        let rel_path =
            crate::modules::media::services::media_service::normalize_file_path(&f.upload_path);
        let media_type = config.get_media_type_from_mime(&f.mime).to_string();
        match crate::modules::media::services::media_service::create_media(
            &state.db,
            0, // debug user
            f.filename.clone(),
            media_type,
            f.mime.clone(),
            rel_path.clone(),
            f.size as i64,
            None,
            None,
            Some("product".to_string()),
            Some(page.id),
        )
        .await
        {
            Ok(media) => {
                let media_json = serde_json::json!({
                    "id": media.id,
                    "file_name": media.file_name,
                    "mime_type": media.mime_type,
                    "file_path": media.file_path,
                    "url": format!("/{}", media.file_path),
                    "title": media.title,
                    "description": media.description,
                    "content_type": media.content_type,
                    "content_id": media.content_id
                });

                // push into data structure
                push_media_to_data(&mut updated_data, &f.lang, &f.slot, media_json.clone());

                // record created media for response and logging
                created_media.push(media_json);
                println!(
                    "Created media record: id={} file_path={}",
                    media.id, media.file_path
                );
            }
            Err(e) => {
                eprintln!("Failed to create media record: {}", e);
                // record failure for debugging
                created_media.push(serde_json::json!({
                    "error": format!("{}", e),
                    "file_path": rel_path,
                    "file_name": f.filename,
                }));
                // continue with others
            }
        }
    }

    // Güncellenmiş data'yı kaydet (tek update)
    let mut update_active: ContentActiveModel = page.clone().into();
    update_active.data = Set(updated_data.clone());
    update_active.updated_at = Set(Some(chrono::Utc::now().into()));

    let updated_page = match update_active.update(&state.db).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Terms & Tags (aynı şekilde kaydedilsin)
    if let Some(term_ids) = &create_req.term_ids {
        if !term_ids.is_empty() {
            if let Err(e) = save_content_terms_local(
                &state.db,
                updated_page.id,
                term_ids,
                &updated_page.content_type,
            )
            .await
            {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        }
    }

    if let Some(tag_ids) = &create_req.tag_ids {
        if !tag_ids.is_empty() {
            if let Err(e) = save_content_terms_local(
                &state.db,
                updated_page.id,
                tag_ids,
                &updated_page.content_type,
            )
            .await
            {
                eprintln!("Veritabanı hatası: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
            }
        }
    }

    // Timeline event
    let action = if updated_page.publish {
        "published"
    } else {
        "created"
    };

    let metadata = serde_json::json!({
        "content_type": updated_page.content_type,
        "publish_status": updated_page.publish,
        "has_parent": updated_page.parent_id.is_some(),
        "term_count": create_req.term_ids.as_ref().map(|t| t.len()).unwrap_or(0) +
                     create_req.tag_ids.as_ref().map(|t| t.len()).unwrap_or(0)
    });

    if let Err(e) = crate::modules::timeline::helpers::TimelineHelper::create_content_event(
        &state.db,
        updated_page.id,
        &updated_page.content_type,
        action,
        None,
        Some(metadata),
    )
    .await
    {
        eprintln!("Timeline event oluşturma hatası: {}", e);
    }

    // Cache yenilemeleri
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

    use crate::modules::admin::controllers::dto::content::ContentExtensions;

    (
        StatusCode::CREATED,
        Json(ApiResponse::<serde_json::Value> {
            success: true,
            error: None,
            results: Some(serde_json::json!({
                "page": updated_page.get_admin_content_response(),
                "media": created_media
            })),
        }),
    )
        .into_response()
}

// Helper: debug routes (JSON ve multipart)

// Media API Controller - JSON responses for AJAX calls
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use axum_extra::extract::Multipart;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::modules::media::services::media_service;

use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter};
use slug::slugify;
use crate::modules::content::models::content::{Column as ContentColumn, Entity as Content};

// ============ API QUERY/RESPONSE MODELS ============

#[derive(Deserialize)]
pub struct MediaQueryParams {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub media_type: Option<String>,
    pub search: Option<String>,
    pub content_type: Option<String>,
    pub content_id: Option<i64>,
}

#[derive(Serialize)]
pub struct MediaListResponse {
    pub id: i64,
    pub user_id: i32,
    pub file_name: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_path: String,
    pub file_size: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_type: Option<String>,
    pub content_id: Option<i64>,
    pub created_at: Option<String>,
    pub url: String,
}

#[derive(Serialize)]
pub struct PaginatedMediaResponse {
    pub data: Vec<MediaListResponse>,
    pub meta: PaginationMeta,
}

#[derive(Serialize)]
pub struct PaginationMeta {
    pub total: u64,
    pub page: u64,
    pub limit: u64,
    pub total_pages: u64,
}

// ============ HELPER FUNCTIONS ============

// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

async fn get_current_user_id(session: &Session) -> Option<i32> {
    session
        .get::<i64>("user_id")
        .await
        .ok()
        .flatten()
        .map(|id| id as i32)
}

// ============ API ENDPOINTS ============

/// API: List media files
pub async fn list_media(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<MediaQueryParams>,
) -> Response {
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Admin access required"})),
        )
            .into_response();
    }

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let (media_files, total) = match media_service::list_media(
        &state.db,
        page,
        limit,
        query.media_type.as_deref(),
        None,
        query.search.as_deref(),
        query.content_type.as_deref(),
        query.content_id,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Veritabanı hatası"})),
            )
                .into_response();
        }
    };

    let media: Vec<MediaListResponse> = media_files
        .into_iter()
        .map(|m| MediaListResponse {
            id: m.id,
            user_id: m.user_id,
            file_name: m.file_name.clone(),
            media_type: m.media_type.clone(),
            mime_type: m.mime_type.clone(),
            file_path: m.file_path.clone(),
            file_size: m.file_size,
            title: m.title.clone(),
            description: m.description.clone(),
            content_type: m.content_type.clone(),
            content_id: m.content_id,
            created_at: m
                .created_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
            url: format!("/{}", m.file_path),
        })
        .collect();

    let total_pages = (total + limit - 1) / limit;

    let response = PaginatedMediaResponse {
        data: media,
        meta: PaginationMeta {
            total,
            page,
            limit,
            total_pages,
        },
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// API: Get media by ID
pub async fn get_media(
    State(state): State<AppState>,
    session: Session,
    Path(media_id): Path<i64>,
) -> Response {
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Admin access required"})),
        )
            .into_response();
    }

    match media_service::get_media_by_id(&state.db, media_id).await {
        Ok(media) => {
            let response = MediaListResponse {
                id: media.id,
                user_id: media.user_id,
                file_name: media.file_name.clone(),
                media_type: media.media_type.clone(),
                mime_type: media.mime_type.clone(),
                file_path: media.file_path.clone(),
                file_size: media.file_size,
                title: media.title.clone(),
                description: media.description.clone(),
                content_type: media.content_type.clone(),
                content_id: media.content_id,
                created_at: media
                    .created_at
                    .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
                url: format!("/{}", media.file_path),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Media not found"})),
        )
            .into_response(),
    }
}

/// Medya dosyasını ilgili içerik satırı (örneğin ürün SKU'su veya içerik slug'ı) ile otomatik olarak ilişkilendirir.
/// 
/// # Parametreler
/// * `db` - Veritabanı bağlantı referansı.
/// * `media` - Veritabanına yeni eklenen medya kaydı modeli.
/// * `content_type` - İlişkilendirilecek içerik türü (örn: "product", "blog", "news", "page").
/// * `title` - Medya için girilen isteğe bağlı başlık metni.
/// * `description` - Medya için girilen isteğe bağlı açıklama metni.
///
/// # İşleyiş
/// 1. Yüklenen dosya adından uzantı temizlenerek dosya kök adı (`file_stem`) elde edilir.
/// 2. İçerik türü "product" ise, `contents` tablosunda `content_type = 'product'` olan ve JSONB verisindeki `product.sku` değeri
///    dosya kök adı veya slug haliyle (büyük/küçük harf duyarsız) eşleşen satır sorgulanır.
/// 3. İçerik türü "blog", "news" veya "page" ise, dillerdeki (`tr` veya `en`) slug alanlarına göre eşleşen satır aranır.
/// 4. Eşleşen bir içerik satırı bulunursa, görsel bilgileri içeriğin JSONB `langs.{dil}.media.cover` dizisine eklenir (mükerrer kontrolü yapılarak).
/// 5. Değişiklikler veritabanına kaydedilir.
/// 6. Son olarak, `media` tablosundaki orijinal medya kaydının `content_id` alanı da bu içerik ID'sine eşitlenerek yaşam döngüsü referansı tamamlanır.
pub async fn associate_media_with_content(
    db: &sea_orm::DatabaseConnection,
    media: &crate::modules::media::models::media::Model,
    content_type: &str,
    title: &Option<String>,
    description: &Option<String>,
) {
    let filename = &media.file_name;
    let file_stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
        .trim();

    let file_stem_slug = slugify(file_stem);

    println!(
        "BİLGİ: associate_media_with_content çağrıldı. içerik_türü: {}, dosya_adı: {}, dosya_kökü: {}, dosya_kökü_slug: {}",
        content_type, filename, file_stem, file_stem_slug
    );

    let matched_content = if content_type == "product" {
        println!("BİLGİ: SKU ile eşleşen ürün sorgulanıyor...");
        match Content::find()
            .filter(ContentColumn::ContentType.eq("product"))
            .filter(ContentColumn::DeletedAt.is_null())
            .filter(sea_orm::prelude::Expr::cust_with_values(
                "LOWER(data->'product'->>'sku') = LOWER($1) OR LOWER(data->'product'->>'sku') = LOWER($2)",
                vec![
                    sea_orm::Value::from(file_stem.to_string()),
                    sea_orm::Value::from(file_stem_slug.clone()),
                ]
            ))
            .one(db)
            .await
        {
            Ok(c) => {
                if let Some(ref r) = c {
                    println!("BİLGİ: Eşleşen ürün bulundu! ID: {}", r.id);
                } else {
                    println!("UYARI: SKU ile eşleşen hiçbir ürün bulunamadı: {}", file_stem);
                }
                c
            }
            Err(e) => {
                eprintln!("HATA: SKU ile eşleşen ürün veritabanından sorgulanırken hata oluştu: {}", e);
                None
            }
        }
    } else {
        println!("BİLGİ: Slug ile eşleşen içerik türü ({}) sorgulanıyor...", content_type);
        match Content::find()
            .filter(ContentColumn::ContentType.eq(content_type))
            .filter(ContentColumn::DeletedAt.is_null())
            .filter(sea_orm::prelude::Expr::cust_with_values(
                "LOWER(data->'langs'->'tr'->>'slug') = LOWER($1) OR LOWER(data->'langs'->'en'->>'slug') = LOWER($2) OR LOWER(data->'langs'->'tr'->>'slug') = LOWER($3) OR LOWER(data->'langs'->'en'->>'slug') = LOWER($4)",
                vec![
                    sea_orm::Value::from(file_stem.to_string()),
                    sea_orm::Value::from(file_stem.to_string()),
                    sea_orm::Value::from(file_stem_slug.clone()),
                    sea_orm::Value::from(file_stem_slug.clone()),
                ]
            ))
            .one(db)
            .await
        {
            Ok(c) => {
                if let Some(ref r) = c {
                    println!("BİLGİ: Eşleşen içerik satırı bulundu! ID: {}", r.id);
                } else {
                    println!("UYARI: Slug ile eşleşen hiçbir içerik bulunamadı: {}", file_stem);
                }
                c
            }
            Err(e) => {
                eprintln!("HATA: Slug ile eşleşen içerik sorgulanırken hata oluştu (tür: {}): {}", content_type, e);
                None
            }
        }
    };

    if let Some(content_row) = matched_content {
        println!("BİLGİ: Eşleşen içerik mevcut (ID: {}). JSON verisi güncelleniyor...", content_row.id);
        let mut data_val = content_row.data.clone();
        
        if let Some(langs_obj) = data_val.get_mut("langs").and_then(|l| l.as_object_mut()) {
            println!("BİLGİ: JSON verisinde 'langs' nesnesi bulundu");
            for (lang_code, lang_val) in langs_obj.iter_mut() {
                if let Some(lang_obj) = lang_val.as_object_mut() {
                    println!("BİLGİ: Dil güncelleniyor: {}", lang_code);
                    let media_obj = lang_obj.entry("media").or_insert_with(|| {
                        serde_json::json!({
                            "icon": [],
                            "cover": [],
                            "video": [],
                            "gallery": [],
                            "document": []
                        })
                    });
                    
                    if let Some(media_map) = media_obj.as_object_mut() {
                        let cover_arr = media_map.entry("cover").or_insert_with(|| serde_json::json!([]));
                        if let Some(arr) = cover_arr.as_array_mut() {
                            let order_id = (arr.len() + 1) as i64;
                            let new_media_item = serde_json::json!({
                                "id": media.id,
                                "url": format!("/{}", media.file_path),
                                "title": title.clone().unwrap_or_default(),
                                "content": "",
                                "order_id": order_id,
                                "file_name": media.file_name.clone(),
                                "mime_type": media.mime_type.clone(),
                                "description": description.clone().unwrap_or_default()
                            });
                            
                            let already_exists = arr.iter().any(|item| {
                                item.get("id").and_then(|id| id.as_i64()) == Some(media.id)
                            });
                            
                            if !already_exists {
                                println!("BİLGİ: Medya ID {} ilgili dilin ({}) kapak görseli dizisine ekleniyor", media.id, lang_code);
                                arr.push(new_media_item);
                            } else {
                                println!("BİLGİ: Medya ID {} bu dilde ({}) zaten kapak resmi olarak ekli durumda", media.id, lang_code);
                            }
                        }
                    }
                }
            }
        } else {
            println!("UYARI: İçerik JSON verisinde 'langs' nesnesi bulunamadı!");
        }
        
        let content_id = content_row.id;
        let mut active_model: crate::modules::content::models::ContentActiveModel = content_row.into();
        active_model.data = sea_orm::Set(data_val);
        active_model.updated_at = sea_orm::Set(Some(chrono::Utc::now().into()));
        
        if let Err(e) = active_model.update(db).await {
            eprintln!("HATA: İçerik JSON verisi yeni görsellerle güncellenirken hata oluştu: {}", e);
        } else {
            println!("BİLGİ: İçerik satırı başarıyla güncellendi! Medya ID: {}, İçerik ID: {}", media.id, content_id);
            
            // Yaşam döngüsü referanslarının senkronizasyonu için medya kaydındaki content_id alanını da veritabanında güncelleyin
            let mut media_active: crate::modules::media::models::media::ActiveModel = media.clone().into();
            media_active.content_id = sea_orm::Set(Some(content_id));
            if let Err(me) = media_active.update(db).await {
                eprintln!("HATA: Medya kaydındaki content_id alanı güncellenirken hata oluştu: {}", me);
            } else {
                println!("BİLGİ: Medya tablosundaki kaydın content_id alanı başarıyla {} olarak güncellendi", content_id);
            }
        }
    } else {
        println!("BİLGİ: Bu dosya adı için eşleşen herhangi bir içerik satırı bulunamadı.");
    }
}

/// API: Upload media files (multiple)
pub async fn upload_media(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<MediaQueryParams>,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Admin access required"})),
        )
            .into_response();
    }

    let user_id = match get_current_user_id(&session).await {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "User not authenticated"})),
            )
                .into_response();
        }
    };

    let config = crate::config::get_config();
    let mut files_data: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut content_type_form: Option<String> = None;

    // Parse multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "files" => {
                let filename = field.file_name().unwrap_or("unknown").to_string();
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                if let Ok(data) = field.bytes().await {
                    files_data.push((filename, data.to_vec(), content_type));
                }
            }
            "title" => {
                if let Ok(value) = field.text().await {
                    title = Some(value);
                }
            }
            "description" => {
                if let Ok(value) = field.text().await {
                    description = Some(value);
                }
            }
            "content_type" => {
                if let Ok(value) = field.text().await {
                    content_type_form = Some(value);
                }
            }
            _ => {}
        }
    }

    let final_content_type = content_type_form.or_else(|| query.content_type.clone());

    if files_data.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No files provided"})),
        )
            .into_response();
    }

    let mut uploaded_files = Vec::new();
    let mut errors = Vec::new();

    // Process each file
    for (filename, data, content_type) in files_data {
        if data.len() as u64 > config.media_max_file_size() {
            errors.push(format!("{}: File too large", filename));
            continue;
        }

        if !config.is_allowed_mime_type(&content_type) {
            errors.push(format!("{}: File type not allowed", filename));
            continue;
        }

        let upload_path =
            media_service::generate_upload_path(config.media_upload_root(), &filename);

        if let Some(parent) = upload_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                eprintln!("Failed to create directory: {}", e);
                errors.push(format!("{}: Failed to create directory", filename));
                continue;
            }
        }

        if let Err(e) = tokio::fs::write(&upload_path, &data).await {
            eprintln!("Failed to write file: {}", e);
            errors.push(format!("{}: Failed to save file", filename));
            continue;
        }

        let relative_path = media_service::normalize_file_path(&upload_path);
        let media_type = config.get_media_type_from_mime(&content_type);

        match media_service::create_media(
            &state.db,
            user_id,
            filename.clone(),
            media_type.to_string(),
            content_type.clone(),
            relative_path.clone(),
            data.len() as i64,
            title.clone(),
            description.clone(),
            final_content_type.clone(),
            query.content_id,
        )
        .await
        {
            Ok(media) => {
                if let Some(ref ct) = final_content_type {
                    if !ct.is_empty() {
                        associate_media_with_content(&state.db, &media, ct, &title, &description).await;
                    }
                }

                let response = MediaListResponse {
                    id: media.id,
                    user_id: media.user_id,
                    file_name: media.file_name.clone(),
                    media_type: media.media_type.clone(),
                    mime_type: media.mime_type.clone(),
                    file_path: media.file_path.clone(),
                    file_size: media.file_size,
                    title: media.title.clone(),
                    description: media.description.clone(),
                    content_type: media.content_type.clone(),
                    content_id: media.content_id,
                    created_at: media
                        .created_at
                        .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
                    url: format!("/{}", media.file_path),
                };
                uploaded_files.push(response);
            }
            Err(e) => {
                eprintln!("Failed to create media record: {}", e);
                let _ = tokio::fs::remove_file(&upload_path).await;
                errors.push(format!("{}: Failed to create database record", filename));
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "uploaded": uploaded_files,
            "errors": errors,
            "total": uploaded_files.len() + errors.len()
        })),
    )
        .into_response()
}

/// API: Update media (metadata and optionally file)
pub async fn update_media(
    State(state): State<AppState>,
    session: Session,
    Path(media_id): Path<i64>,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Admin access required"})),
        )
            .into_response();
    }

    let config = crate::config::get_config();
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;
    let mut file_data: Option<(String, Vec<u8>, String)> = None;
    let mut new_file_name: Option<String> = None;
    let mut new_mime_type: Option<String> = None;
    let mut new_media_type: Option<String> = None;

    // Parse multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "title" => {
                if let Ok(value) = field.text().await {
                    title = Some(value);
                }
            }
            "description" => {
                if let Ok(value) = field.text().await {
                    description = Some(value);
                }
            }
            "file" => {
                let filename = field.file_name().unwrap_or("unknown").to_string();
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                if let Ok(data) = field.bytes().await {
                    file_data = Some((filename, data.to_vec(), content_type));
                }
            }
            _ => {}
        }
    }

    let mut new_file_path: Option<String> = None;
    let mut new_file_size: Option<i64> = None;

    // If new file provided, process it
    if let Some((filename, data, content_type)) = file_data {
        if data.len() as u64 > config.media_max_file_size() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "File too large"})),
            )
                .into_response();
        }

        if !config.is_allowed_mime_type(&content_type) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "File type not allowed"})),
            )
                .into_response();
        }

        // Get old media to delete old file
        if let Ok(old_media) = media_service::get_media_by_id(&state.db, media_id).await {
            let _ = tokio::fs::remove_file(&old_media.file_path).await;
        }

        let upload_path =
            media_service::generate_upload_path(config.media_upload_root(), &filename);

        if let Some(parent) = upload_path.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                eprintln!("Failed to create directory: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to create directory"})),
                )
                    .into_response();
            }
        }

        if let Err(e) = tokio::fs::write(&upload_path, &data).await {
            eprintln!("Failed to write file: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to save file"})),
            )
                .into_response();
        }

        new_file_path = Some(media_service::normalize_file_path(&upload_path));
        new_file_size = Some(data.len() as i64);
        new_file_name = Some(filename);
        new_mime_type = Some(content_type.clone());
        new_media_type = Some(config.get_media_type_from_mime(&content_type).to_string());
    }

    match media_service::update_media(
        &state.db,
        media_id,
        title,
        description,
        new_file_path,
        new_file_size,
        new_file_name,
        new_mime_type,
        new_media_type,
    )
    .await
    {
        Ok(media) => {
            let response = MediaListResponse {
                id: media.id,
                user_id: media.user_id,
                file_name: media.file_name.clone(),
                media_type: media.media_type.clone(),
                mime_type: media.mime_type.clone(),
                file_path: media.file_path.clone(),
                file_size: media.file_size,
                title: media.title.clone(),
                description: media.description.clone(),
                content_type: media.content_type.clone(),
                content_id: media.content_id,
                created_at: media
                    .created_at
                    .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
                url: format!("/{}", media.file_path),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// API: Delete media
pub async fn delete_media(
    State(state): State<AppState>,
    session: Session,
    Path(media_id): Path<i64>,
) -> Response {
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Admin access required"})),
        )
            .into_response();
    }

    let media = match media_service::get_media_by_id(&state.db, media_id).await {
        Ok(m) => m,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Media not found"})),
            )
                .into_response();
        }
    };

    if let Err(e) = media_service::delete_media(&state.db, media_id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    let _ = tokio::fs::remove_file(&media.file_path).await;

    (
        StatusCode::OK,
        Json(serde_json::json!({"message": "Media deleted successfully"})),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct CloneMediaRequest {
    pub media_ids: Vec<i64>,
    pub content_type: String,
    pub content_id: i64,
}

/// API: Clone media files for a specific content
pub async fn clone_media(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<CloneMediaRequest>,
) -> Response {
    if !is_admin(&state, &session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Admin access required"})),
        )
            .into_response();
    }

    let user_id = match get_current_user_id(&session).await {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "User not authenticated"})),
            )
                .into_response();
        }
    };

    let mut cloned = Vec::new();
    let mut errors = Vec::new();

    let config = crate::config::get_config();

    for media_id in payload.media_ids {
        match media_service::get_media_by_id(&state.db, media_id).await {
            Ok(original) => {
                // Read original file
                let original_path = std::path::Path::new(&original.file_path);
                match tokio::fs::read(original_path).await {
                    Ok(file_data) => {
                        // Generate new upload path
                        let new_upload_path = media_service::generate_upload_path(
                            config.media_upload_root(),
                            &original.file_name,
                        );

                        // Create directory if not exists
                        if let Some(parent) = new_upload_path.parent() {
                            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                                eprintln!("Failed to create directory: {}", e);
                                errors.push(format!(
                                    "Media {}: Failed to create directory",
                                    media_id
                                ));
                                continue;
                            }
                        }

                        // Write physical copy
                        if let Err(e) = tokio::fs::write(&new_upload_path, &file_data).await {
                            eprintln!("Failed to write file: {}", e);
                            errors.push(format!("Media {}: Failed to copy file", media_id));
                            continue;
                        }

                        let new_relative_path =
                            media_service::normalize_file_path(&new_upload_path);

                        // Create database record with new file path
                        match media_service::create_media(
                            &state.db,
                            user_id,
                            original.file_name.clone(),
                            original.media_type.clone(),
                            original.mime_type.clone(),
                            new_relative_path.clone(),
                            file_data.len() as i64,
                            original.title.clone(),
                            original.description.clone(),
                            Some(payload.content_type.clone()),
                            Some(payload.content_id),
                        )
                        .await
                        {
                            Ok(cloned_media) => {
                                cloned.push(MediaListResponse {
                                    id: cloned_media.id,
                                    user_id: cloned_media.user_id,
                                    file_name: cloned_media.file_name.clone(),
                                    media_type: cloned_media.media_type.clone(),
                                    mime_type: cloned_media.mime_type.clone(),
                                    file_path: cloned_media.file_path.clone(),
                                    file_size: cloned_media.file_size,
                                    title: cloned_media.title.clone(),
                                    description: cloned_media.description.clone(),
                                    content_type: cloned_media.content_type.clone(),
                                    content_id: cloned_media.content_id,
                                    created_at: cloned_media
                                        .created_at
                                        .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
                                    url: format!("/{}", cloned_media.file_path),
                                });
                            }
                            Err(e) => {
                                // Clean up file if DB insert fails
                                let _ = tokio::fs::remove_file(&new_upload_path).await;
                                errors.push(format!("Media {}: {}", media_id, e));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read original file: {}", e);
                        errors.push(format!("Media {}: Failed to read original file", media_id));
                    }
                }
            }
            Err(_) => {
                errors.push(format!("Media {} not found", media_id));
            }
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "cloned": cloned,
            "errors": errors,
            "total": cloned.len() + errors.len()
        })),
    )
        .into_response()
}

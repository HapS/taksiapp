// Term API Controller - JSON REST endpoints
use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::controllers::web::homepage::clear_homepage_cache;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::media::services::media_service;
use crate::modules::taxonomy::helpers::{CreateTermRequest, TermResponse, UpdateTermRequest};
use crate::modules::taxonomy::models::term::{Column as TermColumn, Entity as Term, TermActiveModel};
use crate::modules::taxonomy::services::term_service;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter, Set, ActiveModelTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// API: List all terms
pub async fn list(State(state): State<AppState>, auth_user: AuthenticatedUser) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match term_service::list_terms(&state.db, "tr").await {
        Ok(terms) => {
            let response = ApiResponse {
                success: true,
                data: Some(terms),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<Vec<TermResponse>> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// API: Get terms by vocabulary ID (hierarchical with pagination)
pub async fn get_by_vocabulary(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(vocabulary_id): Path<i64>,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = query
        .get("page")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let limit = query
        .get("limit")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(20);
    let search = query.get("search").cloned();
    let parent_id = query.get("parent_id").cloned();
    let sort_by = query.get("sort_by").cloned();
    let sort_order = query.get("sort_order").cloned();

    match term_service::get_terms_hierarchical(
        &state.db,
        vocabulary_id,
        parent_id.clone(),
        page,
        limit,
        search,
        sort_by,
        sort_order,
        "tr",
    )
    .await
    {
        Ok((terms, total, breadcrumbs)) => {
            let total_pages = (total + limit - 1) / limit;

            let response = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "data": terms,
                    "meta": {
                        "total": total,
                        "page": page,
                        "limit": limit,
                        "total_pages": total_pages,
                        "parent_id": parent_id.unwrap_or_else(|| "null".to_string()),
                        "breadcrumbs": breadcrumbs,
                    }
                })),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<serde_json::Value> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
        }
    }
}

/// API: Get term by ID
pub async fn get_by_id(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(params): Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let id = params;

    match term_service::get_term_by_id_with_breadcrumbs(&state.db, id, "tr").await {
        Ok((term, breadcrumbs)) => {
            let response = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "term": term,
                    "breadcrumbs": breadcrumbs,
                })),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<serde_json::Value> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::NOT_FOUND, Json(response)).into_response()
        }
    }
}

/// API: Create term
pub async fn create(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(json): Json<CreateTermRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Debug: Gelen data'yı logla
    eprintln!(
        "📝 Term oluşturuluyor - vocabulary_id: {}",
        json.vocabulary_id
    );
    eprintln!("📝 Gelen data: {:?}", json.data);

    clear_homepage_cache(&state);

    match term_service::create_term(&state.db, json).await {
        Ok(term) => {
            // Menu cache'i yenile (navbar menuüleri için)
            refresh_menu_cache_if_needed(&state, &term).await;

            // Global context cache'i yenile (mega menü için)
            refresh_global_context_cache_if_needed(&state, &term).await;

            let response = ApiResponse {
                success: true,
                data: Some(term),
                error: None,
            };
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<crate::modules::taxonomy::models::term::Model> =
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// API: Update term
pub async fn update(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(params): Path<i64>,
    Json(json): Json<UpdateTermRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    clear_homepage_cache(&state);

    let id = params;

    match term_service::update_term(&state.db, id, json).await {
        Ok(term) => {
            // Menu cache'i yenile (navbar menuüleri için)
            refresh_menu_cache_if_needed(&state, &term).await;

            // Global context cache'i yenile (mega menü için)
            refresh_global_context_cache_if_needed(&state, &term).await;

            let response = ApiResponse {
                success: true,
                data: Some(term),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<crate::modules::taxonomy::models::term::Model> =
                ApiResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// API: Delete term
pub async fn delete(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(params): Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let id = params;

    // Load term before deleting to collect media IDs
    let term_model = match Term::find_by_id(id).one(&state.db).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some("Term not found".to_string()),
            };
            return (StatusCode::NOT_FOUND, Json(response)).into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // datayı fonksiyona gönder ve dil içindeki tüm media ID'lerini topla
    let mut media_ids: Vec<i64> = Vec::new();
    collect_media_ids_from_value(&term_model.data, &mut media_ids);
    media_ids.sort();
    media_ids.dedup();

    match term_service::delete_term(&state.db, id).await {
        Ok(_) => {
            // İlişkili media dosyalarını sil
            if !media_ids.is_empty() {
                let upload_root = state.config.media_upload_root().to_string();
                for mid in media_ids {
                    match media_service::delete_media_and_file(&state.db, mid, &upload_root).await {
                        Ok(_) => {
                            eprintln!("Deleted media id={} associated with term id={}", mid, id)
                        }
                        Err(e) => eprintln!(
                            "Failed to delete media id={} for term id={} : {}",
                            mid, id, e
                        ),
                    }
                }
            }

            // Menu cache'i her zaman yenile (term silinince)
            refresh_menu_cache(&state).await;

            // Global context cache'i her zaman yenile (term silinince)
            refresh_global_context_cache(&state).await;

            let response: ApiResponse<()> = ApiResponse {
                success: true,
                data: None,
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// API: Toggle term publish status
pub async fn toggle_publish(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Get current term
    let term = match Term::find_by_id(id).one(&state.db).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some("Term not found".to_string()),
            };
            return (StatusCode::NOT_FOUND, Json(response)).into_response();
        }
        Err(e) => {
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response();
        }
    };

    // Toggle publish status
    let new_publish_status = !term.publish;

    let mut active_model: TermActiveModel = term.into();
    active_model.publish = Set(new_publish_status);

    match active_model.update(&state.db).await {
        Ok(_) => {
            // Refresh cache after toggle
            refresh_menu_cache(&state).await;
            refresh_global_context_cache(&state).await;

            let response = ApiResponse {
                success: true,
                data: Some(serde_json::json!({
                    "publish": new_publish_status
                })),
                error: None,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            let response: ApiResponse<()> = ApiResponse {
                success: false,
                data: None,
                error: Some(e.to_string()),
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct TermOrderItem {
    pub id: i64,
    pub order_id: i32,
}

#[derive(Deserialize)]
pub struct UpdateTermOrderRequest {
    pub orders: Vec<TermOrderItem>,
}

/// API: Update term order
pub async fn update_order(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(json): Json<UpdateTermOrderRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Update each term's order_id
    for order_item in json.orders {
        if let Err(e) = Term::update_many()
            .col_expr(TermColumn::OrderId, Expr::value(order_item.order_id))
            .filter(TermColumn::Id.eq(order_item.id))
            .exec(&state.db)
            .await
        {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    }

    // Menu sıralaması değişti, cache'i yenile
    refresh_menu_cache(&state).await;

    // Global context cache'i de yenile (mega menü sıralaması için)
    refresh_global_context_cache(&state).await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Term order updated successfully"
        })),
    )
        .into_response()
}

/// Helper: Global context cache'i yenile (mega menü için)
async fn refresh_global_context_cache_if_needed(
    state: &AppState,
    term: &crate::modules::taxonomy::models::term::Model,
) {
    // Global context'te bulunan vocabulary'leri kontrol et
    // Şu anda vocabulary ID 1 mega menüde kullanılıyor
    if term.vocabulary_id == 1 {
        refresh_global_context_cache(state).await;
    }
}

/// Helper: Global context cache'i yenile (tüm global context)
async fn refresh_global_context_cache(state: &AppState) {
    match crate::modules::content::helpers::global_context_helper::load_global_context(&state.db)
        .await
    {
        Ok(new_context) => {
            if let Ok(mut cache) = state.global_context_cache.write() {
                *cache = new_context;
                eprintln!("✅ Global context cache yenilendi!");
            } else {
                eprintln!("⚠️  Global context cache lock alınamadı!");
            }
        }
        Err(e) => {
            eprintln!("⚠️  Global context cache yenilenemedi: {}", e);
        }
    }
}

/// Helper: Menu cache'i yenile (navbar menüsü için)
async fn refresh_menu_cache_if_needed(
    state: &AppState,
    term: &crate::modules::taxonomy::models::term::Model,
) {
    // Navbar menü vocabulary ID'sini kontrol et
    let navbar_vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(&state.db, "navbar_menu")
            .await
            .unwrap_or(2);

    // Eğer term navbar menüsüne aitse cache'i yenile
    if term.vocabulary_id == navbar_vocab_id {
        refresh_menu_cache(state).await;
    }
}

/// Helper: Menu cache'i yenile (tüm diller için)
async fn refresh_menu_cache(state: &AppState) {
    let languages: Vec<String> = state.config.supported_languages.keys().cloned().collect();

    match crate::middleware::global_context::MenuCache::load_from_db(&state.db, &languages).await {
        Ok(new_cache) => {
            if let Ok(mut cache) = state.menu_cache.write() {
                *cache = new_cache;
                eprintln!("✅ Menu cache yenilendi!");
            } else {
                eprintln!("⚠️  Menu cache lock alınamadı!");
            }
        }
        Err(e) => {
            eprintln!("⚠️  Menu cache yenilenemedi: {}", e);
        }
    }
}

// Helper: Recursively collect media IDs from arbitrary JSON (e.g., term.data langs gallery, etc.)
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

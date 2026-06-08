use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::middleware::global_context::CurrentLanguage;
use crate::modules::timeline::services::{TimelineService, TimelineEventFilter};
use axum::{
    extract::{Query, State, Path},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    pub module_type: Option<String>,
    pub content_type: Option<String>,
    pub content_id: Option<i64>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    #[allow(dead_code)]
    pub lang: Option<String>, // Dil parametresi
}

#[derive(Debug, Serialize)]
pub struct TimelineEventResponse {
    pub id: i64,
    pub module_type: String,
    pub content_type: String,
    pub content_id: i64,
    pub event_type: String,
    pub title: String, // Kullanıcının diline göre title
    pub description: Option<String>, // Kullanıcının diline göre description
    pub icon: Option<String>,
    pub color: Option<String>,
    pub user_id: Option<i64>,
    pub admin_user_id: Option<i64>,
    pub metadata: Option<serde_json::Value>,
    pub is_public: bool,
    pub is_admin_only: bool,
    pub created_at: Option<String>,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T, message: &str) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data: Some(data),
            total: None,
        }
    }

    pub fn success_with_total(data: T, total: u64, message: &str) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data: Some(data),
            total: Some(total),
        }
    }

    pub fn error(message: &str) -> ApiResponse<T> {
        ApiResponse {
            success: false,
            message: message.to_string(),
            data: None,
            total: None,
        }
    }
}

impl TimelineEventResponse {
    pub fn from_model_with_lang(event: crate::modules::timeline::models::timeline_event::Model, lang: &str) -> Self {
        // Title'ı dile göre çevir
        let title = extract_localized_text(&event.title, lang, "title").unwrap_or_else(|| "Event".to_string());
        
        // Description'ı dile göre çevir
        let description = event.description.as_ref()
            .and_then(|desc| extract_localized_text(desc, lang, "description"));

        Self {
            id: event.id,
            module_type: event.module_type,
            content_type: event.content_type,
            content_id: event.content_id,
            event_type: event.event_type,
            title,
            description,
            icon: event.icon,
            color: event.color,
            user_id: event.user_id,
            admin_user_id: event.admin_user_id,
            metadata: event.metadata,
            is_public: event.is_public,
            is_admin_only: event.is_admin_only,
            created_at: event.created_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        }
    }
}

/// JSON'dan dile göre text çıkarır
/// Format: {"langs": {"tr": {"title": "..."}, "en": {"title": "..."}}}
fn extract_localized_text(json_value: &serde_json::Value, lang: &str, field: &str) -> Option<String> {
    json_value
        .get("langs")
        .and_then(|langs| langs.as_object())
        .and_then(|langs_obj| {
            // Önce istenen dili dene
            langs_obj.get(lang)
                .or_else(|| langs_obj.get("tr")) // Fallback: Türkçe
                .or_else(|| langs_obj.get("en")) // Fallback: İngilizce
                .or_else(|| langs_obj.values().next()) // Fallback: İlk dil
        })
        .and_then(|lang_data| lang_data.get(field))
        .and_then(|text| text.as_str())
        .map(|s| s.to_string())
}

/// Kullanıcının timeline'ını getir
pub async fn get_user_timeline(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    current_lang: CurrentLanguage,
    Query(query): Query<TimelineQuery>,
) -> impl IntoResponse {
    match TimelineService::get_user_timeline(
        &state.db,
        auth_user.id,
        query.limit,
        query.offset,
    ).await {
        Ok(events) => {
            let responses: Vec<TimelineEventResponse> = events.into_iter()
                .map(|event| TimelineEventResponse::from_model_with_lang(event, &current_lang.0))
                .collect();
            (
                StatusCode::OK,
                Json(ApiResponse::success(responses, "timeline başarıyla getirildi")),
            )
        }
        Err(e) => {
            eprintln!("Timeline getirme hatası: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("timeline getirilirken hata oluştu")),
            )
        }
    }
}

/// Belirli içerik için timeline getir
pub async fn get_content_timeline(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    current_lang: CurrentLanguage,
    Path((module_type, content_type, content_id)): Path<(String, String, i64)>,
    Query(_query): Query<TimelineQuery>,
) -> impl IntoResponse {
    // Admin kontrolü yap
    let is_admin = crate::modules::auth::helpers::rbac::check_admin_access_api(&state, auth_user.id).await.is_ok();

    match TimelineService::get_content_timeline(
        &state.db,
        &module_type,
        &content_type,
        content_id,
        Some(auth_user.id),
        is_admin,
    ).await {
        Ok(events) => {
            let responses: Vec<TimelineEventResponse> = events.into_iter()
                .map(|event| TimelineEventResponse::from_model_with_lang(event, &current_lang.0))
                .collect();
            (
                StatusCode::OK,
                Json(ApiResponse::success(responses, "içerik timeline'ı başarıyla getirildi")),
            ).into_response()
        }
        Err(e) => {
            eprintln!("Content timeline getirme hatası: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<Vec<TimelineEventResponse>>::error("timeline getirilirken hata oluştu")),
            ).into_response()
        }
    }
}

/// Genel timeline listesi (filtrelenebilir)
pub async fn list_timeline_events(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    current_lang: CurrentLanguage,
    Query(query): Query<TimelineQuery>,
) -> impl IntoResponse {
    let filter = TimelineEventFilter {
        module_type: query.module_type,
        content_type: query.content_type,
        content_id: query.content_id,
        user_id: Some(auth_user.id), // Kullanıcı sadece kendi eventlerini görebilir
        is_public: Some(true),
        is_admin_only: Some(false),
        limit: query.limit,
        offset: query.offset,
    };

    match TimelineService::get_events(&state.db, filter.clone()).await {
        Ok(events) => {
            let responses: Vec<TimelineEventResponse> = events.into_iter()
                .map(|event| TimelineEventResponse::from_model_with_lang(event, &current_lang.0))
                .collect();
            
            // Toplam sayıyı da getir
            match TimelineService::count_events(&state.db, filter).await {
                Ok(total) => (
                    StatusCode::OK,
                    Json(ApiResponse::success_with_total(responses, total, "timeline eventleri başarıyla getirildi")),
                ),
                Err(_) => (
                    StatusCode::OK,
                    Json(ApiResponse::success(responses, "timeline eventleri başarıyla getirildi")),
                ),
            }
        }
        Err(e) => {
            eprintln!("Timeline eventleri getirme hatası: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("timeline eventleri getirilirken hata oluştu")),
            )
        }
    }
}
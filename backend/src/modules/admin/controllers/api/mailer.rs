// Admin Mailer API Controllers - JSON responses for AJAX calls
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use sea_orm::*;
use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::mailer::models::mail_queue::Column as MailQueueColumn;
use crate::modules::mailer::models::MailQueue;

// use crate::config::get_config;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
// ============ TEST MAIL MODELS ============

#[derive(Deserialize)]
pub struct TestMailRequest {
    pub template_name: String,
    pub to_email: String,
    pub to_name: Option<String>,
    pub language: Option<String>,
}

#[derive(Serialize)]
pub struct TestMailResponse {
    pub success: bool,
    pub message: String,
    pub mail_id: Option<i64>,
}

// ============ API QUERY/RESPONSE MODELS ============

#[derive(Deserialize)]
pub struct MailQueueQueryParams {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub template_name: Option<String>,
}

#[derive(Serialize)]
pub struct MailQueueListResponse {
    pub id: i64,
    pub template_name: Option<String>,
    pub to_email: String,
    pub to_name: Option<String>,
    pub subject: String,
    pub language: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub error_message: Option<String>,
    pub scheduled_at: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct MailQueuePaginatedResponse {
    pub data: Vec<MailQueueListResponse>,
    pub meta: MailQueuePaginationMeta,
}

#[derive(Serialize)]
pub struct MailQueuePaginationMeta {
    pub total: i64,
    pub page: i64,
    pub limit: i64,
    pub total_pages: i64,
}

// ============ API ENDPOINTS ============

// API: Admin mail kuyruğu listesi (gelişmiş filtreleme ile)
pub async fn admin_api_list_mail_queue(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser, // Authentication kontrolü middleware tarafından yapılıyor
    Query(query): Query<MailQueueQueryParams>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * limit;

    // Base query
    let mut select = MailQueue::find();

    // Status filtresi
    if let Some(status) = &query.status {
        if !status.is_empty() {
            select = select.filter(MailQueueColumn::Status.eq(status.as_str()));
        }
    }

    // Template name filtresi
    if let Some(template_name) = &query.template_name {
        if !template_name.is_empty() {
            select = select.filter(MailQueueColumn::TemplateName.eq(template_name.as_str()));
        }
    }

    // Arama filtresi (email veya subject'te arama)
    if let Some(search) = &query.search {
        if !search.is_empty() {
            let search_pattern = format!("%{}%", search.to_lowercase());
            select = select.filter(
                Condition::any()
                    .add(MailQueueColumn::ToEmail.like(&search_pattern))
                    .add(MailQueueColumn::Subject.like(&search_pattern))
                    .add(MailQueueColumn::ToName.like(&search_pattern)),
            );
        }
    }

    // Tarih filtreleri
    if let Some(start_date) = &query.start_date {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
            let datetime = date.and_hms_opt(0, 0, 0).unwrap();
            select = select.filter(MailQueueColumn::CreatedAt.gte(datetime));
        }
    }

    if let Some(end_date) = &query.end_date {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
            let datetime = date.and_hms_opt(23, 59, 59).unwrap();
            select = select.filter(MailQueueColumn::CreatedAt.lte(datetime));
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

    // Veri çekme - created_at'e göre ters sırala (en yeni önce)
    let mails = match select
        .order_by_desc(MailQueueColumn::CreatedAt)
        .offset(offset as u64)
        .limit(limit as u64)
        .all(&state.db)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Veritabanı hatası").into_response();
        }
    };

    // Response formatı
    let mut response_data = Vec::new();
    for mail_model in mails {
        response_data.push(MailQueueListResponse {
            id: mail_model.id,
            template_name: mail_model.template_name,
            to_email: mail_model.to_email,
            to_name: mail_model.to_name,
            subject: mail_model.subject,
            language: mail_model.language.unwrap_or("tr".to_string()),
            status: mail_model.status.unwrap_or("pending".to_string()),
            attempts: mail_model.attempts.unwrap_or(0),
            max_attempts: mail_model.max_attempts.unwrap_or(3),
            error_message: mail_model.error_message,
            scheduled_at: mail_model
                .scheduled_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
            sent_at: mail_model
                .sent_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string()),
            created_at: mail_model
                .created_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default(),
            updated_at: mail_model
                .updated_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default(),
        });
    }

    let total_pages = (total as u64 + limit as u64 - 1) / limit as u64;

    (
        StatusCode::OK,
        Json(MailQueuePaginatedResponse {
            data: response_data,
            meta: MailQueuePaginationMeta {
                total,
                page,
                limit,
                total_pages: total_pages as i64,
            },
        }),
    )
        .into_response()
}

// API: Mail kuyruğunu işle
pub async fn admin_api_process_mail_queue(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser, // Authentication kontrolü middleware tarafından yapılıyor
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let mail_service = crate::modules::mailer::MailService::new(state.db.clone());

    match mail_service.process_queue().await {
        Ok(processed) => Json(serde_json::json!({
            "success": true,
            "message": format!("{} mail işlendi", processed),
            "processed": processed
        }))
        .into_response(),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "message": format!("Mail işleme hatası: {:?}", e)
        }))
        .into_response(),
    }
}

// API: Mail kuyruk istatistikleri
pub async fn admin_api_get_mail_queue_stats(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser, // Authentication kontrolü middleware tarafından yapılıyor
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let mail_service = crate::modules::mailer::MailService::new(state.db.clone());

    match mail_service.get_queue_stats().await {
        Ok(stats) => Json(serde_json::json!({
            "success": true,
            "stats": stats
        }))
        .into_response(),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "message": format!("İstatistik alma hatası: {:?}", e)
        }))
        .into_response(),
    }
}

// API: Mail'i tekrar kuyruğa ekle (retry)
pub async fn admin_api_retry_mail(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser, // Authentication kontrolü middleware tarafından yapılıyor
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Mail'i bul
    let mail = match MailQueue::find_by_id(id).one(&state.db).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            return Json(serde_json::json!({
                "success": false,
                "message": "Mail bulunamadı"
            }))
            .into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return Json(serde_json::json!({
                "success": false,
                "message": "Veritabanı hatası"
            }))
            .into_response();
        }
    };

    // Mail'i tekrar kuyruğa ekle
    let mut active_model: crate::modules::mailer::models::mail_queue::ActiveModel = mail.into();
    active_model.status = Set(Some("pending".to_string()));
    active_model.attempts = Set(Some(0));
    active_model.scheduled_at = Set(None);
    active_model.error_message = Set(None);
    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    match active_model.update(&state.db).await {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": "Mail tekrar kuyruğa eklendi"
        }))
        .into_response(),
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            Json(serde_json::json!({
                "success": false,
                "message": "Mail güncellenemedi"
            }))
            .into_response()
        }
    }
}

// API: Mail'i sil
pub async fn admin_api_delete_mail(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser, // Authentication kontrolü middleware tarafından yapılıyor
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match MailQueue::delete_by_id(id).exec(&state.db).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            Json(serde_json::json!({
                "success": false,
                "message": "Mail silinemedi"
            }))
            .into_response()
        }
    }
}

// ============ TEST MAIL ENDPOINTS ============

/// POST /admin/api/send-simple-mail - Basit mail gönderimi (tek parametre)
pub async fn admin_api_send_simple_mail(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(data): Json<serde_json::Value>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match crate::modules::mailer::MailHelper::send_simple_mail(&state.db, data).await {
        Ok(mail_id) => Json(TestMailResponse {
            success: true,
            message: format!("Mail başarıyla kuyruğa eklendi (ID: {})", mail_id),
            mail_id: Some(mail_id),
        })
        .into_response(),
        Err(e) => Json(TestMailResponse {
            success: false,
            message: format!("Mail gönderilemedi: {}", e),
            mail_id: None,
        })
        .into_response(),
    }
}

/// Test mail gönderme endpoint'i
pub async fn admin_api_send_test_mail(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<TestMailRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let language = request.language.unwrap_or_else(|| "tr".to_string());

    // Template'e göre test verisi hazırla
    let mail_id = match request.template_name.as_str() {
        "user_verification" => {
            crate::modules::mailer::MailHelper::send_user_verification_with_app_state(
                &state,
                &request.to_email,
                &request
                    .to_name
                    .unwrap_or_else(|| "Test Kullanıcı".to_string()),
                "123456",
                &language,
            )
            .await
        }
        "welcome" => {
            // Basit custom mail gönderelim - AppState ile tema desteği
            let template_service = crate::modules::mailer::TemplateService::with_app_state(
                state.db.clone(),
                std::sync::Arc::new(state.clone()),
            );
            let mut variables = std::collections::HashMap::new();
            let user_name = request
                .to_name
                .clone()
                .unwrap_or_else(|| "Test Kullanıcı".to_string());
            variables.insert(
                "name".to_string(),
                serde_json::Value::String(user_name.clone()),
            );
            variables.insert(
                "email".to_string(),
                serde_json::Value::String(request.to_email.clone()),
            );
            variables.insert(
                "registration_date".to_string(),
                serde_json::Value::String(chrono::Utc::now().format("%d.%m.%Y").to_string()),
            );
            variables.insert(
                "site_url".to_string(),
                serde_json::Value::String("https://example.com".to_string()),
            );

            let subject = if language == "tr" {
                "Hoş Geldiniz"
            } else {
                "Welcome"
            };

            template_service
                .queue_mail(
                    "welcome",
                    &request.to_email,
                    Some(&user_name),
                    subject,
                    variables,
                    &language,
                    None,
                )
                .await
        }
        "order_confirmation" => {
            crate::modules::mailer::MailHelper::send_order_confirmation(
                &state.db,
                &request.to_email,
                &request
                    .to_name
                    .unwrap_or_else(|| "Test Kullanıcı".to_string()),
                "ORD-12345",
                "20.12.2024",
                "Kredi Kartı",
                "Test Ürün x1",
                "150.00 TL",
                "Test Adres, İstanbul",
                "https://example.com/orders/12345",
                None,        // order_items
                Some("TRY"), // currency
                &language,
            )
            .await
        }
        _ => {
            return Json(TestMailResponse {
                success: false,
                message: format!("Bilinmeyen template: {}", request.template_name),
                mail_id: None,
            })
            .into_response();
        }
    };

    match mail_id {
        Ok(id) => Json(TestMailResponse {
            success: true,
            message: "Test mail başarıyla kuyruğa eklendi".to_string(),
            mail_id: Some(id),
        })
        .into_response(),
        Err(e) => Json(TestMailResponse {
            success: false,
            message: format!("Mail gönderme hatası: {:?}", e),
            mail_id: None,
        })
        .into_response(),
    }
}

/// Basit test mail endpoint'i (authentication olmadan)
pub async fn admin_api_test_mail_simple(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }
    // Template service oluştur - AppState ile tema desteği
    let template_service = crate::modules::mailer::TemplateService::with_app_state(
        state.db.clone(),
        std::sync::Arc::new(state.clone()),
    );

    // Test değişkenleri
    let mut variables = std::collections::HashMap::new();
    variables.insert(
        "name".to_string(),
        serde_json::Value::String("Tamer Yiğit Kullanıcısı".to_string()),
    );
    variables.insert(
        "email".to_string(),
        serde_json::Value::String("tamer4yigit@gmail.com".to_string()),
    );
    variables.insert(
        "verification_code".to_string(),
        serde_json::Value::String("123456".to_string()),
    );

    // Mail kuyruğa ekle
    match template_service
        .queue_mail(
            "user_verification",
            "tamer4yigit@gmail.com",
            Some("Tamer YİĞİT"),
            "Hesap Doğrulama",
            variables,
            "tr",
            None,
        )
        .await
    {
        Ok(mail_id) => Json(serde_json::json!({
            "success": true,
            "message": "Test mail başarıyla kuyruğa eklendi",
            "mail_id": mail_id
        }))
        .into_response(),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "message": format!("Hata: {:?}", e)
        }))
        .into_response(),
    }
}

use crate::app_state::AppState;
use crate::config::get_config;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::admin::services::order_service::{self, AdminOrderResponse, AdminServiceError};
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::utils::format_price::format_price;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use sea_orm::{EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub meta: Option<PaginationMeta>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
}

#[derive(Deserialize, Clone)]
pub struct OrderListQuery {
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub search: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    // Sorting support
    pub sort_by: Option<String>,
    // Expected values: "asc" or "desc"
    pub sort_order: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateOrderStatusRequest {
    pub status: String,
    pub admin_notes: Option<String>,
    pub cargo_company: Option<i64>,
    pub cargo_tracking_no: Option<String>,
}

/// GET /api/admin/orders - Admin sipariş listesi
pub async fn get_orders(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<OrderListQuery>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    // Debug log
    println!(
        "🔍 Orders API called with: page={}, per_page={}, status={:?}, search={:?}, sort_by={:?}, sort_order={:?}",
        page, per_page, query.status, query.search, query.sort_by, query.sort_order
    );

    match order_service::get_admin_orders(
        &state.db,
        query.status.clone(),
        query.start_date.clone(),
        query.end_date.clone(),
        query.search.clone(),
        page,
        per_page,
        query.sort_by.clone(),
        query.sort_order.clone(),
    )
    .await
    {
        Ok((orders, total)) => {
            let total_pages = (total + per_page - 1) / per_page;

            (
                StatusCode::OK,
                Json(PaginatedResponse {
                    success: true,
                    data: Some(orders),
                    meta: Some(PaginationMeta {
                        page,
                        per_page,
                        total,
                        total_pages,
                    }),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PaginatedResponse::<Vec<AdminOrderResponse>> {
                success: false,
                data: None,
                meta: None,
                error: Some(format!("Siparişler getirilemedi: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// GET /api/admin/orders/{id} - Sipariş detayı
pub async fn get_order(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(order_id): Path<i64>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match order_service::get_admin_order(&state.db, order_id).await {
        Ok(order) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(order),
                error: None,
            }),
        )
            .into_response(),
        Err(AdminServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminOrderResponse> {
                success: false,
                data: None,
                error: Some("Sipariş bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminOrderResponse> {
                success: false,
                data: None,
                error: Some(format!("Hata: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /api/admin/orders/{id}/status - Sipariş durumu güncelle
pub async fn update_order_status(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(order_id): Path<i64>,
    Json(request): Json<UpdateOrderStatusRequest>,
) -> Response {
    // Admin access kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Önce mevcut siparişi al (mail gönderimi için)
    let old_order = match order_service::get_admin_order(&state.db, order_id).await {
        Ok(order) => Some(order),
        Err(_) => None,
    };

    // Admin user ID'sini almak için session'dan çekebiliriz, şimdilik None
    let admin_user_id = None; // TODO: Session'dan admin user ID'sini al

    match order_service::update_order_status(
        &state.db,
        order_id,
        request.status.clone(),
        request.admin_notes.clone(),
        request.cargo_company.clone(),
        request.cargo_tracking_no.clone(),
        admin_user_id,
    )
    .await
    {
        Ok(updated_order) => {
            // Mail gönderimi (async olarak, hata olsa bile API yanıtını etkilemesin)
            if let Some(old_order_data) = old_order {
                let db_clone = state.db.clone();
                let new_status = request.status.clone();
                let old_status = old_order_data.status.clone();
                let cargo_company = request.cargo_company.clone();
                let cargo_tracking_no = request.cargo_tracking_no.clone();

                // Background task olarak mail gönder
                tokio::spawn(async move {
                    // Sipariş durumu değişmişse mail gönder
                    if old_status != new_status {
                        // User'ı database'den al (email için)
                        use crate::modules::auth::models::User;
                        use sea_orm::EntityTrait;

                        let user = match User::find_by_id(old_order_data.user_id)
                            .one(&db_clone)
                            .await
                        {
                            Ok(Some(u)) => u,
                            Ok(None) => {
                                eprintln!("❌ User bulunamadı: {}", old_order_data.user_id);
                                return;
                            }
                            Err(e) => {
                                eprintln!("❌ User sorgu hatası: {:?}", e);
                                return;
                            }
                        };

                        let status_message = match new_status.as_str() {
                            "pending" => "Siparişiniz alınmıştır ve işleniyor.",
                            "confirmed" => "Siparişiniz başarıyla alınmış ve onaylanmıştır.",
                            "preparing" => "Siparişiniz hazırlanmaya başlanmıştır.",
                            "shipped" => "Siparişiniz kargoya verilmiştir.",
                            "delivered" => "Siparişiniz başarıyla teslim edilmiştir.",
                            "cancelled" => "Siparişiniz iptal edilmiştir.",
                            _ => "Sipariş durumunuz güncellenmiştir.",
                        };

                        // Kullanıcı adını oluştur
                        let user_name = format!(
                            "{} {}",
                            user.first_name.as_deref().unwrap_or(""),
                            user.last_name.as_deref().unwrap_or("")
                        )
                        .trim()
                        .to_string();

                        let user_name = if user_name.is_empty() {
                            user.username.clone()
                        } else {
                            user_name
                        };

                        let config = get_config();

                        // Mail gönder
                        if let Err(e) =
                            crate::modules::mailer::MailHelper::send_order_status_update(
                                &db_clone,
                                &user.email, // Users tablosundan email
                                &user_name,
                                &old_order_data
                                    .order_id
                                    .as_deref()
                                    .unwrap_or(&old_order_data.id.to_string()),
                                &old_status,
                                &new_status,
                                status_message,
                                cargo_company.as_ref(),
                                cargo_tracking_no.as_deref(),
                                &format!("{}/my-account/orders", config.get_base_url(),),
                                old_order_data
                                    .total_amount
                                    .as_ref()
                                    .map(|amount| {
                                        use rust_decimal::prelude::ToPrimitive;
                                        let currency =
                                            old_order_data.currency.as_deref().unwrap_or("TRY");
                                        format_price(amount.to_f64().unwrap_or(0.0), currency)
                                    })
                                    .as_deref(),
                                old_order_data
                                    .items
                                    .as_ref()
                                    .map(|items| serde_json::to_value(items).unwrap_or_default())
                                    .as_ref(),
                                old_order_data.currency.as_deref(),
                                "tr", // TODO: Kullanıcının dil tercihini al
                            )
                            .await
                        {
                            eprintln!("❌ Sipariş durumu mail gönderimi hatası: {:?}", e);
                        } else {
                            println!(
                                "✅ Sipariş durumu maili kuyruğa eklendi: {} -> {} ({})",
                                old_status, new_status, user.email
                            );
                        }
                    }
                });
            }

            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(updated_order),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(AdminServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<AdminOrderResponse> {
                success: false,
                data: None,
                error: Some("Sipariş bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<AdminOrderResponse> {
                success: false,
                data: None,
                error: Some(format!("Sipariş güncellenemedi: {:?}", e)),
            }),
        )
            .into_response(),
    }
}
// use crate::modules::ecommerce::models::kargo_sirketleri;
use crate::modules::ecommerce::models::kargo_sirketleri::Column as KargoSirketleriColumn;
use crate::modules::ecommerce::models::KargoSirketleriEntity;
use sea_orm::ColumnTrait;
pub async fn kargo_sirketleri_list(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> impl IntoResponse {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Fetch kargo şirketleri from database
    let kargo_sirketleri_list = match KargoSirketleriEntity::find()
        .filter(KargoSirketleriColumn::Publish.eq(true))
        .all(&state.db)
        .await
    {
        Ok(list) => list,
        Err(err) => {
            eprintln!("Error fetching kargo şirketleri: {}", err);
            vec![]
        }
    };

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: Some(kargo_sirketleri_list),
            error: None,
        }),
    )
        .into_response()
}

/// PUT /api/admin/orders/:cart_id/items/:item_id/cancel/accept - İptal talebini onayla
pub async fn accept_cancel_request(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((cart_id, item_id)): Path<(i64, i64)>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match order_service::accept_cancel_request(&state.db, cart_id, item_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::<String> {
                success: true,
                data: Some("İptal talebi onaylandı".to_string()),
                error: None,
            }),
        )
            .into_response(),
        Err(AdminServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<String> {
                success: false,
                data: None,
                error: Some("İptal talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<String> {
                success: false,
                data: None,
                error: Some(format!("Veritabanı hatası: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /api/admin/orders/:cart_id/items/:item_id/cancel/reject - İptal talebini reddet
pub async fn reject_cancel_request(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path((cart_id, item_id)): Path<(i64, i64)>,
) -> Response {
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match order_service::reject_cancel_request(&state.db, cart_id, item_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::<String> {
                success: true,
                data: Some("İptal talebi reddedildi".to_string()),
                error: None,
            }),
        )
            .into_response(),
        Err(AdminServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<String> {
                success: false,
                data: None,
                error: Some("İptal talebi bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<String> {
                success: false,
                data: None,
                error: Some(format!("Veritabanı hatası: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

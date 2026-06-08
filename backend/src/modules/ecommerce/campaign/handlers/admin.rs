use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::app_state::AppState;
use crate::middleware::global_context::GlobalContext;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::ecommerce::campaign::dto::{
    CampaignListQuery, CampaignTestRequest, CreateCampaignRequest, CreateCouponRequest,
    GenerateCouponsRequest, UpdateCampaignRequest,
};
use crate::modules::ecommerce::campaign::engine::CampaignEngine;
use crate::modules::ecommerce::campaign::errors::CampaignError;
use crate::modules::ecommerce::campaign::services;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub meta: Option<PaginationMeta>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
}

async fn require_admin_access(state: &AppState, user_id: i64) -> Result<(), CampaignError> {
    if check_admin_access_api(state, user_id).await.is_err() {
        Err(CampaignError::Unauthorized)
    } else {
        Ok(())
    }
}

pub async fn create_campaign(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(req): Json<CreateCampaignRequest>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::create_campaign(&state.db, req).await {
        Ok(campaign) => {
            let response = ApiResponse {
                success: true,
                data: Some(campaign),
                error: None,
            };
            (axum::http::StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn list_campaigns(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(query): Query<CampaignListQuery>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    match services::list_campaigns(&state.db, query).await {
        Ok((campaigns, total)) => {
            let total_pages = (total + per_page - 1) / per_page;

            let response = PaginatedResponse {
                success: true,
                data: Some(campaigns),
                meta: Some(PaginationMeta {
                    page,
                    per_page,
                    total,
                    total_pages,
                }),
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn get_campaign(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::get_campaign(&state.db, id).await {
        Ok(campaign) => {
            let response = ApiResponse {
                success: true,
                data: Some(campaign),
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn update_campaign(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateCampaignRequest>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::update_campaign(&state.db, id, req).await {
        Ok(campaign) => {
            let response = ApiResponse {
                success: true,
                data: Some(campaign),
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn delete_campaign(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::delete_campaign(&state.db, id).await {
        Ok(()) => {
            let response = ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn create_coupons(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(campaign_id): Path<i64>,
    Json(req): Json<CreateCouponRequest>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::create_coupons(&state.db, campaign_id, req).await {
        Ok(coupons) => {
            let response = ApiResponse {
                success: true,
                data: Some(coupons),
                error: None,
            };
            (axum::http::StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn list_coupons(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(campaign_id): Path<i64>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::list_coupons(&state.db, campaign_id).await {
        Ok(coupons) => {
            let response = ApiResponse {
                success: true,
                data: Some(coupons),
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn generate_coupons(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(campaign_id): Path<i64>,
    Json(req): Json<GenerateCouponsRequest>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    let prefix = req.prefix.as_deref().unwrap_or("KPN").to_uppercase();
    let length = req.length.unwrap_or(8).max(4).min(16);
    let count = req.count.unwrap_or(1).min(100);
    let charset = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

    let mut codes = Vec::with_capacity(count);
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    for i in 0..count {
        seed = seed.wrapping_add((i as u64).wrapping_mul(1103515245).wrapping_add(12345));
        let mut code = prefix.clone();
        code.push('-');
        for _j in 0..length {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let idx = (seed >> 16) as usize % charset.len();
            code.push(charset[idx] as char);
        }
        codes.push(code);
    }

    match services::generate_coupons(&state.db, campaign_id, codes, &req).await {
        Ok(coupons) => {
            let response = ApiResponse {
                success: true,
                data: Some(coupons),
                error: None,
            };
            (axum::http::StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => e.into_response(),
    }
}

pub async fn delete_coupon(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(coupon_id): Path<i64>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::delete_coupon(&state.db, coupon_id).await {
        Ok(()) => {
            let response = ApiResponse::<()> {
                success: true,
                data: None,
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct UpdateCouponRequest {
    pub is_active: Option<bool>,
    pub max_usage: Option<i32>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn update_coupon(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(coupon_id): Path<i64>,
    Json(req): Json<UpdateCouponRequest>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::update_coupon(&state.db, coupon_id, req.is_active, req.max_usage, req.valid_until).await {
        Ok(coupon) => {
            let response = ApiResponse {
                success: true,
                data: Some(coupon),
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct CampaignTestQuery {
    pub cart_id: Option<i64>,
}

/// Kampanya Test (GET) - URL parametresi ile (?cart_id=123) simülasyon çalıştırır.
pub async fn test_campaign_get(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth_user: AuthenticatedUser,
    Path(campaign_id): Path<i64>,
    Query(q): Query<CampaignTestQuery>,
) -> Response {
    let cart_id = match q.cart_id {
        Some(id) => id,
        None => return Json(json!({
            "success": false, 
            "error": "Lütfen test etmek istediğiniz sepet ID'sini query string olarak ekleyin. Örn: ?cart_id=123"
        })).into_response()
    };

    // Belirli bir sepeti servis üzerinden getiriyoruz
    let cart_resp = match crate::modules::ecommerce::services::cart_service::get_cart(
        &state.db,
        cart_id,
        None,
        Some(auth_user.id),
        Some(global_ctx.display_currency.clone()),
    ).await {
        Ok(c) => c,
        Err(_) => return CampaignError::CartNotFound.into_response(),
    };

    let raw_cargo_fee = rust_decimal::Decimal::from_f64_retain(cart_resp.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(rust_decimal::Decimal::ZERO);

    test_campaign_inner(state, auth_user, campaign_id, cart_id, &global_ctx.display_currency, raw_cargo_fee).await
}

/// Kampanya Test (POST) - JSON gövdesi ile ({"cart_id": 123}) simülasyon çalıştırır.
pub async fn test_campaign_post(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth_user: AuthenticatedUser,
    Path(campaign_id): Path<i64>,
    Json(req): Json<CampaignTestRequest>,
) -> Response {
    // Belirli bir sepeti servis üzerinden getiriyoruz
    let cart_resp = match crate::modules::ecommerce::services::cart_service::get_cart(
        &state.db,
        req.cart_id,
        None,
        Some(auth_user.id),
        Some(global_ctx.display_currency.clone()),
    ).await {
        Ok(c) => c,
        Err(_) => return CampaignError::CartNotFound.into_response(),
    };

    let raw_cargo_fee = rust_decimal::Decimal::from_f64_retain(cart_resp.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(rust_decimal::Decimal::ZERO);

    test_campaign_inner(state, auth_user, campaign_id, req.cart_id, &global_ctx.display_currency, raw_cargo_fee).await
}

/// Kampanya Test İç Mantığı (Dry-Run Simülasyonu)
/// 
/// Bu işlem bir "Dry-Run" çalışmasıdır; yani veritabanında hiçbir değişiklik yapmaz.
/// Amacı, bir kampanyanın belirli bir sepet üzerindeki etkisini önceden görmektir.
/// 
/// Dönen `EvaluateResult` şunları içerir:
/// - `summary`: İndirimler uygulandığında sepetin oluşacak son hali (toplamlar, indirimler).
/// - `dry_run_report`: Kampanyanın neden uygulandığı veya neden atlandığına dair teknik rapor.
async fn test_campaign_inner(
    state: AppState,
    auth_user: AuthenticatedUser,
    campaign_id: i64,
    cart_id: i64,
    display_currency: &str,
    raw_cargo_fee: rust_decimal::Decimal,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    let _campaign = match services::get_campaign(&state.db, campaign_id).await {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };

    let engine = CampaignEngine::new(state.db.clone());

    // Admin testi için simülasyonu çalıştırıyoruz (dry_run = true)
    match engine
        .evaluate(cart_id, auth_user.id, None, true, display_currency, raw_cargo_fee)
        .await
    {
        Ok(result) => Json(json!({
            "success": true,
            "data": result
        }))
        .into_response(),
        Err(e) => {
            Json(json!({
                "success": false,
                "error": e
            }))
            .into_response()
        }
    }
}

pub async fn get_campaign_stats(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Response {
    if let Err(e) = require_admin_access(&state, auth_user.id).await {
        return e.into_response();
    }

    match services::get_campaign_stats(&state.db, id).await {
        Ok(stats) => {
            let response = ApiResponse {
                success: true,
                data: Some(stats),
                error: None,
            };
            Json(response).into_response()
        }
        Err(e) => e.into_response(),
    }
}
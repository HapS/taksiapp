use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::json;

use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::middleware::global_context::GlobalContext;
use crate::modules::ecommerce::campaign::dto::ApplyCouponRequest;
use crate::modules::ecommerce::campaign::engine::CampaignEngine;
use crate::modules::ecommerce::campaign::errors::CampaignError;
use crate::modules::ecommerce::models::cart::{self, Column as C};
use crate::modules::ecommerce::models::cart_discount::{self, Column as CD};
use crate::modules::ecommerce::models::coupon;
use crate::modules::ecommerce::models::{campaign, campaign_usage};

// #[derive(Debug, Clone, Serialize, serde::Deserialize)]
// struct ApiResponse<T: Serialize> {
//     success: bool,
//     data: Option<T>,
//     error: Option<String>,
// }

pub async fn apply_coupon(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth_user: AuthenticatedUser,
    Json(req): Json<ApplyCouponRequest>,
) -> Response {
    let db = &state.db;

    let cart_model = cart::Entity::find()
        .filter(C::UserId.eq(auth_user.id))
        .filter(C::Status.eq("open_cart"))
        .one(db)
        .await
        .ok()
        .flatten();

    let _cart_model = match cart_model {
        Some(c) => c,
        None => return CampaignError::CartNotFound.into_response(),
    };

    let code = req.code.trim().to_uppercase();

    let coupon_model = match coupon::Entity::find()
        .filter(coupon::Column::Code.eq(code.clone()))
        .one(db)
        .await
        .ok()
        .flatten()
    {
        Some(coupon) => coupon,
        None => return CampaignError::CouponNotFound.into_response(),
    };

    if !coupon_model.is_active {
        return CampaignError::CouponNotFound.into_response();
    }

    if coupon_model
        .valid_until
        .map(|valid_until| chrono::Utc::now() > valid_until)
        .unwrap_or(false)
    {
        return CampaignError::CouponExpired.into_response();
    }

    if let Some(max_usage) = coupon_model.max_usage {
        if coupon_model.usage_count >= max_usage {
            return CampaignError::CouponUsageLimitExceeded.into_response();
        }
    }

    if let Some(campaign_model) = campaign::Entity::find_by_id(coupon_model.campaign_id)
        .one(db)
        .await
        .ok()
        .flatten()
    {
        if let Some(max_uses) = campaign_model.max_uses {
            if campaign_model.usage_count >= max_uses {
                return CampaignError::CampaignUsageLimitExceeded.into_response();
            }
        }

        if let Some(max_per_user) = campaign_model.max_uses_per_user {
            let user_usage = campaign_usage::Entity::find()
                .filter(campaign_usage::Column::CampaignId.eq(campaign_model.id))
                .filter(campaign_usage::Column::UserId.eq(auth_user.id))
                .count(db)
                .await
                .unwrap_or(0);
            if user_usage >= max_per_user as u64 {
                return CampaignError::CampaignUsageLimitExceeded.into_response();
            }
        }

        let now = chrono::Utc::now();
        if !campaign_model.is_active
            || campaign_model.starts_at.map(|s| s > now).unwrap_or(false)
            || campaign_model.ends_at.map(|e| e < now).unwrap_or(false)
        {
            return CampaignError::CampaignNotActive.into_response();
        }
    }

    // Adım 2: Kuponun zaten uygulanıp uygulanmadığını kontrol etmeye gerek yok.
    // Çünkü evaluate() fonksiyonu her çağrıldığında mevcut indirimleri silip baştan hesaplar.
    // Bu kontrol hem gereksiz bir DB sorgusu yaratıyor hem de yarış durumlarına (race condition) karşı korumasız.
    // evaluate() içindeki transaction ve kilit mekanizması bu süreci güvenli hale getirecektir.

    let engine = CampaignEngine::new(db.clone());

    // Kargo ücretini almak için sepeti servis üzerinden getiriyoruz
    let cart_resp = match crate::modules::ecommerce::services::cart_service::get_active_cart(
        db,
        auth_user.id,
        None,
        Some(auth_user.id),
        Some(global_ctx.display_currency.clone()),
    )
    .await
    {
        Ok(c) => c,
        Err(_) => return CampaignError::CartNotFound.into_response(),
    };
    let raw_cargo_fee =
        rust_decimal::Decimal::from_f64_retain(cart_resp.raw_cargo_fee.unwrap_or(0.0))
            .unwrap_or(rust_decimal::Decimal::ZERO);

    match engine
        .evaluate(
            cart_resp.id,
            auth_user.id,
            Some(&code),
            false,
            &global_ctx.display_currency,
            raw_cargo_fee,
        )
        .await
    {
        Ok(result) => {
            // Check if coupon discount was actually applied
            let coupon_applied = result
                .summary
                .discounts
                .iter()
                .any(|d| d.scenario_type == "coupon_code");

            if !coupon_applied {
                return Json(json!({
                    "success": false,
                    "error": "Kupon kodu uygulandı ancak indirim sağlamadı. Koşullar sağlanmıyor olabilir."
                })).into_response();
            }

            Json(json!({
                "success": true,
                "data": result.summary
            }))
            .into_response()
        }
        Err(e) => Json(json!({
            "success": false,
            "error": e
        }))
        .into_response(),
    }
}

pub async fn remove_coupon(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth_user: AuthenticatedUser,
) -> Response {
    let db = &state.db;

    match cart::Entity::find()
        .filter(C::UserId.eq(auth_user.id))
        .filter(C::Status.eq("open_cart"))
        .one(db)
        .await
    {
        Ok(Some(cart_model)) => {
            let _ = cart_discount::Entity::delete_many()
                .filter(CD::CartId.eq(cart_model.id))
                .filter(CD::DiscountType.ne(cart_discount::discount_type::FREE_PRODUCT))
                .filter(CD::DiscountType.ne(cart_discount::discount_type::PENDING_COUPON))
                .exec(db)
                .await;

            let engine = CampaignEngine::new(db.clone());

            // Kargo ücretini almak için sepeti servis üzerinden getiriyoruz
            let cart_resp =
                match crate::modules::ecommerce::services::cart_service::get_active_cart(
                    db,
                    auth_user.id,
                    None,
                    Some(auth_user.id),
                    Some(global_ctx.display_currency.clone()),
                )
                .await
                {
                    Ok(c) => c,
                    Err(_) => return CampaignError::CartNotFound.into_response(),
                };
            let raw_cargo_fee =
                rust_decimal::Decimal::from_f64_retain(cart_resp.raw_cargo_fee.unwrap_or(0.0))
                    .unwrap_or(rust_decimal::Decimal::ZERO);

            match engine
                .evaluate(
                    cart_resp.id,
                    auth_user.id,
                    None,
                    false,
                    &global_ctx.display_currency,
                    raw_cargo_fee,
                )
                .await
            {
                Ok(result) => Json(json!({
                    "success": true,
                    "data": result.summary
                }))
                .into_response(),
                Err(e) => Json(json!({
                    "success": false,
                    "error": e
                }))
                .into_response(),
            }
        }
        _ => CampaignError::CartNotFound.into_response(),
    }
}

pub async fn cart_summary(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth_user: AuthenticatedUser,
) -> Response {
    let db = &state.db;

    // Kargo ücretini almak için sepeti servis üzerinden getiriyoruz
    let cart_resp = match crate::modules::ecommerce::services::cart_service::get_active_cart(
        db,
        auth_user.id,
        None,
        Some(auth_user.id),
        Some(global_ctx.display_currency.clone()),
    )
    .await
    {
        Ok(c) => c,
        Err(_) => return CampaignError::CartNotFound.into_response(),
    };

    let raw_cargo_fee =
        rust_decimal::Decimal::from_f64_retain(cart_resp.raw_cargo_fee.unwrap_or(0.0))
            .unwrap_or(rust_decimal::Decimal::ZERO);

    // Look up applied coupon code from existing cart_discounts
    let applied_coupon_code: Option<String> = {
        if let Some(discount) = cart_discount::Entity::find()
            .filter(CD::CartId.eq(cart_resp.id))
            .filter(CD::ScenarioType.eq("coupon_code"))
            .filter(CD::CouponId.is_not_null())
            .one(db)
            .await
            .ok()
            .flatten()
        {
            let coupon_code = if let Some(cid) = discount.coupon_id {
                coupon::Entity::find_by_id(cid)
                    .one(db)
                    .await
                    .ok()
                    .flatten()
                    .map(|c| c.code)
            } else {
                None
            };
            coupon_code
        } else {
            None
        }
    };

    let engine = CampaignEngine::new(db.clone());

    match engine
        .evaluate(
            cart_resp.id,
            auth_user.id,
            applied_coupon_code.as_deref(),
            false,
            &global_ctx.display_currency,
            raw_cargo_fee,
        )
        .await
    {
        Ok(result) => Json(json!({
            "success": true,
            "data": result.summary
        }))
        .into_response(),
        Err(e) => Json(json!({
            "success": false,
            "error": e
        }))
        .into_response(),
    }
}

pub async fn campaign_preview(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth_user: AuthenticatedUser,
) -> Response {
    let db = &state.db;

    // Kargo ücretini almak için sepeti servis üzerinden getiriyoruz
    let cart_resp = match crate::modules::ecommerce::services::cart_service::get_active_cart(
        db,
        auth_user.id,
        None,
        Some(auth_user.id),
        Some(global_ctx.display_currency.clone()),
    )
    .await
    {
        Ok(c) => c,
        Err(_) => return CampaignError::CartNotFound.into_response(),
    };

    let raw_cargo_fee =
        rust_decimal::Decimal::from_f64_retain(cart_resp.raw_cargo_fee.unwrap_or(0.0))
            .unwrap_or(rust_decimal::Decimal::ZERO);

    let engine = CampaignEngine::new(db.clone());

    match engine
        .evaluate(
            cart_resp.id,
            auth_user.id,
            None,
            true,
            &global_ctx.display_currency,
            raw_cargo_fee,
        )
        .await
    {
        Ok(result) => Json(json!({
            "success": true,
            "data": result
        }))
        .into_response(),
        Err(e) => Json(json!({
            "success": false,
            "error": e
        }))
        .into_response(),
    }
}

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::app_state::AppState;

use super::handlers::{admin, cart};

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/api/campaigns", post(admin::create_campaign))
        .route("/admin/api/campaigns", get(admin::list_campaigns))
        .route("/admin/api/campaigns/{id}", get(admin::get_campaign))
        .route("/admin/api/campaigns/{id}", put(admin::update_campaign))
        .route("/admin/api/campaigns/{id}", delete(admin::delete_campaign))
        .route(
            "/admin/api/campaigns/{id}/coupons",
            get(admin::list_coupons),
        )
        .route(
            "/admin/api/campaigns/{id}/coupons",
            post(admin::create_coupons),
        )
        .route(
            "/admin/api/campaigns/{id}/coupons/generate",
            post(admin::generate_coupons),
        )
        .route(
            "/admin/api/campaigns/{id}/test",
            get(admin::test_campaign_get).post(admin::test_campaign_post),
        )
        .route(
            "/admin/api/campaigns/{id}/stats",
            get(admin::get_campaign_stats),
        )
        .route(
            "/admin/api/coupons/{coupon_id}",
            delete(admin::delete_coupon).put(admin::update_coupon),
        )
}

pub fn cart_routes() -> Router<AppState> {
    Router::new()
        .route("/api/cart/apply-coupon", post(cart::apply_coupon))
        .route("/api/cart/remove-coupon", delete(cart::remove_coupon))
        .route("/api/cart/summary", get(cart::cart_summary))
        .route("/api/cart/campaign-preview", post(cart::campaign_preview))
}
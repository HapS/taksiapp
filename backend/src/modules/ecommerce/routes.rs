use super::controllers::{api as api_controllers, web as web_controllers};
use super::campaign;
use crate::app_state::AppState;
use axum::{
    routing::{delete, get, post, put},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(campaign::routes::admin_routes())
        .merge(campaign::routes::cart_routes())
        // Web Routes - Cart Pages
        .route("/my-cart", get(web_controllers::cart::my_cart_page))
        .route(
            "/payment/credit-card/{payment_url}",
            get(web_controllers::payment::credit_card_payment),
        )
        .route(
            "/payment/credit-card/{payment_url}",
            post(web_controllers::payment::process_credit_card_payment),
        )
        .route(
            "/payment/bank-transfer/{payment_url}",
            get(web_controllers::payment::bank_transfer_payment),
        )
        .route(
            "/payment/cash-on-delivery/{payment_url}",
            get(web_controllers::payment::cash_on_delivery_payment),
        )
        .route(
            "/payment/b2b-credit/{payment_url}",
            get(web_controllers::payment::b2b_credit_payment),
        )
        .route(
            "/payment/b2b-credit/{payment_url}",
            post(web_controllers::payment::process_b2b_credit_payment),
        )
        .route(
            "/payment/b2b-credit/success/{payment_url}",
            get(web_controllers::payment::b2b_credit_payment_success),
        )
        // .route(
        //     "/payment/pickup/{payment_url}",
        //     get(web_controllers::payment::pickup_payment),
        // )
        // Cart API Routes
        .route("/api/cart", get(api_controllers::cart::get_cart))
        .route("/api/cart", delete(api_controllers::cart::clear_cart))
        .route(
            "/api/cart/address",
            put(api_controllers::cart::update_cart_address),
        )
        .route(
            "/api/cart/payment-methods",
            get(api_controllers::cart::get_payment_methods),
        )
        .route(
            "/api/cart/payment-method",
            put(api_controllers::cart::update_cart_payment_method),
        )
        .route(
            "/api/cart/payment-start",
            post(api_controllers::cart::start_payment),
        )
        .route(
            "/api/cart/complete-order",
            post(api_controllers::cart::complete_order_not_credit_card_payment),
        )
        .route(
            "/api/cart/orders",
            get(api_controllers::cart::get_user_orders),
        )
        .route(
            "/api/cart/orders/{id}",
            get(api_controllers::cart::get_user_order),
        )
        .route(
            "/api/cart/orders/{id}/status",
            put(api_controllers::cart::update_order_status),
        )
        .route(
            "/api/cart/orders/{id}/upload-document",
            post(api_controllers::cart::upload_payment_document),
        )
        .route(
            "/api/cart/guest-info",
            get(api_controllers::cart::get_guest_info),
        )
        .route(
            "/api/cart/guest-info",
            put(api_controllers::cart::update_guest_info),
        )
        .route("/api/cart/items", post(api_controllers::cart::add_item))
        .route(
            "/api/cart/items/{id}",
            put(api_controllers::cart::update_item),
        )
        .route(
            "/api/cart/items/{id}",
            delete(api_controllers::cart::remove_item),
        )
        .route(
            "/api/cart/shipping-method",
            put(api_controllers::cart::update_cart_shipping_method),
        )
        // İptal talebi routes
        .route(
            "/api/orders/{cart_id}/items/{item_id}/cancel/preview",
            post(api_controllers::cart::preview_cancel_item),
        )
        .route(
            "/api/orders/{cart_id}/items/{item_id}/cancel",
            post(api_controllers::cart::request_cancel_item),
        )
        .route(
            "/api/orders/{cart_id}/items/{item_id}/cancel",
            delete(api_controllers::cart::cancel_cancel_request),
        )
        //cart bazında iptal talebi
        .route(
            "/api/order/cancel-request",
            post(api_controllers::cart::request_cancel_cart),
        )
        // Location API Routes
        .route(
            "/api/location/countries",
            get(api_controllers::location::get_countries),
        )
        .route(
            "/api/location/cities/{country_id}",
            get(api_controllers::location::get_cities),
        )
        .route(
            "/api/location/districts/{city_id}",
            get(api_controllers::location::get_districts),
        )
        .route(
            "/api/shipping-methods",
            get(api_controllers::kargo::get_shipping_providers),
        )
        // İade talebi routes (müşteri tarafı)
        .route(
            "/api/orders/{cart_id}/items/{item_id}/return",
            post(api_controllers::returns::create_return_request),
        )
        .route(
            "/api/returns",
            get(api_controllers::returns::list_return_requests),
        )
        .route(
            "/api/returns/{return_id}",
            get(api_controllers::returns::get_return_request),
        )
        .route(
            "/api/returns/{return_id}",
            delete(api_controllers::returns::cancel_return_request),
        )
        .route(
            "/api/returns/{return_id}/cargo",
            put(api_controllers::returns::update_return_cargo),
        )
        // Currency API Routes (frontend para birimi dropdown)
        .route(
            "/api/currencies",
            get(api_controllers::currency::list_currencies),
        )
        .route(
            "/api/currencies/current",
            get(api_controllers::currency::get_current_currency),
        )
        .route(
            "/api/currencies/current",
            put(api_controllers::currency::set_currency),
        )
}

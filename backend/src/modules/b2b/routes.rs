use super::controllers::admin;
use super::controllers::api as api_controllers;
use super::controllers::web as web_controllers;
use crate::app_state::AppState;
use axum::{
    routing::{delete, get, post, put},
    Router,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        // Admin HTML Routes (Sayfa render)
        .route(
            "/admin/b2b/companies",
            get(admin::html::company_html::company_list_page),
        )
        .route(
            "/admin/b2b/companies/new",
            get(admin::html::company_html::company_add_page),
        )
        .route(
            "/admin/b2b/companies/{id}",
            get(admin::html::company_html::company_edit_page),
        )
        // Admin API Routes (JSON endpoints)
        .route(
            "/admin/api/b2b/companies",
            get(admin::api::company_api::list_companies),
        )
        .route(
            "/admin/api/b2b/companies",
            post(admin::api::company_api::create_company),
        )
        .route(
            "/admin/api/b2b/companies/{id}",
            get(admin::api::company_api::get_company),
        )
        .route(
            "/admin/api/b2b/companies/{id}",
            put(admin::api::company_api::update_company),
        )
        .route(
            "/admin/api/b2b/companies/{id}",
            delete(admin::api::company_api::delete_company),
        )
        .route(
            "/admin/api/b2b/companies/{id}/admin",
            put(admin::api::company_api::admin_update_company),
        )
        .route(
            "/admin/api/b2b/companies/{id}/approve",
            post(admin::api::company_api::approve_company),
        )
        .route(
            "/admin/api/b2b/companies/{id}/toggle-active",
            post(admin::api::company_api::toggle_active),
        )
        // Representative Management API Routes
        .route(
            "/admin/api/b2b/companies/{company_id}/representatives",
            get(admin::api::representative_api::list_company_representatives),
        )
        .route(
            "/admin/api/b2b/representatives",
            post(admin::api::representative_api::create_representative),
        )
        .route(
            "/admin/api/b2b/representatives/{id}",
            put(admin::api::representative_api::update_representative),
        )
        .route(
            "/admin/api/b2b/representatives/{id}",
            delete(admin::api::representative_api::delete_representative),
        )
        // Web Routes - B2B Pages
        .route("/my-account/b2b", get(web_controllers::dashboard::b2b_home))
        // User B2B Credit API Routes
        .route(
            "/api/b2b/credit/me",
            get(api_controllers::credit::get_my_company_credit_summary),
        )
        .route(
            "/api/b2b/credit/transactions",
            get(api_controllers::credit::get_my_credit_transactions),
        )
    // .route(
    //     "/{lang}/b2b/my-cart",
    //     get(web_controllers::orders::b2b_cart_html),
    // )
    // .route(
    //     "/{lang}/b2b/my-orders",
    //     get(web_controllers::orders::b2b_orders_html),
    // )
    // .route(
    //     "/{lang}/b2b/products",
    //     get(web_controllers::products::product_list),
    // )
    // .route(
    //     "/{lang}/b2b/product/{slug_id}",
    //     get(web_controllers::products::product_detail),
    // )
    // .route(
    //     "/{lang}/b2b/products/category/{slug_id}",
    //     get(web_controllers::products::product_list_category),
    // )
    // // B2B API Routes
    // .route(
    //     "/b2b/api/my-company",
    //     get(api_controllers::company::get_my_company),
    // )
    // .route(
    //     "/b2b/api/products",
    //     get(api_controllers::products::product_list),
    // )
    // .route(
    //     "/b2b/api/products/{id}",
    //     get(api_controllers::products::get_by_id),
    // )
    // .route(
    //     "/b2b/api/products/categories",
    //     get(api_controllers::products::get_categories),
    // )
    // .route(
    //     "/b2b/api/categories/{category_id}/attributes",
    //     get(api_controllers::products::get_category_attributes),
    // )
    // .route(
    //     "/b2b/api/categories_list",
    //     get(api_controllers::products::product_categories_list),
    // )
}

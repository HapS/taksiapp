use super::controllers::{api as api_controllers, web as web_controllers};
use crate::app_state::AppState;
use axum::{routing::get, Router};

// Content app URL patterns - Axum router
pub fn routes() -> Router<AppState> {
    Router::new()
        // Public Page API Routes
        .route("/api/pages", get(api_controllers::pages::list))
        .route("/api/pages/{id}", get(api_controllers::pages::get_by_id))
        .route(
            "/api/products/{id}",
            get(api_controllers::products::get_by_id),
        )
        .route(
            "/api/pages/slug/{slug}",
            get(api_controllers::pages::get_by_slug),
        )
        .route(
            "/api/products",
            get(api_controllers::products::product_list),
        )
        .route(
            "/api/products/categories",
            get(api_controllers::products::get_categories),
        )
        // .route(
        //     "/api/debug/products",
        //     post(api_controllers::products::debug_create_product),
        // )
        // .route(
        //     "/api/debug/products/media",
        //     post(api_controllers::products::debug_create_product_multipart),
        // )
        .route(
            "/api/categories/{category_id}/attributes",
            get(api_controllers::products::get_category_attributes),
        )
        // Language switching routes
        .route(
            "/set-language/{lang_code}",
            get(web_controllers::language::set_language),
        )
        .route(
            "/set-language/{lang_code}/{redirect_path}",
            get(web_controllers::language::set_language_with_redirect),
        )
        // Frontend HTML Views (Language-based)
        .route("/{lang}", get(web_controllers::home::index))
        // .route("/{lang}", get(web_controllers::products::product_list))
        .route("/{lang}/pages", get(web_controllers::pages::list))
        .route("/{lang}/news", get(web_controllers::pages::list))
        .route("/{lang}/blog", get(web_controllers::pages::list))
        .route(
            "/{lang}/products",
            get(web_controllers::products::product_list),
        )
        .route(
            "/{lang}/page/{slug_id}",
            get(web_controllers::pages::detail),
        )
        .route(
            "/{lang}/news/{slug_id}",
            get(web_controllers::pages::detail),
        )
        .route(
            "/{lang}/blog/{slug_id}",
            get(web_controllers::pages::detail),
        )
        .route(
            "/{lang}/product/{slug_id}",
            get(web_controllers::products::product_detail),
        )
        .route(
            "/{lang}/products/category/{slug_id}",
            get(web_controllers::products::product_list_category),
        )
}

// Web Controllers - Pages list and detail
use crate::app_state::AppState;
use crate::config;
use crate::middleware::global_context::{GlobalContext, ViewContext};
use crate::modules::content::helpers::language_helper::{validate_language, LanguageValidation};
use crate::modules::content::services::product_service;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    Extension,
};
use rust_i18n::t;

/// List all published pages
pub async fn product_list(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    mut ctx: ViewContext,
    Path(language): Path<String>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let config = config::get_config();

    // Validate the language parameter
    let lang = match validate_language(&language, &config) {
        LanguageValidation::Valid(lang) => lang,
        LanguageValidation::ReservedPath => {
            return StatusCode::NOT_FOUND.into_response();
        }
        LanguageValidation::Unsupported { redirect_to } => {
            return Redirect::to(&redirect_to).into_response();
        }
    };

    let sort = query.get("sort").cloned();

    // Pagination parameters
    let page: usize = query
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(1)
        .max(1);
    let per_page: usize = query
        .get("per_page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(24)
        .max(1)
        .min(100); // Max 100 items per page

    // Kullanıcının seçtiği display_currency'yi global context'ten al
    let display_currency = global_ctx.display_currency.clone();

    let (products, total_count) = match product_service::list_products_paginated(
        &state.db,
        &lang,
        sort.clone(),
        Some(&display_currency),
        page,
        per_page,
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch products",
            )
                .into_response();
        }
    };

    let products_categories = product_service::get_producs_all_categories(&state.db, &lang, None)
        .await
        .unwrap_or_default();
    ctx.0.insert("product_categories", &products_categories);

    // Pagination calculations
    let total_pages = (total_count as f64 / per_page as f64).ceil() as usize;
    let has_next_page = page < total_pages;
    let has_prev_page = page > 1;

    ctx.0.insert("title", &t!("product_list", locale = lang));
    ctx.0.insert("products", &products);
    ctx.0.insert("current_sort", &sort.unwrap_or_default());
    ctx.0.insert("request_path", &format!("/{}/products", lang));

    // Pagination context
    ctx.0.insert("current_page", &page);
    ctx.0.insert("per_page", &per_page);
    ctx.0.insert("total_count", &total_count);
    ctx.0.insert("total_pages", &total_pages);
    ctx.0.insert("has_next_page", &has_next_page);
    ctx.0.insert("has_prev_page", &has_prev_page);

    //json true ise json döndür
    if query.get("json").map(|v| v == "true").unwrap_or(false) {
        // Context'i serialize edilebilir bir map'e çevir (BTreeMap kullanarak alfabetik sıralama sağlıyoruz)
        let json_data: std::collections::BTreeMap<String, serde_json::Value> = ctx
            .0
            .into_json()
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        return axum::Json(json_data).into_response();
    }

    match state.render_frontend_template("pages/product_list.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            let prefer_json = query.get("json").map(|v| v == "true").unwrap_or(false);
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                prefer_json,
                Some(&state),
            );
        }
    }
}

pub async fn product_list_category(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    mut ctx: ViewContext,
    Path((language, slug_id)): Path<(String, String)>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let config = config::get_config();

    // Validate the language parameter
    let lang = match validate_language(&language, &config) {
        LanguageValidation::Valid(lang) => lang,
        LanguageValidation::ReservedPath => {
            return StatusCode::NOT_FOUND.into_response();
        }
        LanguageValidation::Unsupported { redirect_to } => {
            return Redirect::to(&redirect_to).into_response();
        }
    };

    let (_slug, id) = match slug_id.rfind('-') {
        Some(pos) => {
            let (slug_part, id_part) = slug_id.split_at(pos);
            let id_str = &id_part[1..]; // Skip the dash
            match id_str.parse::<i64>() {
                Ok(id) => (slug_part.to_string(), id),
                Err(_) => {
                    return (StatusCode::BAD_REQUEST, "Invalid page ID format").into_response();
                }
            }
        }
        None => {
            return (StatusCode::BAD_REQUEST, "Invalid slug-id format").into_response();
        }
    };

    // Extract attribute filters from query parameters
    let attribute_filters: std::collections::HashMap<String, Vec<i64>> = query
        .iter()
        .filter_map(|(key, value)| {
            // Skip non-attribute parameters
            if key == "page" || key == "limit" {
                return None;
            }

            // Parse comma-separated term IDs
            let term_ids: Vec<i64> = value
                .split(',')
                .filter_map(|id_str| id_str.parse::<i64>().ok())
                .collect();

            if !term_ids.is_empty() {
                Some((key.clone(), term_ids))
            } else {
                None
            }
        })
        .collect();

    let sort = query.get("sort").cloned();

    // Pagination parameters
    let page: usize = query
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(1)
        .max(1);
    let per_page: usize = query
        .get("per_page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(24)
        .max(1)
        .min(100); // Max 100 items per page

    // Kullanıcının seçtiği display_currency'yi global context'ten al
    let display_currency = global_ctx.display_currency.clone();

    let (products, total_count) = match product_service::list_products_category_with_filters_paginated(
        &state.db,
        &lang,
        Some(id),
        attribute_filters,
        sort.clone(),
        Some(&display_currency),
        page,
        per_page,
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch products",
            )
                .into_response();
        }
    };

    let products_categories =
        product_service::get_producs_all_categories(&state.db, &lang, Some(id))
            .await
            .unwrap_or_default();
    ctx.0.insert("product_categories", &products_categories);

    let breadcrumb = product_service::get_producs_breadcrumb(&state.db, &lang, Some(id))
        .await
        .unwrap_or_default();
    ctx.0.insert("breadcrumb", &breadcrumb);

    // Pagination calculations
    let total_pages = (total_count as f64 / per_page as f64).ceil() as usize;
    let has_next_page = page < total_pages;
    let has_prev_page = page > 1;

    ctx.0.insert("title", &t!("product_list", locale = lang));
    ctx.0.insert("products", &products);
    ctx.0.insert("current_sort", &sort.unwrap_or_default());
    ctx.0.insert("request_path", &format!("/{}/products", lang));

    // Pagination context
    ctx.0.insert("current_page", &page);
    ctx.0.insert("per_page", &per_page);
    ctx.0.insert("total_count", &total_count);
    ctx.0.insert("total_pages", &total_pages);
    ctx.0.insert("has_next_page", &has_next_page);
    ctx.0.insert("has_prev_page", &has_prev_page);

    if query.get("json").map(|v| v == "true").unwrap_or(false) {
        // Context'i serialize edilebilir bir map'e çevir (BTreeMap kullanarak alfabetik sıralama sağlıyoruz)
        let json_data: std::collections::BTreeMap<String, serde_json::Value> = ctx
            .0
            .into_json()
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        return axum::Json(json_data).into_response();
    }

    match state.render_frontend_template("pages/product_list.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            let prefer_json = query.get("json").map(|v| v == "true").unwrap_or(false);
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                prefer_json,
                Some(&state),
            );
        }
    }
}

/// Product detail by slug-id format (e.g., "my-product-123")
pub async fn product_detail(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    mut ctx: ViewContext,
    Path((language, slug_id)): Path<(String, String)>,
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let config = config::get_config();

    // Validate the language parameter
    let lang = match validate_language(&language, &config) {
        LanguageValidation::Valid(lang) => lang,
        LanguageValidation::ReservedPath => {
            return StatusCode::NOT_FOUND.into_response();
        }
        LanguageValidation::Unsupported { redirect_to } => {
            return Redirect::to(&redirect_to).into_response();
        }
    };

    // Parse slug-id format: "my-article-123" -> slug="my-article", id=123
    // Find the last dash and split there
    let (slug, id) = match slug_id.rfind('-') {
        Some(pos) => {
            let (slug_part, id_part) = slug_id.split_at(pos);
            let id_str = &id_part[1..]; // Skip the dash
            match id_str.parse::<i64>() {
                Ok(id) => (slug_part.to_string(), id),
                Err(_) => {
                    return (StatusCode::BAD_REQUEST, "Invalid page ID format").into_response();
                }
            }
        }
        None => {
            return (StatusCode::BAD_REQUEST, "Invalid slug-id format").into_response();
        }
    };

    // Kullanıcının seçtiği display_currency'yi global context'ten al
    let display_currency = global_ctx.display_currency.clone();

    let product = match product_service::get_product(
        &state.db,
        &lang,
        Some(&slug),
        Some(id),
        Some(&display_currency),
    )
    .await
    {
        Ok(p) => p,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Product not found").into_response();
        }
    };

    // Get term_master_id from product.data and build breadcrumb
    let term_master_id = product.data.get("term_master_id").and_then(|v| v.as_i64());

    let breadcrumb = if let Some(category_id) = term_master_id {
        product_service::get_producs_breadcrumb(&state.db, &lang, Some(category_id))
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    ctx.0.insert("breadcrumb", &breadcrumb);

    // Check if slug matches the actual product slug
    if !product.slug.is_empty() && product.slug != slug {
        // Redirect to correct slug
        let correct_url = format!("/{}/product/{}-{}", lang, product.slug, id);
        return Redirect::permanent(&correct_url).into_response();
    }

    ctx.0.insert("title", &product.title);
    ctx.0.insert("result", &product);
    ctx.0.insert(
        "request_path",
        &format!("/{}/product/{}-{}", lang, slug, id),
    );

    // ?json=true  Json response  tera ile aynı çıktığı verir  context i doğrudan json yapıyoruz
    if query.get("json").map(|v| v == "true").unwrap_or(false) {
        // Context'i serialize edilebilir bir map'e çevir
        let json_data: std::collections::BTreeMap<String, serde_json::Value> = ctx
            .0
            .into_json()
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        return axum::Json(json_data).into_response();
    }

    // Admin panelden seçilen template'i kullan, yoksa default
    let template_path = if let Some(template) = &product.template {
        if !template.is_empty() {
            format!("pages/{}", template)
        } else {
            "pages/product_detail.html".to_string()
        }
    } else {
        // Default template
        "pages/product_detail.html".to_string()
    };

    match state.render_frontend_template(&template_path, &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Template bulunamazsa fallback template kullan
            if config.is_debug() {
                eprintln!("❌ Template error: {}", e);
                eprintln!("❌ Tried template: {}", template_path);
                eprintln!("💡 Falling back to default template");
            }

            // Fallback to default template
            match state.render_frontend_template("pages/product_detail.html", &ctx.0) {
                Ok(html) => Html(html).into_response(),
                Err(e) => {
                    let prefer_json = query.get("json").map(|v| v == "true").unwrap_or(false);
                    return crate::middleware::error_handler::handle_template_error_with_context(
                        &e,
                        config.is_debug(),
                        prefer_json,
                        Some(&state),
                    );
                }
            }
        }
    }
}

// Page Service - Business logic for page operations
// use crate::modules::content::helpers::product_helper::*;
use crate::modules::content::models::content::Column as ContentColumn;
use crate::modules::content::models::Content;
use crate::modules::content::models::{content_terms, ContentTerm};
use crate::modules::taxonomy::models::{term, Term};
use crate::modules::utils::terms_utils::NestedTerm;
use crate::modules::utils::terms_utils::{build_term_hierarchy, build_term_hierarchy_breadcrumb};
use sea_orm::*;

#[derive(Debug)]
pub enum ServiceError {
    NotFound,
    #[allow(dead_code)]
    DatabaseError(DbErr),
}

impl From<DbErr> for ServiceError {
    fn from(err: DbErr) -> Self {
        ServiceError::DatabaseError(err)
    }
}

pub async fn get_product(
    db: &DatabaseConnection,
    lang: &str,
    slug: Option<&str>,
    id: Option<i64>,
    display_currency: Option<&str>,
) -> Result<crate::modules::b2b::helpers::product_helper::ProductResponse, ServiceError> {
    let filter = Content::find()
        .filter(ContentColumn::Id.eq(id.unwrap_or(0)))
        .filter(ContentColumn::ContentType.eq("product"))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null());

    let model = match (id, slug) {
        (Some(id), _) => filter.filter(ContentColumn::Id.eq(id)).one(db).await?,
        (_, Some(slug)) => filter.all(db).await?.into_iter().find(|p| {
            let page_slug = crate::modules::b2b::helpers::product_helper::get_string_from_json(
                &p.data, lang, "slug",
            )
            .unwrap_or_default();
            page_slug == slug
        }),
        _ => None,
    }
    .ok_or(ServiceError::NotFound)?;

    Ok(
        crate::modules::b2b::helpers::product_helper::to_product_get_response(
            &model,
            lang,
            db,
            display_currency,
        )
        .await,
    )
}

pub async fn list_products(
    db: &DatabaseConnection,
    lang: &str,
    sort_by: Option<String>,
    display_currency: Option<&str>,
) -> Result<Vec<crate::modules::b2b::helpers::product_helper::ProductResponse>, ServiceError> {
    let items = Content::find()
        .filter(ContentColumn::ContentType.eq("product"))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null())
        .order_by_asc(ContentColumn::OrderId)
        .all(db)
        .await?;

    // Pre-fetch all tags for all pages in a single batch query (N+1 fix)
    let content_ids: Vec<i64> = items.iter().map(|p| p.id).collect();
    let tags_map = if !content_ids.is_empty() {
        crate::modules::b2b::helpers::product_helper::fetch_tags_for_contents(
            db,
            &content_ids,
            lang,
        )
        .await
        .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    // Get sale currency and exchange rates for price conversion
    let sale_currency = crate::modules::admin::services::settings_service::get_sale_currency(db)
        .await
        .unwrap_or_else(|| "TRY".to_string());
    let target_currency = display_currency.unwrap_or(&sale_currency);
    let rates =
        crate::modules::currency::services::exchange_rate_service::get_cached_rates(db).await;

    // Tüm ürünlerde kullanılan öznitelik term ID'lerini önceden getir (N+1 sorgu sorununu önlemek için)
    let mut attr_ids: Vec<i64> = Vec::new();
    for p in items.iter() {
        if let Some(prod) = p.data.get("product") {
            if let Some(attrs) = prod.get("attributes").and_then(|a| a.as_object()) {
                for (_k, v) in attrs {
                    if let Some(arr) = v.as_array() {
                        for idv in arr {
                            if let Some(id) = idv.as_i64() {
                                attr_ids.push(id);
                            }
                        }
                    }
                }
            }
        }
    }
    attr_ids.sort_unstable();
    attr_ids.dedup();

    let attribute_terms_map = if !attr_ids.is_empty() {
        crate::modules::b2b::helpers::product_helper::fetch_terms_by_ids(db, &attr_ids, lang)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let mut results = Vec::new();
    for product in items.iter() {
        if crate::modules::b2b::helpers::product_helper::has_content_in_language(
            &product.data,
            lang,
        ) {
            results.push(
                crate::modules::b2b::helpers::product_helper::to_product_list_response(
                    db,
                    product,
                    lang,
                    &tags_map,
                    target_currency,
                    rates.as_ref(),
                    &attribute_terms_map,
                )
                .await,
            );
        }
    }

    // Apply sorting
    if let Some(sort) = sort_by {
        match sort.as_str() {
            "price_asc" => results.sort_by(|a, b| {
                a.display_price
                    .partial_cmp(&b.display_price)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "price_desc" => results.sort_by(|a, b| {
                b.display_price
                    .partial_cmp(&a.display_price)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "newest" => results.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
            "title_asc" => results.sort_by(|a, b| a.title.cmp(&b.title)),
            "title_desc" => results.sort_by(|a, b| b.title.cmp(&a.title)),
            _ => {}
        }
    }

    Ok(results)
}

/// Original function continues...
pub async fn list_products_category_with_filters(
    db: &sea_orm::DatabaseConnection,
    lang: &str,
    category_id: Option<i64>,
    attribute_filters: std::collections::HashMap<String, Vec<i64>>,
    sort_by: Option<String>,
    display_currency: Option<&str>,
) -> Result<Vec<crate::modules::b2b::helpers::product_helper::ProductResponse>, ServiceError> {
    println!("Category filter: {}", category_id.unwrap_or(-1));
    println!("Attribute filters: {:?}", attribute_filters);

    // First get products by category (same as original function)
    let product_category_terms = if let Some(cat_id) = category_id {
        ContentTerm::find()
            .filter(content_terms::Column::TermId.eq(cat_id))
            .filter(content_terms::Column::ContentType.eq("product"))
            .all(db)
            .await?
    } else {
        Vec::new()
    };

    let mut content_ids: Vec<i64> = product_category_terms
        .iter()
        .map(|pct| pct.content_id)
        .collect();

    // Apply attribute filters using raw SQL for JSONB queries
    if !attribute_filters.is_empty() {
        println!("Applying attribute filters: {:?}", attribute_filters);

        use sea_orm::{ConnectionTrait, Statement};

        // Build the SQL query with JSONB conditions
        let mut sql_conditions = Vec::new();

        // Base conditions
        sql_conditions.push("content_type = 'product'".to_string());
        sql_conditions.push("publish = true".to_string());
        sql_conditions.push("deleted_at IS NULL".to_string());

        // Add category filter if we have content_ids
        if !content_ids.is_empty() {
            let ids_str = content_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            sql_conditions.push(format!("id IN ({})", ids_str));
        }

        // Add JSONB attribute filters
        for (vocabulary_name, term_ids) in attribute_filters.iter() {
            // For each vocabulary, create OR conditions for term_ids
            let mut vocab_conditions = Vec::new();

            for term_id in term_ids {
                // Check if the JSONB array contains the term_id
                // data->'product'->'attributes'->'vocabulary_name' @> '[term_id]'
                vocab_conditions.push(format!(
                    "data->'product'->'attributes'->'{}' @> '[{}]'",
                    vocabulary_name, term_id
                ));
            }

            if !vocab_conditions.is_empty() {
                sql_conditions.push(format!("({})", vocab_conditions.join(" OR ")));
            }
        }

        let sql = format!(
            "SELECT id FROM contents WHERE {}",
            sql_conditions.join(" AND ")
        );

        println!("Executing SQL: {}", sql);

        // Execute raw SQL query
        let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        let query_result = db.query_all(statement).await?;

        // Extract IDs from result
        content_ids = query_result
            .iter()
            .filter_map(|row| row.try_get::<i64>("", "id").ok())
            .collect();

        println!("Found {} products after JSONB filtering", content_ids.len());
    }

    let items = if !content_ids.is_empty() {
        Content::find()
            .filter(ContentColumn::ContentType.eq("product"))
            .filter(ContentColumn::Publish.eq(true))
            .filter(ContentColumn::DeletedAt.is_null())
            .filter(ContentColumn::Id.is_in(content_ids.clone()))
            .order_by_asc(ContentColumn::OrderId)
            .all(db)
            .await?
    } else {
        Vec::new()
    };

    // Pre-fetch all tags for all pages in a single batch query (N+1 fix)
    let tags_map = if !content_ids.is_empty() {
        crate::modules::b2b::helpers::product_helper::fetch_tags_for_contents(
            db,
            &content_ids,
            lang,
        )
        .await
        .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    // Get sale currency and exchange rates for price conversion
    let sale_currency = crate::modules::admin::services::settings_service::get_sale_currency(db)
        .await
        .unwrap_or_else(|| "TRY".to_string());
    let target_currency = display_currency.unwrap_or(&sale_currency);
    let rates =
        crate::modules::currency::services::exchange_rate_service::get_cached_rates(db).await;

    // Filtrelenmiş ürünlerde kullanılan öznitelik term ID'lerini önceden getir (N+1 sorgu sorununu önlemek için)
    let mut attr_ids: Vec<i64> = Vec::new();
    for p in items.iter() {
        if let Some(prod) = p.data.get("product") {
            if let Some(attrs) = prod.get("attributes").and_then(|a| a.as_object()) {
                for (_k, v) in attrs {
                    if let Some(arr) = v.as_array() {
                        for idv in arr {
                            if let Some(id) = idv.as_i64() {
                                attr_ids.push(id);
                            }
                        }
                    }
                }
            }
        }
    }

    attr_ids.sort_unstable();
    attr_ids.dedup();

    let attribute_terms_map = if !attr_ids.is_empty() {
        crate::modules::b2b::helpers::product_helper::fetch_terms_by_ids(db, &attr_ids, lang)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let mut results = Vec::new();
    for product in items.iter() {
        if crate::modules::b2b::helpers::product_helper::has_content_in_language(
            &product.data,
            lang,
        ) {
            results.push(
                crate::modules::b2b::helpers::product_helper::to_product_list_response(
                    db,
                    product,
                    lang,
                    &tags_map,
                    target_currency,
                    rates.as_ref(),
                    &attribute_terms_map,
                )
                .await,
            );
        }
    }

    // Apply sorting
    if let Some(sort) = sort_by {
        match sort.as_str() {
            "price_asc" => results.sort_by(|a, b| {
                a.display_price
                    .partial_cmp(&b.display_price)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "price_desc" => results.sort_by(|a, b| {
                b.display_price
                    .partial_cmp(&a.display_price)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "newest" => results.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
            "title_asc" => results.sort_by(|a, b| a.title.cmp(&b.title)),
            "title_desc" => results.sort_by(|a, b| b.title.cmp(&a.title)),
            _ => {}
        }
    }

    Ok(results)
}

// get_categories_for_product
pub async fn get_producs_breadcrumb(
    db: &DatabaseConnection,
    lang: &str,
    parent_id: Option<i64>,
) -> Result<Vec<NestedTerm>, ServiceError> {
    // Settings'ten ürün kategorileri vocabulary ID'sini al
    let vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(db, "product_categories")
            .await
            .unwrap_or(1); // Fallback olarak 1 kullan

    let all_terms = match Term::find()
        .filter(term::Column::VocabularyId.eq(vocab_id))
        .filter(term::Column::Publish.eq(true))
        .all(db)
        .await
    {
        Ok(terms) => terms,
        Err(e) => return Err(ServiceError::DatabaseError(e)),
    };

    let term_hierarchy = build_term_hierarchy_breadcrumb(&all_terms, lang, parent_id);
    Ok(term_hierarchy)
}

// get_categories_for_product
pub async fn get_producs_all_categories(
    db: &DatabaseConnection,
    lang: &str,
    parent_id: Option<i64>,
) -> Result<Vec<NestedTerm>, ServiceError> {
    // Settings'ten ürün kategorileri vocabulary ID'sini al
    let vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(db, "product_categories")
            .await
            .unwrap_or(1); // Fallback olarak 1 kullan

    let all_terms = match Term::find()
        .filter(term::Column::VocabularyId.eq(vocab_id))
        .filter(term::Column::Publish.eq(true))
        .order_by_asc(term::Column::OrderId)
        .all(db)
        .await
    {
        Ok(terms) => terms,
        Err(e) => return Err(ServiceError::DatabaseError(e)),
    };

    let term_hierarchy = build_term_hierarchy(&all_terms, lang, parent_id);
    Ok(term_hierarchy)
}

// Page Helper - JSON parsing utilities
use crate::config::get_config;
use serde::Serialize;
use serde_json::Value;
// use tera::{Function as TeraFunction, Result as TeraResult};
// use std::collections::HashMap;
// use std::sync::Arc;
// use sea_orm::{DatabaseConnection};

/// Tag for frontend display
#[derive(Serialize, Clone)]
pub struct TagInfo {
    pub id: i64,
    pub title: String,
    pub slug: String,
}

/// ProductResponse for frontend templates
#[derive(Serialize, Clone)]
pub struct ProductResponse {
    pub id: i64,
    pub data: Value,
    pub product: Option<Value>,
    pub publish: bool,
    pub language: String,

    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub body: String,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
    pub template: Option<String>,
    pub available_languages: Vec<String>,
    pub tags: Vec<TagInfo>,
    pub sub_contents: std::collections::HashMap<String, serde_json::Value>,

    pub get_absolute_url: Option<String>, // Absolute URL for links

    // Price fields for easy template access (original currency)
    pub price: Option<f64>,
    pub old_price: Option<f64>,
    pub discount_percentage: Option<f64>,
    pub currency: Option<String>, // Product's original currency (USD, EUR, TRY, etc.)

    // Display prices (converted to sale currency)
    pub display_price: Option<f64>,
    pub display_old_price: Option<f64>,
    pub display_currency: Option<String>, // Sale currency (from settings)

    // Formatted prices (ready for display)
    pub price_formatted: Option<String>,
    pub old_price_formatted: Option<String>,

    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Extract string from JSON data based on language and field
/// Supports formats: data.langs.tr.field OR data.tr.field
pub fn get_string_from_json(data: &Value, lang: &str, field: &str) -> Option<String> {
    // Try data.langs.tr.field format
    if let Some(langs) = data.get("langs") {
        if let Some(lang_data) = langs.get(lang) {
            if let Some(value) = lang_data.get(field) {
                return value.as_str().map(|s| s.to_string());
            }
        }
    }

    // Try data.tr.field format (alternative)
    if let Some(lang_data) = data.get(lang) {
        if let Some(value) = lang_data.get(field) {
            return value.as_str().map(|s| s.to_string());
        }
    }

    None
}

/// Get available languages from JSON data
#[allow(dead_code)]
pub fn get_available_languages(data: &Value) -> Vec<String> {
    let config = get_config();

    // Try data.langs format
    if let Some(langs) = data.get("langs") {
        if let Some(obj) = langs.as_object() {
            return obj.keys().cloned().collect();
        }
    }

    // Alternative: check root keys against supported languages
    if let Some(obj) = data.as_object() {
        return obj
            .keys()
            .filter(|k| config.supported_languages.contains_key(*k))
            .cloned()
            .collect();
    }

    vec![]
}

/// Check if content exists in given language
pub fn has_content_in_language(data: &Value, language: &str) -> bool {
    let title =
        get_string_from_json(data, language, "title").unwrap_or_else(|| "Başlıksız".to_string());
    !title.is_empty() && title != "Başlıksız"
}

/// Convert prices for batch operations (uses pre-fetched rates)
pub fn convert_prices_with_rates(
    price: Option<f64>,
    old_price: Option<f64>,
    product_currency: Option<&str>,
    sale_currency: &str,
    rates: Option<&crate::modules::currency::models::ExchangeRateModel>,
) -> (Option<f64>, Option<f64>, Option<String>) {
    use crate::modules::currency::services::exchange_rate_service;

    let product_curr = product_currency.unwrap_or("TRY");

    // If same currency, no conversion needed
    if product_curr.to_uppercase() == sale_currency.to_uppercase() {
        return (price, old_price, Some(sale_currency.to_string()));
    }

    // If no rates, return original
    let rates = match rates {
        Some(r) => r,
        None => return (price, old_price, Some(product_curr.to_string())),
    };

    // Convert prices
    let display_price = price.and_then(|p| {
        exchange_rate_service::convert_currency(p, product_curr, sale_currency, rates)
    });

    let display_old_price = old_price.and_then(|p| {
        exchange_rate_service::convert_currency(p, product_curr, sale_currency, rates)
    });

    (
        display_price,
        display_old_price,
        Some(sale_currency.to_string()),
    )
}

/// Convert product data with variants - adds display_price and display_old_price to each variant
pub fn convert_product_with_variants(
    product_data: Option<&Value>,
    product_currency: Option<&str>,
    sale_currency: &str,
    rates: Option<&crate::modules::currency::models::ExchangeRateModel>,
) -> Option<Value> {
    use crate::modules::currency::services::exchange_rate_service;
    use crate::modules::utils::format_price::format_price;

    let product = product_data?;
    let mut product_clone = product.clone();

    let product_curr = product_currency.unwrap_or("TRY");

    // Convert variants if they exist
    if let Some(variants) = product_clone
        .get_mut("variants")
        .and_then(|v| v.as_array_mut())
    {
        for variant in variants.iter_mut() {
            if let Some(variant_obj) = variant.as_object_mut() {
                // Get original price (what merchant entered)
                let original_price = variant_obj
                    .get("price")
                    .and_then(|p| p.as_f64().or_else(|| p.as_i64().map(|i| i as f64)));

                // Calculate discounted price (what customer pays)
                let discount_percentage = variant_obj
                    .get("discount_percentage")
                    .and_then(|d| d.as_f64());
                let current_price = calculate_discounted_price(original_price, discount_percentage);

                // Convert prices
                if product_curr.to_uppercase() == sale_currency.to_uppercase() {
                    // Same currency, just copy
                    if let Some(p) = current_price {
                        variant_obj.insert(
                            "price_formatted".to_string(),
                            serde_json::json!(format_price(p, product_curr)),
                        );
                        variant_obj.insert("display_price".to_string(), serde_json::json!(p));
                        variant_obj.insert(
                            "display_price_formatted".to_string(),
                            serde_json::json!(format_price(p, sale_currency)),
                        );
                    }
                    if let Some(op) = original_price {
                        variant_obj.insert(
                            "old_price_formatted".to_string(),
                            serde_json::json!(format_price(op, product_curr)),
                        );
                        variant_obj.insert("display_old_price".to_string(), serde_json::json!(op));
                        variant_obj.insert(
                            "display_old_price_formatted".to_string(),
                            serde_json::json!(format_price(op, sale_currency)),
                        );
                    }
                } else if let Some(r) = rates {
                    // Different currency, convert
                    if let Some(p) = current_price {
                        variant_obj.insert(
                            "price_formatted".to_string(),
                            serde_json::json!(format_price(p, product_curr)),
                        );
                        if let Some(converted) = exchange_rate_service::convert_currency(
                            p,
                            product_curr,
                            sale_currency,
                            r,
                        ) {
                            variant_obj
                                .insert("display_price".to_string(), serde_json::json!(converted));
                            variant_obj.insert(
                                "display_price_formatted".to_string(),
                                serde_json::json!(format_price(converted, sale_currency)),
                            );
                        }
                    }
                    if let Some(op) = original_price {
                        variant_obj.insert(
                            "old_price_formatted".to_string(),
                            serde_json::json!(format_price(op, product_curr)),
                        );
                        if let Some(converted) = exchange_rate_service::convert_currency(
                            op,
                            product_curr,
                            sale_currency,
                            r,
                        ) {
                            variant_obj.insert(
                                "display_old_price".to_string(),
                                serde_json::json!(converted),
                            );
                            variant_obj.insert(
                                "display_old_price_formatted".to_string(),
                                serde_json::json!(format_price(converted, sale_currency)),
                            );
                        }
                    }
                }

                // Add display_currency to variant
                variant_obj.insert(
                    "display_currency".to_string(),
                    serde_json::json!(sale_currency),
                );

                // B2C kullanıcıları için b2b_price'ı kaldır
                variant_obj.remove("b2b_price");
                variant_obj.insert(
                    "display_currency".to_string(),
                    serde_json::json!(sale_currency),
                );
            }
        }
    }

    // Also add display_price and display_old_price to product level if they exist
    let (current_price, original_price, _) = extract_product_price(product);
    if current_price.is_some() || original_price.is_some() {
        let (display_price, display_old_price, _) = convert_prices_with_rates(
            current_price,
            original_price,
            product_currency,
            sale_currency,
            rates,
        );

        if let Some(obj) = product_clone.as_object_mut() {
            if let Some(dp) = display_price {
                obj.insert("display_price".to_string(), serde_json::json!(dp));
                obj.insert(
                    "display_price_formatted".to_string(),
                    serde_json::json!(format_price(dp, sale_currency)),
                );
            }
            if let Some(dop) = display_old_price {
                obj.insert("display_old_price".to_string(), serde_json::json!(dop));
                obj.insert(
                    "display_old_price_formatted".to_string(),
                    serde_json::json!(format_price(dop, sale_currency)),
                );
            }
        }
    }

    // Also add display_currency to product level
    if let Some(obj) = product_clone.as_object_mut() {
        obj.insert(
            "display_currency".to_string(),
            serde_json::json!(sale_currency),
        );
    }

    Some(product_clone)
}

/// Extract price from product data (first variant price if variants exist, otherwise price old_price)
/// Returns (current_price, original_price, currency)
/// Note: current_price is calculated by applying discount_percentage to the original price
fn extract_product_price(product_data: &Value) -> (Option<f64>, Option<f64>, Option<String>) {
    // Get currency from product data
    let currency = product_data
        .get("currency")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    // Try to get first variant's price
    if let Some(variants) = product_data.get("variants").and_then(|v| v.as_array()) {
        if let Some(first_variant) = variants.first() {
            let original_price = first_variant
                .get("price")
                .and_then(|p| p.as_f64().or_else(|| p.as_i64().map(|i| i as f64)));

            // Calculate discounted price (what customer actually pays)
            let discount_percentage = first_variant
                .get("discount_percentage")
                .and_then(|d| d.as_f64());
            let current_price = calculate_discounted_price(original_price, discount_percentage);

            return (current_price, original_price, currency);
        }
    }

    // Fallback to price if no variants
    let original_price = product_data
        .get("price")
        .and_then(|p| p.as_f64().or_else(|| p.as_i64().map(|i| i as f64)));

    // Calculate discounted price (what customer actually pays)
    let discount_percentage = product_data
        .get("discount_percentage")
        .and_then(|d| d.as_f64());
    let current_price = calculate_discounted_price(original_price, discount_percentage);

    (current_price, original_price, currency)
}

/// Calculate discounted price from original price and discount_percentage
/// Formula: discounted_price = original_price * (1 - discount_percentage / 100)
fn calculate_discounted_price(
    original_price: Option<f64>,
    discount_percentage: Option<f64>,
) -> Option<f64> {
    match (original_price, discount_percentage) {
        (Some(p), Some(d)) if d > 0.0 && d < 100.0 => Some(p * (1.0 - d / 100.0)),
        (Some(p), _) => Some(p),
        _ => None,
    }
}

/// Batch fetch tags for multiple contents (N+1 query fix)
pub async fn fetch_tags_for_contents(
    db: &sea_orm::DatabaseConnection,
    content_ids: &[i64],
    language: &str,
) -> Result<std::collections::HashMap<i64, Vec<TagInfo>>, sea_orm::DbErr> {
    use crate::modules::content::models::{content_terms, ContentTerm};
    use crate::modules::taxonomy::models::{term, Term};
    use sea_orm::*;

    if content_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    // Get all content_terms for these contents
    let content_terms = ContentTerm::find()
        .filter(content_terms::Column::ContentId.is_in(content_ids.to_vec()))
        .all(db)
        .await?;

    if content_terms.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    // Group by content_id
    let mut content_term_map: std::collections::HashMap<i64, Vec<i64>> =
        std::collections::HashMap::new();
    for ct in &content_terms {
        content_term_map
            .entry(ct.content_id)
            .or_insert_with(Vec::new)
            .push(ct.term_id);
    }

    // Get all term IDs
    let term_ids: Vec<i64> = content_terms.iter().map(|ct| ct.term_id).collect();

    // Settings'ten tags kategorileri vocabulary ID'sini al
    let vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(db, "tags_categories")
            .await
            .unwrap_or(7); // Fallback olarak 7 kullan

    // Fetch all terms in one query
    let terms = Term::find()
        .filter(term::Column::Id.is_in(term_ids))
        .filter(term::Column::VocabularyId.eq(vocab_id)) // Tags vocabulary
        .filter(term::Column::Publish.eq(true))
        .all(db)
        .await?;

    // Create term_id -> TagInfo map
    let term_info_map: std::collections::HashMap<i64, TagInfo> = terms
        .into_iter()
        .map(|term| {
            let title = get_string_from_json(&term.data, language, "title")
                .unwrap_or_else(|| format!("Tag {}", term.id));
            let slug = get_string_from_json(&term.data, language, "slug")
                .unwrap_or_else(|| format!("tag-{}", term.id));

            (
                term.id,
                TagInfo {
                    id: term.id,
                    title,
                    slug,
                },
            )
        })
        .collect();

    // Build final map: content_id -> Vec<TagInfo>
    let mut result: std::collections::HashMap<i64, Vec<TagInfo>> = std::collections::HashMap::new();
    for (content_id, term_ids) in content_term_map {
        let tags: Vec<TagInfo> = term_ids
            .iter()
            .filter_map(|tid| term_info_map.get(tid).cloned())
            .collect();
        result.insert(content_id, tags);
    }

    Ok(result)
}

/// Get tags for content
async fn get_content_tags(
    db: &sea_orm::DatabaseConnection,
    content_id: i64,
    content_type: &str,
    language: &str,
) -> Vec<TagInfo> {
    use crate::modules::content::models::{content_terms, ContentTerm};
    use crate::modules::taxonomy::models::{term, Term};
    use sea_orm::*;

    // Get term IDs for this content
    let content_terms = match ContentTerm::find()
        .filter(content_terms::Column::ContentId.eq(content_id))
        .filter(content_terms::Column::ContentType.eq(content_type))
        .all(db)
        .await
    {
        Ok(terms) => terms,
        Err(_) => return vec![],
    };

    let term_ids: Vec<i64> = content_terms.iter().map(|ct| ct.term_id).collect();
    if term_ids.is_empty() {
        return vec![];
    }

    // Settings'ten tags kategorileri vocabulary ID'sini al
    let vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(db, "tags_categories")
            .await
            .unwrap_or(7); // Fallback olarak 7 kullan

    // Get terms (tags from vocabulary_id = 7)
    let terms = match Term::find()
        .filter(term::Column::Id.is_in(term_ids))
        .filter(term::Column::VocabularyId.eq(vocab_id)) // Tags vocabulary
        .filter(term::Column::Publish.eq(true))
        .all(db)
        .await
    {
        Ok(terms) => terms,
        Err(_) => return vec![],
    };

    // Convert to TagInfo
    let taglar: Vec<TagInfo> = terms
        .iter()
        .map(|term| {
            let title = get_string_from_json(&term.data, language, "title")
                .unwrap_or_else(|| format!("Tag {}", term.id));
            let slug = get_string_from_json(&term.data, language, "slug")
                .unwrap_or_else(|| format!("tag-{}", term.id));

            TagInfo {
                id: term.id,
                title,
                slug,
            }
        })
        .collect();

    // println!("Found {} tags for content {}", taglar.len(), content_id);
    // for tag in &taglar {
    //     println!(" - Tag: {} (slug: {})", tag.title, tag.slug);
    // }
    taglar
}

/// Verilen term ID'lerini toplu olarak getirir ve id -> term JSON (id, title, slug, data) şeklinde bir harita döner
pub async fn fetch_terms_by_ids(
    db: &sea_orm::DatabaseConnection,
    term_ids: &[i64],
    language: &str,
) -> Result<std::collections::HashMap<i64, serde_json::Value>, sea_orm::DbErr> {
    use crate::modules::taxonomy::models::{term, Term};
    use sea_orm::*;

    let mut result: std::collections::HashMap<i64, serde_json::Value> =
        std::collections::HashMap::new();
    if term_ids.is_empty() {
        return Ok(result);
    }

    let terms = Term::find()
        .filter(term::Column::Id.is_in(term_ids.to_vec()))
        .filter(term::Column::Publish.eq(true))
        .all(db)
        .await?;

    for t in terms {
        let title = get_string_from_json(&t.data, language, "title")
            .unwrap_or_else(|| format!("Term {}", t.id));
        let slug = get_string_from_json(&t.data, language, "slug")
            .unwrap_or_else(|| format!("term-{}", t.id));
        let description = get_string_from_json(&t.data, language, "description")
            .unwrap_or_else(|| "Tanımsız Description".to_string());

        let term_json = serde_json::json!({
            "id": t.id,
            "title": title,
            "description": description,
            "slug": slug,
            "data": t.data,
            "vocabulary_id": t.vocabulary_id,
        });

        result.insert(t.id, term_json);
    }

    Ok(result)
}

/// Öznitelik ID dizilerini, önceden getirilmiş term haritasını kullanarak term nesnelerinin dizilerine çevirir
pub fn enrich_product_attributes(
    product: Option<serde_json::Value>,
    term_map: &std::collections::HashMap<i64, serde_json::Value>,
) -> Option<serde_json::Value> {
    let mut product = product?;

    if let Some(attributes) = product
        .get_mut("attributes")
        .and_then(|v| v.as_object_mut())
    {
        let keys: Vec<String> = attributes.keys().cloned().collect();
        for key in keys {
            if let Some(val) = attributes.get(&key).cloned() {
                if let Some(arr) = val.as_array() {
                    let enriched: Vec<serde_json::Value> = arr
                        .iter()
                        .filter_map(|idv| idv.as_i64())
                        .map(|id| match term_map.get(&id) {
                            Some(term) => term.clone(),
                            None => serde_json::json!({"id": id}),
                        })
                        .collect();
                    attributes.insert(key, serde_json::Value::Array(enriched));
                }
            }
        }
    }

    Some(product)
}

/// Ürün verisini listeslemede/detayda/homepage render'ında gösterime hazır hale getirir
/// - fiyat dönüşümlerini uygular
/// - variant/display fiyatlarını ekler
/// - attributes içindeki ID'leri term nesneleriyle zenginleştirir
pub async fn prepare_product_for_display(
    product_data: Option<&serde_json::Value>,
    db: &sea_orm::DatabaseConnection,
    language: &str,
    sale_currency: &str,
    rates: Option<&crate::modules::currency::models::ExchangeRateModel>,
    attribute_terms_map: Option<&std::collections::HashMap<i64, serde_json::Value>>,
) -> Option<serde_json::Value> {
    let product_data = product_data?;

    // Ürün para birimi
    let product_currency = product_data
        .get("currency")
        .and_then(|c| c.as_str())
        .unwrap_or("TRY");

    // Variant ve üretken fiyat dönüşümlerini uygula
    let mut product = convert_product_with_variants(
        Some(product_data),
        Some(product_currency),
        sale_currency,
        rates,
    )?;

    // Ürün seviyesindeki fiyatları da garantiye al (display_price, display_old_price, formatted)
    let (current_price, original_price, _) = extract_product_price(product_data);
    let (display_price, display_old_price, display_currency) = convert_prices_with_rates(
        current_price,
        original_price,
        Some(product_currency),
        sale_currency,
        rates,
    );

    if let Some(obj) = product.as_object_mut() {
        if let Some(dp) = display_price {
            obj.remove("b2b_price"); //b2b price normal b2c kullanıcısına görünmesin diye kaldırıyoruz, b2b response zaten b2b modülü içinde
            obj.insert("display_price".to_string(), serde_json::json!(dp));
            let formatted = crate::modules::utils::format_price::format_price(
                dp,
                display_currency.as_deref().unwrap_or("TRY"),
            );
            obj.insert(
                "display_price_formatted".to_string(),
                serde_json::json!(formatted),
            );
        }
        if let Some(dop) = display_old_price {
            obj.insert("display_old_price".to_string(), serde_json::json!(dop));
            let formatted = crate::modules::utils::format_price::format_price(
                dop,
                display_currency.as_deref().unwrap_or("TRY"),
            );
            obj.insert(
                "display_old_price_formatted".to_string(),
                serde_json::json!(formatted),
            );
        }
        obj.insert(
            "display_currency".to_string(),
            serde_json::json!(display_currency.unwrap_or_else(|| sale_currency.to_string())),
        );
    }

    // Attributes için term haritasını kullanarak zenginleştir
    if let Some(map) = attribute_terms_map {
        product = enrich_product_attributes(Some(product), map)?;
    } else {
        if let Some(attrs) = product_data.get("attributes").and_then(|a| a.as_object()) {
            let mut ids: Vec<i64> = Vec::new();
            for (_k, v) in attrs {
                if let Some(arr) = v.as_array() {
                    for idv in arr {
                        if let Some(id) = idv.as_i64() {
                            ids.push(id);
                        }
                    }
                }
            }

            ids.sort_unstable();
            ids.dedup();

            if !ids.is_empty() {
                let term_map = fetch_terms_by_ids(db, &ids, language)
                    .await
                    .unwrap_or_default();
                product = enrich_product_attributes(Some(product), &term_map)?;
            }
        }
    }

    Some(product)
}

/// Convert ContentModel to ProductResponse for a specific language
pub async fn to_product_get_response(
    content: &crate::modules::content::models::ContentModel,
    language: &str,
    db: &sea_orm::DatabaseConnection,
    display_currency: Option<&str>,
) -> ProductResponse {
    let config = get_config();
    let lang = config.get_language_or_default(Some(language));

    // content.data.product removed cloned
    let mut data = content.data.clone();
    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut data, &lang);

    data.as_object_mut().map(|obj| obj.remove("product"));
    data.as_object_mut().map(|obj| obj.remove("form_settings"));

    let template = content
        .data
        .get("template")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Get tags for this content
    let tags = get_content_tags(db, content.id, &content.content_type, &lang).await;

    // Load sub contents for this content
    let sub_contents = crate::modules::content::helpers::sub_content_helper::load_sub_contents(
        db,
        content.id,
        Some(&lang),
    )
    .await
    .unwrap_or_default();

    // Generate absolute URL
    let get_absolute_url = content.get_absolute_url(&lang);

    // Extract price from product data
    let (price, old_price, currency) = if let Some(product_data) = content.data.get("product") {
        extract_product_price(product_data)
    } else {
        (None, None, None)
    };

    // Get discount_percentage from product data
    let discount_percentage = if let Some(product_data) = content.data.get("product") {
        product_data
            .get("discount_percentage")
            .and_then(|d| d.as_f64())
            .filter(|&d| d > 0.0)
    } else {
        None
    };

    // Get sale currency and rates for conversion
    let sale_currency = crate::modules::admin::services::settings_service::get_sale_currency(db)
        .await
        .unwrap_or_else(|| "TRY".to_string());
    let target_currency = display_currency.unwrap_or(&sale_currency);
    let rates =
        crate::modules::currency::services::exchange_rate_service::get_cached_rates(db).await;

    // Convert main prices
    let (display_price, display_old_price, display_currency) = convert_prices_with_rates(
        price,
        old_price,
        currency.as_deref(),
        target_currency,
        rates.as_ref(),
    );

    // Hazır product objesini tek fonksiyonda hazırla (fiyat dönüşümü + attributes zenginleştirme)
    let product_with_converted_variants = prepare_product_for_display(
        content.data.get("product"),
        db,
        &lang,
        target_currency,
        rates.as_ref(),
        None,
    )
    .await;

    // Format prices for display
    let price_formatted = display_price.map(|p| {
        crate::modules::utils::format_price::format_price(
            p,
            display_currency.as_deref().unwrap_or(target_currency),
        )
    });
    let old_price_formatted = display_old_price.map(|p| {
        crate::modules::utils::format_price::format_price(
            p,
            display_currency.as_deref().unwrap_or(target_currency),
        )
    });

    ProductResponse {
        id: content.id,
        data: data,
        product: product_with_converted_variants,
        publish: content.publish,
        language: lang.clone(),

        title: get_string_from_json(&content.data, &lang, "title")
            .unwrap_or_else(|| "Başlıksız".to_string()),
        slug: get_string_from_json(&content.data, &lang, "slug").unwrap_or_default(),
        description: get_string_from_json(&content.data, &lang, "description"),
        body: get_string_from_json(&content.data, &lang, "body").unwrap_or_default(),
        meta_title: get_string_from_json(&content.data, &lang, "meta_title"),
        meta_description: get_string_from_json(&content.data, &lang, "meta_description"),
        template,
        available_languages: get_available_languages(&content.data),
        tags,
        sub_contents,
        get_absolute_url,
        price,
        old_price,
        discount_percentage,
        currency,
        display_price,
        display_old_price,
        display_currency,
        price_formatted,
        old_price_formatted,

        created_at: content.created_at.map(|dt| dt.naive_utc().and_utc()),
        updated_at: content.updated_at.map(|dt| dt.naive_utc().and_utc()),
    }
}

/// Convert ContentModel to PageResponse using pre-fetched tags (for batch operations)
pub async fn to_product_list_response(
    db: &sea_orm::DatabaseConnection,
    content: &crate::modules::content::models::ContentModel,
    language: &str,
    tags_map: &std::collections::HashMap<i64, Vec<TagInfo>>,
    sale_currency: &str,
    rates: Option<&crate::modules::currency::models::ExchangeRateModel>,
    attribute_terms_map: &std::collections::HashMap<i64, serde_json::Value>,
) -> ProductResponse {
    let config = get_config();
    let lang = config.get_language_or_default(Some(language));

    // content.data.product removed cloned
    let mut data = content.data.clone();
    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut data, &lang);

    data.as_object_mut().map(|obj| obj.remove("product"));
    data.as_object_mut().map(|obj| obj.remove("form_settings"));

    let template = content
        .data
        .get("template")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Get tags from pre-fetched map
    let tags = tags_map.get(&content.id).cloned().unwrap_or_default();

    // For batch operations, don't load sub contents (performance)
    let sub_contents = std::collections::HashMap::new();

    // Generate absolute URL
    let get_absolute_url = content.get_absolute_url(&lang);

    // Extract price from product data
    let (price, old_price, currency) = if let Some(product_data) = content.data.get("product") {
        extract_product_price(product_data)
    } else {
        (None, None, None)
    };

    // Get discount_percentage from product data
    let discount_percentage = if let Some(product_data) = content.data.get("product") {
        product_data
            .get("discount_percentage")
            .and_then(|d| d.as_f64())
            .filter(|&d| d > 0.0)
    } else {
        None
    };

    // Convert prices using pre-fetched rates
    let (display_price, display_old_price, display_currency) =
        convert_prices_with_rates(price, old_price, currency.as_deref(), sale_currency, rates);

    // Hazır product objesini tek fonksiyonda hazırla (fiyat dönüşümü + attributes zenginleştirme)
    let product_with_converted_variants = prepare_product_for_display(
        content.data.get("product"),
        db,
        &lang,
        sale_currency,
        rates,
        Some(attribute_terms_map),
    )
    .await;

    // Format prices for display
    let price_formatted = display_price.map(|p| {
        crate::modules::utils::format_price::format_price(
            p,
            display_currency.as_deref().unwrap_or("TRY"),
        )
    });
    let old_price_formatted = display_old_price.map(|p| {
        crate::modules::utils::format_price::format_price(
            p,
            display_currency.as_deref().unwrap_or("TRY"),
        )
    });

    ProductResponse {
        id: content.id,
        data: data,
        product: product_with_converted_variants,
        publish: content.publish,
        language: lang.clone(),

        title: get_string_from_json(&content.data, &lang, "title")
            .unwrap_or_else(|| "Başlıksız".to_string()),
        slug: get_string_from_json(&content.data, &lang, "slug").unwrap_or_default(),
        description: get_string_from_json(&content.data, &lang, "description"),
        body: get_string_from_json(&content.data, &lang, "body").unwrap_or_default(),
        meta_title: get_string_from_json(&content.data, &lang, "meta_title"),
        meta_description: get_string_from_json(&content.data, &lang, "meta_description"),
        template,
        available_languages: get_available_languages(&content.data),
        tags,
        sub_contents,
        get_absolute_url,
        price,
        old_price,
        discount_percentage,
        currency,
        display_price,
        display_old_price,
        display_currency,
        price_formatted,
        old_price_formatted,

        created_at: content.created_at.map(|dt| dt.naive_utc().and_utc()),
        updated_at: content.updated_at.map(|dt| dt.naive_utc().and_utc()),
    }
}

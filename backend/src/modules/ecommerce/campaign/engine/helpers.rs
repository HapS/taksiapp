use rust_decimal::Decimal;
use sea_orm::{EntityTrait, ColumnTrait, QueryFilter, PaginatorTrait, ConnectionTrait};

use crate::modules::currency::models::exchange_rate::Model as ExchangeRateModel;



pub async fn get_product_title(db: &impl ConnectionTrait, product_id: i64) -> String {
    use crate::modules::content::models::Content;

    if let Some(product) = Content::find_by_id(product_id).one(db).await.ok().flatten() {
        if let Some(data) = product.data.as_object() {
            if let Some(langs) = data.get("langs").and_then(|l| l.as_object()) {
                for (_, lang_data) in langs {
                    if let Some(title) = lang_data.get("title").and_then(|t| t.as_str()) {
                        if !title.is_empty() {
                            return title.to_string();
                        }
                    }
                }
            }
            if let Some(title) = data.get("title").and_then(|t| t.as_str()) {
                return title.to_string();
            }
        }
    }
    format!("Ürün #{}", product_id)
}

pub async fn get_product_price_in_currency(
    db: &impl ConnectionTrait,
    product_id: i64,
    display_currency: &str,
    rates: Option<&ExchangeRateModel>,
) -> Option<Decimal> {
    use crate::modules::content::models::{Content};
    use crate::modules::currency::services::exchange_rate_service::convert_currency;

    let product = Content::find_by_id(product_id).one(db).await.ok()??;
    let data = product.data.as_object()?;
    let product_obj = data.get("product")?;
    let price_val = product_obj.get("price")?;
    let price_f64 = price_val.as_f64()?;
    let product_price = Decimal::try_from(price_f64).ok()?;

    let product_currency = product_obj
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("TRY")
        .to_uppercase();

    let display = display_currency.to_uppercase();

    if product_currency == display {
        Some(product_price)
    } else if let Some(rates) = rates {
        let converted = convert_currency(
            product_price.to_string().parse::<f64>().unwrap_or(0.0),
            &product_currency,
            &display,
            rates,
        );
        converted.and_then(|v| Decimal::try_from(v).ok()).or(Some(product_price))
    } else {
        Some(product_price)
    }
}

pub async fn get_products_by_term(db: &impl ConnectionTrait, term_id: i64) -> Vec<i64> {
    use crate::modules::content::models::{Content, content::Column as C};

    let contents = Content::find()
        .filter(C::ContentType.eq("product"))
        .filter(C::Publish.eq(true))
        .filter(C::DeletedAt.is_null())
        .all(db)
        .await
        .unwrap_or_default();

    let mut product_ids = Vec::new();

    for content in &contents {
        if let Some(data) = content.data.as_object() {
            if let Some(term_master_id) = data.get("term_master_id").and_then(|v| v.as_i64()) {
                if term_master_id == term_id {
                    product_ids.push(content.id);
                    continue;
                }
            }
            if let Some(tags) = data.get("tags").and_then(|v| v.as_array()) {
                for tag in tags {
                    if let Some(tag_id) = tag.as_i64() {
                        if tag_id == term_id {
                            product_ids.push(content.id);
                            break;
                        }
                    }
                }
            }
        }
    }

    product_ids
}

pub async fn user_has_completed_orders(db: &impl ConnectionTrait, user_id: i64) -> bool {
    use crate::modules::ecommerce::models::cart::{Entity as Cart, Column as C};
    use crate::modules::ecommerce::models::cart::status;

    let count = Cart::find()
        .filter(C::UserId.eq(user_id))
        .filter(
            C::Status.is_in(vec![
                status::CONFIRMED.to_string(),
                status::PREPARING.to_string(),
                status::SHIPPED.to_string(),
                status::DELIVERED.to_string(),
            ]),
        )
        .count(db)
        .await
        .unwrap_or(0);

    count > 0
}
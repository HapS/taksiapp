use rust_decimal::Decimal;
use sea_orm::ConnectionTrait;

use crate::modules::ecommerce::campaign::engine::{CartItemInfo, DiscountResult, ScenarioType};
use crate::modules::ecommerce::campaign::scenario::QuantityDiscountPercentParams;
use crate::modules::ecommerce::models::cart_discount::{discount_type, scope};

use super::super::helpers::get_product_title;

#[allow(unused_variables)]
pub async fn eval_quantity_discount_percent(
    db: &impl ConnectionTrait,
    params: &QuantityDiscountPercentParams,
    cart_items: &[CartItemInfo],
) -> Option<DiscountResult> {
    let mut total_quantity: i32 = 0;
    let mut target_cart_item_id: Option<i64> = None;
    let mut total_line_total: Decimal = Decimal::ZERO;
    let mut target_currency: Option<String> = None;

    for item in cart_items {
        if item.product_id == params.product_id {
            total_quantity += item.quantity;
            // İndirimi son eşleşen satıra bağlıyoruz (scope::ITEM için)
            target_cart_item_id = Some(item.cart_item_id);
            total_line_total += item.line_total;
            if target_currency.is_none() {
                target_currency = Some(item.currency.clone());
            }
        }
    }

    if total_quantity < params.min_quantity {
        return None;
    }

    // İndirim tutarı, o ürüne ait tüm satırların toplamı üzerinden hesaplanır
    let discount_amount = total_line_total * params.discount_percent / Decimal::from(100);
    let currency = target_currency.unwrap_or_else(|| "TRY".to_string());

    let product_title = get_product_title(db, params.product_id).await;

    Some(DiscountResult {
        campaign_id: 0,
        coupon_id: None,
        scenario_type: ScenarioType::QuantityDiscountPercent,
        discount_type: discount_type::PERCENT.to_string(),
        scope: scope::ITEM.to_string(),
        cart_item_id: target_cart_item_id,
        amount: discount_amount,
        currency,
        description: format!("{} adet {} alındığında %{} indirim", params.min_quantity, product_title, params.discount_percent),
        cart_id: 0,
    })
}
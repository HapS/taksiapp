use rust_decimal::Decimal;
use sea_orm::ConnectionTrait;

use crate::modules::ecommerce::campaign::engine::{CartItemInfo, DiscountResult, ScenarioType};
use crate::modules::ecommerce::campaign::scenario::BuyXGetYFreeParams;
use crate::modules::ecommerce::models::cart_discount::{discount_type, scope};
use crate::modules::currency::models::exchange_rate::Model as ExchangeRateModel;

use super::super::helpers::{get_product_price_in_currency, get_product_title};

pub async fn eval_buy_x_get_y_free(
    db: &impl ConnectionTrait,
    params: &BuyXGetYFreeParams,
    cart_items: &[CartItemInfo],
    display_currency: &str,
    exchange_rates: Option<&ExchangeRateModel>,
) -> Option<DiscountResult> {
    let mut buy_count: i32 = 0;
    for item in cart_items {
        if item.product_id == params.buy_product_id {
            buy_count += item.quantity;
        }
    }

    if buy_count < params.buy_quantity {
        return None;
    }

    let gift_price = get_product_price_in_currency(db, params.get_product_id, display_currency, exchange_rates).await?;
    let amount = gift_price * Decimal::from(params.get_quantity);

    // Adım 3: Hediye ürünün sepette olup olmadığını kontrol ediyoruz.
    // Eğer hediye ürün sepette yoksa kampanya uygulanmaz.
    let get_cart_item_id = cart_items
        .iter()
        .find(|i| i.product_id == params.get_product_id)
        .map(|i| i.cart_item_id)?;

    let buy_title = get_product_title(db, params.buy_product_id).await;
    let get_title = get_product_title(db, params.get_product_id).await;

    let description = if params.buy_product_id == params.get_product_id {
        format!("{} adet {} alınca {} adet bedava", params.buy_quantity, buy_title, params.get_quantity)
    } else {
        format!("{} adet {} alınca {} adet {} bedava", params.buy_quantity, buy_title, params.get_quantity, get_title)
    };

    Some(DiscountResult {
        campaign_id: 0,
        coupon_id: None,
        scenario_type: ScenarioType::BuyXGetYFree,
        discount_type: discount_type::FREE_PRODUCT.to_string(),
        scope: scope::ITEM.to_string(),
        cart_item_id: Some(get_cart_item_id),
        amount,
        currency: display_currency.to_string(),
        description,
        cart_id: 0,
    })
}
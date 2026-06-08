use rust_decimal::Decimal;
use sea_orm::ConnectionTrait;

use crate::modules::ecommerce::campaign::engine::{CartItemInfo, DiscountResult};
use crate::modules::ecommerce::campaign::scenario::{CategorySpendGetDiscountParams, RewardType, ScenarioType};
use crate::modules::ecommerce::models::cart_discount::{discount_type, scope};
use crate::modules::currency::models::exchange_rate::Model as ExchangeRateModel;
use crate::modules::currency::services::exchange_rate_service::convert_currency;

use super::super::helpers::get_products_by_term;

fn convert_amount(
    amount: Decimal,
    from_currency: &str,
    to_currency: &str,
    exchange_rates: Option<&ExchangeRateModel>,
) -> Decimal {
    if from_currency.eq_ignore_ascii_case(to_currency) {
        return amount;
    }
    if let Some(rates) = exchange_rates {
        let converted = convert_currency(
            amount.to_string().parse::<f64>().unwrap_or(0.0),
            from_currency,
            to_currency,
            rates,
        );
        converted.and_then(|v| Decimal::try_from(v).ok()).unwrap_or(amount)
    } else {
        amount
    }
}

pub async fn eval_category_spend_get_discount(
    db: &impl ConnectionTrait,
    params: &CategorySpendGetDiscountParams,
    cart_items: &[CartItemInfo],
    display_currency: &str,
    exchange_rates: Option<&ExchangeRateModel>,
) -> Option<DiscountResult> {
    let category_product_ids = get_products_by_term(db, params.term_id).await;

    let mut category_total = Decimal::ZERO;

    for item in cart_items {
        if category_product_ids.contains(&item.product_id) {
            category_total += item.line_total;
        }
    }

    let min_spend_converted = convert_amount(params.min_spend, &params.currency, display_currency, exchange_rates);

    if category_total < min_spend_converted {
        return None;
    }

    match &params.reward_type {
        RewardType::FixedDiscount => {
            let reward_converted = convert_amount(params.reward_value, &params.currency, display_currency, exchange_rates);
            Some(DiscountResult {
                campaign_id: 0,
                coupon_id: None,
                scenario_type: ScenarioType::CategorySpendGetDiscount,
                discount_type: discount_type::FIXED.to_string(),
                scope: scope::CART.to_string(),
                cart_item_id: None,
                amount: reward_converted,
                currency: display_currency.to_string(),
                description: format!("Kategori harcamasında {} {} indirim", params.reward_value, params.currency),
                cart_id: 0,
            })
        }
        RewardType::PercentDiscount => {
            let discount_amount = category_total * params.reward_value / Decimal::from(100);
            Some(DiscountResult {
                campaign_id: 0,
                coupon_id: None,
                scenario_type: ScenarioType::CategorySpendGetDiscount,
                discount_type: discount_type::PERCENT.to_string(),
                scope: scope::CART.to_string(),
                cart_item_id: None,
                amount: discount_amount,
                currency: display_currency.to_string(),
                description: format!("Kategori harcamasında %{} indirim", params.reward_value),
                cart_id: 0,
            })
        }
        RewardType::Coupon => Some(DiscountResult {
            campaign_id: 0,
            coupon_id: None,
            scenario_type: ScenarioType::CategorySpendGetDiscount,
            discount_type: discount_type::PENDING_COUPON.to_string(),
            scope: scope::CART.to_string(),
            cart_item_id: None,
            amount: params.reward_value,
            currency: display_currency.to_string(),
            description: format!(
                "Sipariş tamamlandığında {} {} kupon kazanacaksınız",
                params.reward_value, params.currency
            ),
            cart_id: 0,
        }),
    }
}
use rust_decimal::Decimal;
use sea_orm::ConnectionTrait;

use crate::modules::ecommerce::campaign::engine::DiscountResult;
use crate::modules::ecommerce::campaign::scenario::{FirstOrderDiscountParams, RewardType, ScenarioType};
use crate::modules::ecommerce::models::cart_discount::{discount_type, scope};
use crate::modules::currency::models::exchange_rate::Model as ExchangeRateModel;
use crate::modules::currency::services::exchange_rate_service::convert_currency;

use super::super::helpers::user_has_completed_orders;

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

pub async fn eval_first_order_discount(
    db: &impl ConnectionTrait,
    params: &FirstOrderDiscountParams,
    user_id: i64,
    cart_total: Decimal,
    cart_currency: &str,
    exchange_rates: Option<&ExchangeRateModel>,
) -> Option<DiscountResult> {
    if user_has_completed_orders(db, user_id).await {
        return None;
    }

    match &params.reward_type {
        RewardType::FixedDiscount => {
            let reward_converted = convert_amount(params.reward_value, &params.currency, cart_currency, exchange_rates);
            Some(DiscountResult {
                campaign_id: 0,
                coupon_id: None,
                scenario_type: ScenarioType::FirstOrderDiscount,
                discount_type: discount_type::FIXED.to_string(),
                scope: scope::CART.to_string(),
                cart_item_id: None,
                amount: reward_converted,
                currency: cart_currency.to_string(),
                description: format!("İlk siparişe özel {} {} indirim", params.reward_value, params.currency),
                cart_id: 0,
            })
        }
        RewardType::PercentDiscount => {
            let discount_amount = cart_total * params.reward_value / Decimal::from(100);
            Some(DiscountResult {
                campaign_id: 0,
                coupon_id: None,
                scenario_type: ScenarioType::FirstOrderDiscount,
                discount_type: discount_type::PERCENT.to_string(),
                scope: scope::CART.to_string(),
                cart_item_id: None,
                amount: discount_amount,
                currency: cart_currency.to_string(),
                description: format!("İlk siparişe özel %{} indirim", params.reward_value),
                cart_id: 0,
            })
        }
        RewardType::Coupon => None,
    }
}
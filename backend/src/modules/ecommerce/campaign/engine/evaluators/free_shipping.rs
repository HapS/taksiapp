use rust_decimal::Decimal;

use crate::modules::ecommerce::campaign::engine::DiscountResult;
use crate::modules::ecommerce::campaign::scenario::{FreeShippingParams, ScenarioType};
use crate::modules::ecommerce::models::cart_discount::{discount_type, scope};
use crate::modules::currency::models::exchange_rate::Model as ExchangeRateModel;
use crate::modules::currency::services::exchange_rate_service::convert_currency;

pub fn convert_amount(
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

pub fn eval_free_shipping(
    params: &FreeShippingParams,
    cart_total: Decimal,
    cart_currency: &str,
    exchange_rates: Option<&ExchangeRateModel>,
) -> Option<DiscountResult> {
    let min_converted = convert_amount(params.min_cart_total, &params.currency, cart_currency, exchange_rates);

    if cart_total < min_converted {
        return None;
    }

    Some(DiscountResult {
        campaign_id: 0,
        coupon_id: None,
        scenario_type: ScenarioType::FreeShipping,
        discount_type: discount_type::FREE_SHIPPING.to_string(),
        scope: scope::CART.to_string(),
        cart_item_id: None,
        amount: Decimal::ZERO,
        currency: cart_currency.to_string(),
        description: format!(
            "Sepet toplamı {} {} üzeri, kargo bedava",
            params.min_cart_total, params.currency
        ),
        cart_id: 0,
    })
}
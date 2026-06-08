use rust_decimal::Decimal;
use sea_orm::ConnectionTrait;

use crate::modules::ecommerce::campaign::engine::{CartItemInfo, DiscountResult};
use crate::modules::ecommerce::campaign::scenario::{CouponCodeParams, CouponScope, RewardType, ScenarioType};
use crate::modules::ecommerce::models::cart_discount::{discount_type, scope};
use crate::modules::ecommerce::models::CouponModel;
use crate::modules::currency::models::exchange_rate::Model as ExchangeRateModel;
use crate::modules::currency::services::exchange_rate_service::convert_currency;

use super::super::helpers::get_product_title;

fn convert_reward_value(
    reward_value: Decimal,
    from_currency: &str,
    to_currency: &str,
    exchange_rates: Option<&ExchangeRateModel>,
) -> Decimal {
    if from_currency.eq_ignore_ascii_case(to_currency) {
        return reward_value;
    }
    if let Some(rates) = exchange_rates {
        let converted = convert_currency(
            reward_value.to_string().parse::<f64>().unwrap_or(0.0),
            from_currency,
            to_currency,
            rates,
        );
        converted.and_then(|v| Decimal::try_from(v).ok()).unwrap_or(reward_value)
    } else {
        reward_value
    }
}

pub async fn eval_coupon_code(
    db: &impl ConnectionTrait,
    params: &CouponCodeParams,
    coupon: &CouponModel,
    cart_total: Decimal,
    cart_currency: &str,
    cart_items: &[CartItemInfo],
    exchange_rates: Option<&ExchangeRateModel>,
) -> Option<DiscountResult> {
    if !coupon.is_active {
        return None;
    }

    if let Some(max_usage) = coupon.max_usage {
        if coupon.usage_count >= max_usage {
            return None;
        }
    }

    if let Some(valid_until) = coupon.valid_until {
        let now = chrono::Utc::now();
        if now > valid_until {
            return None;
        }
    }

    let converted_reward = convert_reward_value(
        params.reward_value,
        &params.currency,
        cart_currency,
        exchange_rates,
    );

    match &params.scope {
        CouponScope::Cart => match &params.reward_type {
            RewardType::FixedDiscount => Some(DiscountResult {
                campaign_id: 0,
                coupon_id: Some(coupon.id),
                scenario_type: ScenarioType::CouponCode,
                discount_type: discount_type::FIXED.to_string(),
                scope: scope::CART.to_string(),
                cart_item_id: None,
                amount: converted_reward,
                currency: cart_currency.to_string(),
                description: format!("Kupon: {} {} indirim", params.reward_value, params.currency),
                cart_id: 0,
            }),
            RewardType::PercentDiscount => {
                let discount_amount = cart_total * params.reward_value / Decimal::from(100);
                Some(DiscountResult {
                    campaign_id: 0,
                    coupon_id: Some(coupon.id),
                    scenario_type: ScenarioType::CouponCode,
                    discount_type: discount_type::PERCENT.to_string(),
                    scope: scope::CART.to_string(),
                    cart_item_id: None,
                    amount: discount_amount,
                    currency: cart_currency.to_string(),
                    description: format!("Kupon: %{} indirim", params.reward_value),
                    cart_id: 0,
                })
            }
            RewardType::Coupon => None,
        },
        CouponScope::Product => {
            let target_id = params.scope_target_id?;
            let mut target_line_total = Decimal::ZERO;
            let mut target_cart_item_id: Option<i64> = None;

            for item in cart_items {
                if item.product_id == target_id {
                    target_line_total = item.line_total;
                    target_cart_item_id = Some(item.cart_item_id);
                    break;
                }
            }

            if target_line_total == Decimal::ZERO {
                return None;
            }

            let product_title = get_product_title(db, target_id).await;

            match &params.reward_type {
                RewardType::FixedDiscount => {
                    if converted_reward > target_line_total {
                        Some(DiscountResult {
                            campaign_id: 0,
                            coupon_id: Some(coupon.id),
                            scenario_type: ScenarioType::CouponCode,
                            discount_type: discount_type::FIXED.to_string(),
                            scope: scope::ITEM.to_string(),
                            cart_item_id: target_cart_item_id,
                            amount: target_line_total,
                            currency: cart_currency.to_string(),
                            description: format!("Kupon: {} için {} {} indirim", product_title, params.reward_value, params.currency),
                            cart_id: 0,
                        })
                    } else {
                        Some(DiscountResult {
                            campaign_id: 0,
                            coupon_id: Some(coupon.id),
                            scenario_type: ScenarioType::CouponCode,
                            discount_type: discount_type::FIXED.to_string(),
                            scope: scope::ITEM.to_string(),
                            cart_item_id: target_cart_item_id,
                            amount: converted_reward,
                            currency: cart_currency.to_string(),
                            description: format!("Kupon: {} için {} {} indirim", product_title, params.reward_value, params.currency),
                            cart_id: 0,
                        })
                    }
                }
                RewardType::PercentDiscount => {
                    let discount_amount = target_line_total * params.reward_value / Decimal::from(100);
                    Some(DiscountResult {
                        campaign_id: 0,
                        coupon_id: Some(coupon.id),
                        scenario_type: ScenarioType::CouponCode,
                        discount_type: discount_type::PERCENT.to_string(),
                        scope: scope::ITEM.to_string(),
                        cart_item_id: target_cart_item_id,
                        amount: discount_amount,
                        currency: cart_currency.to_string(),
                        description: format!("Kupon: {} için %{} indirim", product_title, params.reward_value),
                        cart_id: 0,
                    })
                }
                RewardType::Coupon => None,
            }
        }
        CouponScope::Term => {
            let target_id = params.scope_target_id?;
            let category_product_ids = super::super::helpers::get_products_by_term(db, target_id).await;
            
            let mut category_total = Decimal::ZERO;
            for item in cart_items {
                if category_product_ids.contains(&item.product_id) {
                    category_total += item.line_total;
                }
            }

            if category_total == Decimal::ZERO {
                return None;
            }

            match &params.reward_type {
                RewardType::FixedDiscount => {
                    let mut final_amount = converted_reward;
                    if final_amount > category_total {
                        final_amount = category_total;
                    }
                    Some(DiscountResult {
                        campaign_id: 0,
                        coupon_id: Some(coupon.id),
                        scenario_type: ScenarioType::CouponCode,
                        discount_type: discount_type::FIXED.to_string(),
                        scope: scope::CART.to_string(),
                        cart_item_id: None,
                        amount: final_amount,
                        currency: cart_currency.to_string(),
                        description: format!("Kupon: Kategori için {} {} indirim", params.reward_value, params.currency),
                        cart_id: 0,
                    })
                }
                RewardType::PercentDiscount => {
                    let discount_amount = category_total * params.reward_value / Decimal::from(100);
                    Some(DiscountResult {
                        campaign_id: 0,
                        coupon_id: Some(coupon.id),
                        scenario_type: ScenarioType::CouponCode,
                        discount_type: discount_type::PERCENT.to_string(),
                        scope: scope::CART.to_string(),
                        cart_item_id: None,
                        amount: discount_amount,
                        currency: cart_currency.to_string(),
                        description: format!("Kupon: Kategori için %{} indirim", params.reward_value),
                        cart_id: 0,
                    })
                }
                RewardType::Coupon => None,
            }
        }
    }
}
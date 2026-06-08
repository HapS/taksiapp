use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioType {
    BuyXGetYFree,
    QuantityDiscountPercent,
    CategorySpendGetDiscount,
    CartTotalDiscount,
    CouponCode,
    FreeShipping,
    FirstOrderDiscount,
}

impl ScenarioType {
    pub fn all() -> Vec<Self> {
        vec![
            Self::BuyXGetYFree,
            Self::QuantityDiscountPercent,
            Self::CategorySpendGetDiscount,
            Self::CartTotalDiscount,
            Self::CouponCode,
            Self::FreeShipping,
            Self::FirstOrderDiscount,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BuyXGetYFree => "buy_x_get_y_free",
            Self::QuantityDiscountPercent => "quantity_discount_percent",
            Self::CategorySpendGetDiscount => "category_spend_get_discount",
            Self::CartTotalDiscount => "cart_total_discount",
            Self::CouponCode => "coupon_code",
            Self::FreeShipping => "free_shipping",
            Self::FirstOrderDiscount => "first_order_discount",
        }
    }
}

impl std::fmt::Display for ScenarioType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RewardType {
    FixedDiscount,
    PercentDiscount,
    Coupon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CouponScope {
    Cart,
    Product,
    Term,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuyXGetYFreeParams {
    pub buy_product_id: i64,
    pub buy_quantity: i32,
    pub get_product_id: i64,
    pub get_quantity: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantityDiscountPercentParams {
    pub product_id: i64,
    pub min_quantity: i32,
    pub discount_percent: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySpendGetDiscountParams {
    pub term_id: i64,
    pub min_spend: Decimal,
    pub currency: String,
    pub reward_type: RewardType,
    pub reward_value: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartTotalDiscountParams {
    pub min_cart_total: Decimal,
    pub currency: String,
    pub reward_type: RewardType,
    pub reward_value: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouponCodeParams {
    pub reward_type: RewardType,
    pub reward_value: Decimal,
    pub currency: String,
    pub scope: CouponScope,
    #[serde(default)]
    pub scope_target_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeShippingParams {
    pub min_cart_total: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstOrderDiscountParams {
    pub reward_type: RewardType,
    pub reward_value: Decimal,
    pub currency: String,
}

use rust_decimal::Decimal;
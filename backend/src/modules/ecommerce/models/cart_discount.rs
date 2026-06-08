use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cart_discounts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    pub cart_id: i64,

    pub campaign_id: i64,

    pub coupon_id: Option<i64>,

    pub scenario_type: String,

    pub discount_type: String,

    pub scope: String,

    pub cart_item_id: Option<i64>,

    #[sea_orm(column_type = "Decimal(Some((12, 2)))", default_value = 0)]
    pub amount: Decimal,

    #[sea_orm(default_value = "TRY")]
    pub currency: String,

    pub description: String,

    pub created_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::modules::ecommerce::models::cart::Entity",
        from = "Column::CartId",
        to = "crate::modules::ecommerce::models::cart::Column::Id"
    )]
    Cart,

    #[sea_orm(
        belongs_to = "super::campaign::Entity",
        from = "Column::CampaignId",
        to = "super::campaign::Column::Id"
    )]
    Campaign,

    #[sea_orm(
        belongs_to = "super::coupon::Entity",
        from = "Column::CouponId",
        to = "super::coupon::Column::Id"
    )]
    Coupon,

    #[sea_orm(
        belongs_to = "crate::modules::ecommerce::models::cart_item::Entity",
        from = "Column::CartItemId",
        to = "crate::modules::ecommerce::models::cart_item::Column::Id"
    )]
    CartItem,
}

impl Related<crate::modules::ecommerce::models::cart::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cart.def()
    }
}

impl Related<super::campaign::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Campaign.def()
    }
}

impl Related<super::coupon::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Coupon.def()
    }
}

impl Related<crate::modules::ecommerce::models::cart_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CartItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub mod discount_type {
    pub const PERCENT: &str = "percent";
    pub const FIXED: &str = "fixed";
    pub const FREE_SHIPPING: &str = "free_shipping";
    pub const FREE_PRODUCT: &str = "free_product";
    pub const PENDING_COUPON: &str = "pending_coupon";
}

pub mod scope {
    pub const CART: &str = "cart";
    pub const ITEM: &str = "item";
}
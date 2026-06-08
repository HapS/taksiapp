use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "campaigns")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    pub name: String,

    #[sea_orm(column_type = "Text", nullable)]
    pub description: Option<String>,

    pub scenario_type: String,

    #[sea_orm(column_type = "Json")]
    pub params: Json,

    #[sea_orm(default_value = "automatic")]
    pub campaign_type: String,

    pub starts_at: Option<DateTimeWithTimeZone>,

    pub ends_at: Option<DateTimeWithTimeZone>,

    #[sea_orm(default_value = true)]
    pub is_active: bool,

    #[sea_orm(default_value = 0)]
    pub priority: i32,

    #[sea_orm(default_value = false)]
    pub stackable: bool,

    pub max_uses: Option<i32>,

    pub max_uses_per_user: Option<i32>,

    #[sea_orm(default_value = 0)]
    pub usage_count: i32,

    #[sea_orm(default_value = "both")]
    pub target_cart_type: String,

    pub created_at: Option<DateTimeWithTimeZone>,

    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::coupon::Entity")]
    Coupon,

    #[sea_orm(has_many = "super::cart_discount::Entity")]
    CartDiscount,

    #[sea_orm(has_many = "super::campaign_usage::Entity")]
    CampaignUsage,
}

impl Related<super::coupon::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Coupon.def()
    }
}

impl Related<super::cart_discount::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CartDiscount.def()
    }
}

impl Related<super::campaign_usage::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CampaignUsage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub mod campaign_type {
    pub const AUTOMATIC: &str = "automatic";
    pub const COUPON: &str = "coupon";
}
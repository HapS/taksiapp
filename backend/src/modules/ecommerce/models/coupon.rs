use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "coupons")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    pub campaign_id: i64,

    pub code: String,

    pub user_id: Option<i64>,

    pub max_usage: Option<i32>,

    #[sea_orm(default_value = 0)]
    pub usage_count: i32,

    pub valid_until: Option<DateTimeWithTimeZone>,

    #[sea_orm(default_value = true)]
    pub is_active: bool,

    pub created_at: Option<DateTimeWithTimeZone>,

    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::campaign::Entity",
        from = "Column::CampaignId",
        to = "super::campaign::Column::Id"
    )]
    Campaign,

    #[sea_orm(has_many = "super::cart_discount::Entity")]
    CartDiscount,

    #[sea_orm(has_many = "super::campaign_usage::Entity")]
    CampaignUsage,
}

impl Related<super::campaign::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Campaign.def()
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
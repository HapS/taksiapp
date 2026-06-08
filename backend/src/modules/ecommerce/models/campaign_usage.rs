use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "campaign_usages")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    pub campaign_id: i64,

    pub coupon_id: Option<i64>,

    pub user_id: i64,

    pub cart_id: i64,

    pub used_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
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

impl ActiveModelBehavior for ActiveModel {}
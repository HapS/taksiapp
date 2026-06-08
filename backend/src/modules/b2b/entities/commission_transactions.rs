use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// İşlem tipleri
pub mod transaction_type {
    pub const EARNED: &str = "earned";       // Komisyon kazanıldı
    pub const PAYMENT: &str = "payment";     // Komisyon ödendi
    pub const ADJUSTMENT: &str = "adjustment"; // Manuel düzeltme
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "representative_commission_transactions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    
    pub representative_id: i64,
    pub company_id: i64,
    pub cart_id: Option<i64>,
    
    pub transaction_type: String,
    pub amount: Decimal,
    pub order_amount: Option<Decimal>,
    pub commission_rate: Option<Decimal>,
    pub currency: String,
    
    pub balance_before: Decimal,
    pub balance_after: Decimal,
    
    pub description: Option<String>,
    pub reference_number: Option<String>,
    pub created_by: Option<i64>,
    pub created_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::company_representatives::Entity",
        from = "Column::RepresentativeId",
        to = "super::company_representatives::Column::Id"
    )]
    Representative,

    #[sea_orm(
        belongs_to = "super::companies::Entity",
        from = "Column::CompanyId",
        to = "super::companies::Column::Id"
    )]
    Company,

    #[sea_orm(
        belongs_to = "crate::modules::ecommerce::models::cart::Entity",
        from = "Column::CartId",
        to = "crate::modules::ecommerce::models::cart::Column::Id"
    )]
    Cart,

    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::CreatedBy",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    CreatedByUser,
}

impl Related<super::company_representatives::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Representative.def()
    }
}

impl Related<super::companies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Company.def()
    }
}

impl Related<crate::modules::ecommerce::models::cart::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cart.def()
    }
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CreatedByUser.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

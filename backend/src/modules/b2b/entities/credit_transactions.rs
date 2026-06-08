use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "b2b_credit_transactions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    // İlişkiler
    pub company_id: i64,
    pub cart_id: Option<i64>,

    // İşlem Bilgileri
    pub transaction_type: String, // 'purchase', 'payment', 'adjustment', 'refund'
    pub amount: Decimal,
    pub currency: String,

    // Bakiye Takibi
    pub balance_before: Decimal,
    pub balance_after: Decimal,

    // Açıklama ve Referans
    pub description: Option<String>,
    pub reference_number: Option<String>,

    // Oluşturan (manuel işlemler için admin user_id)
    pub created_by: Option<i64>,

    // Zaman Damgası
    pub created_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
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

impl ActiveModelBehavior for ActiveModel {}

// İşlem tipleri için sabitler
pub mod transaction_type {
    pub const PURCHASE: &str = "purchase"; // Kredili alışveriş
    pub const PAYMENT: &str = "payment"; // Ödeme yapıldı (kredi limiti arttı)
    pub const ADJUSTMENT: &str = "adjustment"; // Manuel düzeltme (admin)
    pub const REFUND: &str = "refund"; // İade (kredi limiti arttı)
}

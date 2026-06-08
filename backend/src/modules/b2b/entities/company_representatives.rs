use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "company_representatives")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub company_id: i64,
    pub user_id: i64,
    pub commission_rate: Decimal,
    pub accumulated_commission: Decimal,
    pub total_sales_amount: Decimal,
    pub is_active: bool,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    User,
}

impl Related<super::companies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Company.def()
    }
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Komisyon hesapla
    pub fn calculate_commission(&self, order_amount: Decimal) -> Decimal {
        order_amount * (self.commission_rate / Decimal::from(100))
    }
}

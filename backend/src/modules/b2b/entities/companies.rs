use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "companies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    // İlişkiler
    pub user_id: i64,

    // Şirket Bilgileri
    pub company_name: String,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub trade_registry_no: Option<String>,

    // İletişim (mevcut tablolarla ilişkili)
    pub country_id: Option<i64>,
    pub city_id: Option<i64>,
    pub district_id: Option<i64>,
    pub address_line: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub logo: Option<String>,

    // B2B Özellikleri
    pub discount_percentage: Decimal,
    pub credit_limit: Decimal,
    pub used_credit: Decimal,
    pub payment_term_days: i32,
    pub min_order_amount: Decimal,
    pub currency: Option<String>,

    // Durum
    pub is_active: bool,
    pub notes: Option<String>,

    // Zaman Damgaları
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub approved_at: Option<DateTimeWithTimeZone>,
    pub approved_by: Option<i64>,

    // Hiyerarşi
    pub parent_company_id: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    User,

    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::ApprovedBy",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    ApprovedByUser,

    #[sea_orm(
        belongs_to = "crate::modules::ecommerce::models::country::Entity",
        from = "Column::CountryId",
        to = "crate::modules::ecommerce::models::country::Column::Id"
    )]
    Country,

    #[sea_orm(
        belongs_to = "crate::modules::ecommerce::models::city::Entity",
        from = "Column::CityId",
        to = "crate::modules::ecommerce::models::city::Column::Id"
    )]
    City,

    #[sea_orm(
        belongs_to = "crate::modules::ecommerce::models::district::Entity",
        from = "Column::DistrictId",
        to = "crate::modules::ecommerce::models::district::Column::Id"
    )]
    District,

    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::ParentCompanyId",
        to = "Column::Id"
    )]
    ParentCompany,

    #[sea_orm(has_many = "super::company_users::Entity")]
    CompanyUsers,

    #[sea_orm(has_one = "super::company_representatives::Entity")]
    Representative,
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<crate::modules::ecommerce::models::country::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Country.def()
    }
}

impl Related<crate::modules::ecommerce::models::city::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::City.def()
    }
}

impl Related<crate::modules::ecommerce::models::district::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::District.def()
    }
}

impl Related<super::company_users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CompanyUsers.def()
    }
}

impl Related<super::company_representatives::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Representative.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Kullanılabilir kredi limitini hesapla
    pub fn available_credit(&self) -> Decimal {
        self.credit_limit - self.used_credit
    }
}

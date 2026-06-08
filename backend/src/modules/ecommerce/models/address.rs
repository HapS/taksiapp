use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "addresses")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub country_id: i64,
    pub city_id: i64,
    pub district_id: i64,
    #[sea_orm(column_type = "Text")]
    pub address_line: String,
    pub is_default: bool,
    pub phone_country_code: String,
    pub phone_number: String,

    pub address_type: String, // "individual" or "corporate"
    pub company_name: Option<String>,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub id_number: Option<String>,

    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::country::Entity",
        from = "Column::CountryId",
        to = "super::country::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    Country,
    #[sea_orm(
        belongs_to = "super::city::Entity",
        from = "Column::CityId",
        to = "super::city::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    City,
    #[sea_orm(
        belongs_to = "super::district::Entity",
        from = "Column::DistrictId",
        to = "super::district::Column::Id",
        on_update = "NoAction",
        on_delete = "Restrict"
    )]
    District,
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::country::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Country.def()
    }
}

impl Related<super::city::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::City.def()
    }
}

impl Related<super::district::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::District.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

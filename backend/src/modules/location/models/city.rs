use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub country_id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::country::Entity",
        from = "Column::CountryId",
        to = "super::country::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Country,
    #[sea_orm(has_many = "super::district::Entity")]
    Districts,
}

impl Related<super::country::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Country.def()
    }
}

impl Related<super::district::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Districts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

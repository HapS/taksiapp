use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "countries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub code: Option<String>,
    pub phone_code: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::city::Entity")]
    Cities,
}

impl Related<super::city::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cities.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

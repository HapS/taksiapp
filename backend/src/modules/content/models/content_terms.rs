// Content-Terms pivot table entity
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "content_terms")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub content_id: i64,

    #[sea_orm(primary_key, auto_increment = false)]
    pub term_id: i64,

    #[sea_orm(primary_key, auto_increment = false)]
    pub content_type: String,

    pub created_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

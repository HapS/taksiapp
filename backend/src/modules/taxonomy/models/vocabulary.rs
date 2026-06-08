// Vocabulary Entity
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "vocabularies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub data: Json,
    pub vocabulary_type: String,
    pub order_id: Option<i32>,
    pub gcx: bool,
    pub lock: bool,
    pub hide: bool,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Type aliases for convenience
pub use ActiveModel as VocabularyActiveModel;
pub use Entity as Vocabulary;
pub use Model as VocabularyModel;

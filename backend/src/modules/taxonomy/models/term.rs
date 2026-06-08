// Term Entity
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "terms")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub vocabulary_id: i64,
    pub data: Json,
    pub parent_id: Option<i64>,
    pub order_id: Option<i32>,
    pub publish: bool,
    pub lock: bool,
    pub hide: bool,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn generate_slug(&self, title: &str) -> Option<String> {
        use slug::slugify;
        let slug = slugify(title);
        if slug.is_empty() {
            None
        } else {
            Some(slug)
        }
    }
}

// Type aliases for convenience
pub use ActiveModel as TermActiveModel;
pub use Entity as Term;
pub use Model as TermModel;

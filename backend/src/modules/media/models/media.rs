// Media Model
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "media")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i32,
    pub file_name: String,
    pub media_type: String, // image, video, audio, document
    pub mime_type: String,  // image/png, video/mp4, etc.
    pub file_path: String,  // Relative path from media root
    pub file_size: i64,     // File size in bytes
    pub title: Option<String>,
    pub description: Option<String>,
    pub content_type: Option<String>, // e.g., "pages", "product"
    pub content_id: Option<i64>,      // Related content ID
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

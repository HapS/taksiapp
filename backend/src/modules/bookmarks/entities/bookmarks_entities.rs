use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "bookmarks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub user_id: i64,
    pub module_name: String,
    pub content_type: String,
    pub content_id: i64,
    pub title: String,
    pub price: Option<String>,
    pub variant_key: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
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
        belongs_to = "crate::modules::content::models::content::Entity",
        from = "Column::ContentId",
        to = "crate::modules::content::models::content::Column::Id"
    )]
    Content,
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<crate::modules::content::models::content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Content.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Form Submission Model
/// Stores all form submissions from various forms (contact, HR, etc.)
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "form_submissions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Reference to the form content (content.id)
    pub form_id: i64,

    /// Form data stored as JSON
    pub data: JsonValue,

    /// IP address of the submitter
    pub ip: Option<String>,

    /// User ID if the user was logged in (optional)
    pub user_id: Option<i64>,

    /// Timestamps
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::modules::content::models::content::Entity",
        from = "Column::FormId",
        to = "crate::modules::content::models::content::Column::Id"
    )]
    Form,

    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    User,
}

impl Related<crate::modules::content::models::content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Form.def()
    }
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

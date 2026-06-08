use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "roles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::user_role::Entity")]
    UserRoles,
    #[sea_orm(has_many = "super::role_permission::Entity")]
    RolePermissions,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_role::Relation::Users.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::user_role::Relation::Roles.def().rev())
    }
}

impl Related<super::permission::Entity> for Entity {
    fn to() -> RelationDef {
        super::role_permission::Relation::Permissions.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::role_permission::Relation::Roles.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

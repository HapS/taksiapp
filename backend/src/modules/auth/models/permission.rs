use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "permissions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub module: String,
    pub created_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::role_permission::Entity")]
    RolePermissions,
    #[sea_orm(has_many = "super::user_permission::Entity")]
    UserPermissions,
}

impl Related<super::role::Entity> for Entity {
    fn to() -> RelationDef {
        super::role_permission::Relation::Roles.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::role_permission::Relation::Permissions.def().rev())
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_permission::Relation::Users.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::user_permission::Relation::Permissions.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

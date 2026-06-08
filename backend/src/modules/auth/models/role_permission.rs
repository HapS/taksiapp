use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "role_permissions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub permission_id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::role::Entity",
        from = "Column::RoleId",
        to = "super::role::Column::Id"
    )]
    Roles,
    #[sea_orm(
        belongs_to = "super::permission::Entity",
        from = "Column::PermissionId",
        to = "super::permission::Column::Id"
    )]
    Permissions,
}

impl Related<super::role::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Roles.def()
    }
}

impl Related<super::permission::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Permissions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

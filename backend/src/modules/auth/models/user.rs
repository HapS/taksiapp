// Auth models - User ve Session modelleri
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// User Entity
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub birth_date: Option<DateTimeWithTimeZone>,
    #[sea_orm(unique)]
    pub email: String,
    pub password: Option<String>,
    #[sea_orm(unique)]
    pub google_id: Option<String>,
    #[sea_orm(unique)]
    pub apple_id: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub profile: Option<serde_json::Value>,
    // Guest support fields
    pub is_guest: bool,
    pub guest_session_id: Option<String>,
    pub phone_number: Option<String>,
    pub phone_country_code: Option<String>,
    pub ip: Option<String>,
    pub ip_v6: Option<String>,
    pub user_type: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::session::Entity")]
    Sessions,
    #[sea_orm(has_many = "super::user_role::Entity")]
    UserRoles,
    #[sea_orm(has_many = "super::user_permission::Entity")]
    UserPermissions,
    #[sea_orm(has_many = "crate::modules::bookmarks::entities::bookmarks_entities::Entity")]
    Bookmarks,
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sessions.def()
    }
}

impl Related<super::role::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_role::Relation::Roles.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::user_role::Relation::Users.def().rev())
    }
}

impl Related<super::permission::Entity> for Entity {
    fn to() -> RelationDef {
        super::user_permission::Relation::Permissions.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::user_permission::Relation::Users.def().rev())
    }
}

impl Related<crate::modules::bookmarks::entities::bookmarks_entities::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bookmarks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Session Entity - ayrı modül olarak tanımla
// User type aliases - models/mod.rs'de export ediliyor
#[allow(dead_code)]
pub type UserActiveModel = ActiveModel;
#[allow(dead_code)]
pub type UserColumn = Column;

// Helper structs for API
// DTOs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub user_id: i64,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: String,
    pub has_admin_access: bool, // RBAC: has system.admin_access permission
    pub has_b2b_access: bool,   // RBAC: has system.b2b_access permission
    pub permissions: Vec<String>, // User's all permissions (from roles + overrides)
    pub login_time: chrono::DateTime<chrono::Utc>,
    pub profile: Option<serde_json::Value>,
}

impl Model {
    /// Convert User to SessionData (async version with permissions)
    pub async fn to_session_data(&self, db: &DatabaseConnection) -> Result<SessionData, DbErr> {
        // Load permissions with timeout protection
        let permissions = match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.get_all_permissions(db),
        )
        .await
        {
            Ok(Ok(perms)) => perms,
            Ok(Err(e)) => {
                eprintln!("Error loading permissions for user {}: {}", self.id, e);
                Vec::new()
            }
            Err(_) => {
                eprintln!("Timeout loading permissions for user {}", self.id);
                Vec::new()
            }
        };

        let has_admin_access = permissions.contains(&"system.admin_access".to_string());
        let has_b2b_access = permissions.contains(&"system.b2b_access".to_string());

        Ok(SessionData {
            user_id: self.id,
            username: self.username.clone(),
            first_name: self.first_name.clone(),
            last_name: self.last_name.clone(),
            email: self.email.clone(),
            has_admin_access,
            has_b2b_access,
            permissions,
            login_time: chrono::Utc::now(),
            profile: self.profile.clone(),
        })
    }

    /// Check if user has a specific permission
    ///
    /// Checks in this order:
    /// 1. If user is_admin -> always true
    /// 2. Check user_permissions (overrides)
    /// Check if user has a specific permission
    /// Uses RBAC system: checks user_permissions (overrides) and role_permissions
    pub async fn has_permission(
        &self,
        db: &DatabaseConnection,
        permission_name: &str,
    ) -> Result<bool, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};

        // First, find the permission by name
        let permission = super::permission::Entity::find()
            .filter(super::permission::Column::Name.eq(permission_name))
            .one(db)
            .await?;

        let Some(perm) = permission else {
            return Ok(false); // Permission doesn't exist
        };

        // Check user-specific permission overrides
        let user_permission = super::user_permission::Entity::find()
            .filter(super::user_permission::Column::UserId.eq(self.id))
            .filter(super::user_permission::Column::PermissionId.eq(perm.id))
            .one(db)
            .await?;

        if let Some(up) = user_permission {
            // If explicitly granted or denied, return that
            return Ok(up.is_granted);
        }

        // Check role permissions
        // Get user's roles
        let user_roles = super::user_role::Entity::find()
            .filter(super::user_role::Column::UserId.eq(self.id))
            .all(db)
            .await?;

        for user_role in user_roles {
            let has_perm = super::role_permission::Entity::find()
                .filter(super::role_permission::Column::RoleId.eq(user_role.role_id))
                .filter(super::role_permission::Column::PermissionId.eq(perm.id))
                .one(db)
                .await?;

            if has_perm.is_some() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check if user has any of the given permissions
    #[allow(dead_code)]
    pub async fn has_any_permission(
        &self,
        db: &DatabaseConnection,
        permissions: &[&str],
    ) -> Result<bool, DbErr> {
        for permission in permissions {
            if self.has_permission(db, permission).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check if user has all of the given permissions
    #[allow(dead_code)]
    pub async fn has_all_permissions(
        &self,
        db: &DatabaseConnection,
        permissions: &[&str],
    ) -> Result<bool, DbErr> {
        for permission in permissions {
            if !self.has_permission(db, permission).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Check if user has a specific role
    #[allow(dead_code)]
    pub async fn has_role(&self, db: &DatabaseConnection, role_name: &str) -> Result<bool, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};

        // Find the role by name
        let role = super::role::Entity::find()
            .filter(super::role::Column::Name.eq(role_name))
            .one(db)
            .await?;

        let Some(r) = role else {
            return Ok(false); // Role doesn't exist
        };

        // Check if user has this role
        let has_role = super::user_role::Entity::find()
            .filter(super::user_role::Column::UserId.eq(self.id))
            .filter(super::user_role::Column::RoleId.eq(r.id))
            .one(db)
            .await?
            .is_some();

        Ok(has_role)
    }

    /// Get all user permissions (from roles + user overrides)
    /// Uses RBAC system only
    pub async fn get_all_permissions(&self, db: &DatabaseConnection) -> Result<Vec<String>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};

        let mut permissions: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Get user's roles
        let user_roles = super::user_role::Entity::find()
            .filter(super::user_role::Column::UserId.eq(self.id))
            .all(db)
            .await?;

        // Get permissions from each role
        for user_role in user_roles {
            let role_perms = super::role_permission::Entity::find()
                .filter(super::role_permission::Column::RoleId.eq(user_role.role_id))
                .all(db)
                .await?;

            for rp in role_perms {
                if let Some(perm) = super::permission::Entity::find_by_id(rp.permission_id)
                    .one(db)
                    .await?
                {
                    permissions.insert(perm.name);
                }
            }
        }

        // Apply user-specific overrides
        let user_overrides = super::user_permission::Entity::find()
            .filter(super::user_permission::Column::UserId.eq(self.id))
            .all(db)
            .await?;

        for override_perm in user_overrides {
            let perm = super::permission::Entity::find_by_id(override_perm.permission_id)
                .one(db)
                .await?;

            if let Some(p) = perm {
                if override_perm.is_granted {
                    permissions.insert(p.name);
                } else {
                    permissions.remove(&p.name);
                }
            }
        }

        Ok(permissions.into_iter().collect())
    }

    /// Get all user roles
    #[allow(dead_code)]
    pub async fn get_roles(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<super::role::Model>, DbErr> {
        use sea_orm::{ColumnTrait, QueryFilter};

        let user_roles = super::user_role::Entity::find()
            .filter(super::user_role::Column::UserId.eq(self.id))
            .all(db)
            .await?;

        let mut roles = Vec::new();
        for ur in user_roles {
            if let Some(role) = super::role::Entity::find_by_id(ur.role_id).one(db).await? {
                roles.push(role);
            }
        }

        Ok(roles)
    }
}

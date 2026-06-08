use crate::modules::auth::models::{Permission, PermissionModel, Role, RoleModel};
use sea_orm::*;

#[derive(Debug)]
pub enum PermissionError {
    NotFound,
    AlreadyExists,
    DatabaseError(DbErr),
}

impl std::fmt::Display for PermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionError::NotFound => write!(f, "Resource not found"),
            PermissionError::AlreadyExists => write!(f, "Resource already exists"),
            PermissionError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
        }
    }
}

impl std::error::Error for PermissionError {}

impl From<DbErr> for PermissionError {
    fn from(err: DbErr) -> Self {
        PermissionError::DatabaseError(err)
    }
}

/// Assign a role to a user
pub async fn assign_role_to_user(
    db: &DatabaseConnection,
    user_id: i64,
    role_id: i64,
) -> Result<(), PermissionError> {
    use crate::modules::auth::models::user_role::*;

    // Check if already exists
    let existing = Entity::find()
        .filter(Column::UserId.eq(user_id))
        .filter(Column::RoleId.eq(role_id))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(PermissionError::AlreadyExists);
    }

    // Create new user_role
    let user_role = ActiveModel {
        user_id: Set(user_id),
        role_id: Set(role_id),
        created_at: Set(Some(chrono::Utc::now().into())),
    };

    user_role.insert(db).await?;

    Ok(())
}

/// Remove a role from a user
pub async fn remove_role_from_user(
    db: &DatabaseConnection,
    user_id: i64,
    role_id: i64,
) -> Result<(), PermissionError> {
    use crate::modules::auth::models::user_role::*;

    let user_role = Entity::find()
        .filter(Column::UserId.eq(user_id))
        .filter(Column::RoleId.eq(role_id))
        .one(db)
        .await?
        .ok_or(PermissionError::NotFound)?;

    user_role.delete(db).await?;

    Ok(())
}

/// Grant a permission to a user (override)
pub async fn grant_permission_to_user(
    db: &DatabaseConnection,
    user_id: i64,
    permission_id: i64,
) -> Result<(), PermissionError> {
    use crate::modules::auth::models::user_permission::*;

    // Check if already exists
    let existing = Entity::find()
        .filter(Column::UserId.eq(user_id))
        .filter(Column::PermissionId.eq(permission_id))
        .one(db)
        .await?;

    if let Some(existing_model) = existing {
        // Update to granted
        let mut active: ActiveModel = existing_model.into();
        active.is_granted = Set(true);
        active.update(db).await?;
    } else {
        // Create new
        let user_permission = ActiveModel {
            user_id: Set(user_id),
            permission_id: Set(permission_id),
            is_granted: Set(true),
            created_at: Set(Some(chrono::Utc::now().into())),
        };

        user_permission.insert(db).await?;
    }

    Ok(())
}

/// Revoke a permission from a user (deny override)
pub async fn revoke_permission_from_user(
    db: &DatabaseConnection,
    user_id: i64,
    permission_id: i64,
) -> Result<(), PermissionError> {
    use crate::modules::auth::models::user_permission::*;

    // Check if already exists
    let existing = Entity::find()
        .filter(Column::UserId.eq(user_id))
        .filter(Column::PermissionId.eq(permission_id))
        .one(db)
        .await?;

    if let Some(existing_model) = existing {
        // Update to denied
        let mut active: ActiveModel = existing_model.into();
        active.is_granted = Set(false);
        active.update(db).await?;
    } else {
        // Create new with denied
        let user_permission = ActiveModel {
            user_id: Set(user_id),
            permission_id: Set(permission_id),
            is_granted: Set(false),
            created_at: Set(Some(chrono::Utc::now().into())),
        };

        user_permission.insert(db).await?;
    }

    Ok(())
}

/// Remove user permission override (let role permissions take effect)
pub async fn remove_user_permission_override(
    db: &DatabaseConnection,
    user_id: i64,
    permission_id: i64,
) -> Result<(), PermissionError> {
    use crate::modules::auth::models::user_permission::*;

    let user_permission = Entity::find()
        .filter(Column::UserId.eq(user_id))
        .filter(Column::PermissionId.eq(permission_id))
        .one(db)
        .await?
        .ok_or(PermissionError::NotFound)?;

    user_permission.delete(db).await?;

    Ok(())
}

/// Get all permissions for a user (from roles + overrides)
pub async fn get_user_permissions(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<PermissionModel>, PermissionError> {
    use crate::modules::auth::models::user;

    let user = user::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or(PermissionError::NotFound)?;

    let permission_names = user.get_all_permissions(db).await?;

    // Batch fetch permissions by names (N+1 fix)
    let mut permissions = if !permission_names.is_empty() {
        Permission::find()
            .filter(crate::modules::auth::models::permission::Column::Name.is_in(permission_names))
            .all(db)
            .await?
    } else {
        Vec::new()
    };

    // Sort by module then name
    permissions.sort_by(|a, b| {
        match a.module.cmp(&b.module) {
            std::cmp::Ordering::Equal => a.name.cmp(&b.name),
            other => other,
        }
    });

    Ok(permissions)
}

/// Get all permissions for a role
#[allow(dead_code)]
pub async fn get_role_permissions(
    db: &DatabaseConnection,
    role_id: i64,
) -> Result<Vec<PermissionModel>, PermissionError> {
    use crate::modules::auth::models::role_permission::*;

    let role_perms = Entity::find()
        .filter(Column::RoleId.eq(role_id))
        .all(db)
        .await?;

    let mut permissions = Vec::new();
    for rp in role_perms {
        if let Some(perm) = Permission::find_by_id(rp.permission_id).one(db).await? {
            permissions.push(perm);
        }
    }

    Ok(permissions)
}

/// Assign a permission to a role
#[allow(dead_code)]
pub async fn assign_permission_to_role(
    db: &DatabaseConnection,
    role_id: i64,
    permission_id: i64,
) -> Result<(), PermissionError> {
    use crate::modules::auth::models::role_permission::*;

    // Check if already exists
    let existing = Entity::find()
        .filter(Column::RoleId.eq(role_id))
        .filter(Column::PermissionId.eq(permission_id))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(PermissionError::AlreadyExists);
    }

    // Create new role_permission
    let role_permission = ActiveModel {
        role_id: Set(role_id),
        permission_id: Set(permission_id),
    };

    role_permission.insert(db).await?;

    Ok(())
}

/// Remove a permission from a role
#[allow(dead_code)]
pub async fn remove_permission_from_role(
    db: &DatabaseConnection,
    role_id: i64,
    permission_id: i64,
) -> Result<(), PermissionError> {
    use crate::modules::auth::models::role_permission::*;

    let role_permission = Entity::find()
        .filter(Column::RoleId.eq(role_id))
        .filter(Column::PermissionId.eq(permission_id))
        .one(db)
        .await?
        .ok_or(PermissionError::NotFound)?;

    role_permission.delete(db).await?;

    Ok(())
}

/// Get all roles
pub async fn list_roles(db: &DatabaseConnection) -> Result<Vec<RoleModel>, PermissionError> {
    use sea_orm::QueryOrder;
    let roles = Role::find()
        .order_by_asc(crate::modules::auth::models::role::Column::Name)
        .all(db)
        .await?;
    Ok(roles)
}

/// Get all permissions
#[allow(dead_code)]
pub async fn list_permissions(
    db: &DatabaseConnection,
) -> Result<Vec<PermissionModel>, PermissionError> {
    let permissions = Permission::find().all(db).await?;
    Ok(permissions)
}

/// Get all permissions grouped by module
pub async fn list_permissions_by_module(
    db: &DatabaseConnection,
) -> Result<std::collections::BTreeMap<String, Vec<PermissionModel>>, PermissionError> {
    use sea_orm::QueryOrder;

    let permissions = Permission::find()
        .order_by_asc(crate::modules::auth::models::permission::Column::Module)
        .order_by_asc(crate::modules::auth::models::permission::Column::Name)
        .all(db)
        .await?;

    let mut grouped: std::collections::BTreeMap<String, Vec<PermissionModel>> =
        std::collections::BTreeMap::new();

    for perm in permissions {
        grouped
            .entry(perm.module.clone())
            .or_insert_with(Vec::new)
            .push(perm);
    }

    Ok(grouped)
}

/// Get user's roles
pub async fn get_user_roles(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<RoleModel>, PermissionError> {
    use crate::modules::auth::models::user_role::*;

    let user_roles = Entity::find()
        .filter(Column::UserId.eq(user_id))
        .all(db)
        .await?;

    // Batch fetch roles (N+1 fix)
    let role_ids: Vec<i64> = user_roles.iter().map(|ur| ur.role_id).collect();
    let mut roles = if !role_ids.is_empty() {
        Role::find()
            .filter(crate::modules::auth::models::role::Column::Id.is_in(role_ids))
            .all(db)
            .await?
    } else {
        Vec::new()
    };

    // Sort by name
    roles.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(roles)
}

/// Get user's permission overrides
pub async fn get_user_permission_overrides(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Vec<(PermissionModel, bool)>, PermissionError> {
    use crate::modules::auth::models::user_permission::*;

    let user_perms = Entity::find()
        .filter(Column::UserId.eq(user_id))
        .all(db)
        .await?;

    // Batch fetch permissions (N+1 fix)
    let permission_ids: Vec<i64> = user_perms.iter().map(|up| up.permission_id).collect();
    let permissions_map: std::collections::HashMap<i64, PermissionModel> = if !permission_ids.is_empty() {
        Permission::find()
            .filter(crate::modules::auth::models::permission::Column::Id.is_in(permission_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    let mut result = Vec::new();
    for up in user_perms {
        if let Some(perm) = permissions_map.get(&up.permission_id) {
            result.push((perm.clone(), up.is_granted));
        }
    }

    // Sort by module then name
    result.sort_by(|a, b| {
        match a.0.module.cmp(&b.0.module) {
            std::cmp::Ordering::Equal => a.0.name.cmp(&b.0.name),
            other => other,
        }
    });

    Ok(result)
}


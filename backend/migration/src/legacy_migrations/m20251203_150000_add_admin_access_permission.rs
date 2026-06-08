use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add system.admin_access permission
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(Permissions::Table)
                    .columns([
                        Permissions::Name,
                        Permissions::Description,
                        Permissions::Module,
                    ])
                    .values_panic([
                        "system.admin_access".into(),
                        "Access admin panel".into(),
                        "system".into(),
                    ])
                    .to_owned(),
            )
            .await?;

        // Assign to super_admin, admin, editor, and author roles using raw SQL
        let sql1 = r#"
            INSERT INTO role_permissions (role_id, permission_id)
            SELECT r.id, p.id
            FROM roles r, permissions p
            WHERE p.name = 'system.admin_access'
            AND r.name IN ('super_admin', 'admin', 'editor', 'author')
        "#;
        
        manager.get_connection().execute_unprepared(sql1).await?;

        // Migrate existing is_admin=true users to super_admin role
        // This ensures backward compatibility
        let sql2 = r#"
            INSERT INTO user_roles (user_id, role_id, created_at)
            SELECT u.id, r.id, NOW()
            FROM users u, roles r
            WHERE u.is_admin = true
            AND r.name = 'super_admin'
            AND NOT EXISTS (
                SELECT 1 FROM user_roles ur
                WHERE ur.user_id = u.id AND ur.role_id = r.id
            )
        "#;
        
        manager.get_connection().execute_unprepared(sql2).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove role assignments
        manager
            .exec_stmt(
                Query::delete()
                    .from_table(RolePermissions::Table)
                    .and_where(
                        Expr::col(RolePermissions::PermissionId).in_subquery(
                            Query::select()
                                .column(Permissions::Id)
                                .from(Permissions::Table)
                                .and_where(Expr::col(Permissions::Name).eq("system.admin_access"))
                                .to_owned(),
                        ),
                    )
                    .to_owned(),
            )
            .await?;

        // Remove permission
        manager
            .exec_stmt(
                Query::delete()
                    .from_table(Permissions::Table)
                    .and_where(Expr::col(Permissions::Name).eq("system.admin_access"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Permissions {
    Table,
    Id,
    Name,
    Description,
    Module,
}

#[derive(DeriveIden)]
enum RolePermissions {
    Table,
    #[allow(dead_code)]
    RoleId,
    PermissionId,
}

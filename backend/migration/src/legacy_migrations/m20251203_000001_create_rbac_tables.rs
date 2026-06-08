use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create roles table
        manager
            .create_table(
                Table::create()
                    .table(Roles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Roles::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Roles::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Roles::Description).text())
                    .col(ColumnDef::new(Roles::IsSystem).boolean().not_null().default(false))
                    .col(
                        ColumnDef::new(Roles::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Roles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 2. Create permissions table
        manager
            .create_table(
                Table::create()
                    .table(Permissions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Permissions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Permissions::Name)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Permissions::Description).text())
                    .col(ColumnDef::new(Permissions::Module).string().not_null())
                    .col(
                        ColumnDef::new(Permissions::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Create role_permissions junction table
        manager
            .create_table(
                Table::create()
                    .table(RolePermissions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RolePermissions::RoleId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RolePermissions::PermissionId)
                            .big_integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(RolePermissions::RoleId)
                            .col(RolePermissions::PermissionId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RolePermissions::Table, RolePermissions::RoleId)
                            .to(Roles::Table, Roles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RolePermissions::Table, RolePermissions::PermissionId)
                            .to(Permissions::Table, Permissions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 4. Create user_roles junction table
        manager
            .create_table(
                Table::create()
                    .table(UserRoles::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(UserRoles::UserId).big_integer().not_null())
                    .col(ColumnDef::new(UserRoles::RoleId).big_integer().not_null())
                    .col(
                        ColumnDef::new(UserRoles::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(UserRoles::UserId)
                            .col(UserRoles::RoleId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserRoles::Table, UserRoles::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserRoles::Table, UserRoles::RoleId)
                            .to(Roles::Table, Roles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 5. Create user_permissions junction table (for overrides)
        manager
            .create_table(
                Table::create()
                    .table(UserPermissions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserPermissions::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPermissions::PermissionId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserPermissions::IsGranted)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(UserPermissions::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(UserPermissions::UserId)
                            .col(UserPermissions::PermissionId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserPermissions::Table, UserPermissions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserPermissions::Table, UserPermissions::PermissionId)
                            .to(Permissions::Table, Permissions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 6. Insert default roles
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(Roles::Table)
                    .columns([Roles::Name, Roles::Description, Roles::IsSystem])
                    .values_panic([
                        "super_admin".into(),
                        "Super Administrator - Full system access".into(),
                        true.into(),
                    ])
                    .values_panic([
                        "admin".into(),
                        "Administrator - Content and user management".into(),
                        true.into(),
                    ])
                    .values_panic([
                        "editor".into(),
                        "Editor - Create and edit content".into(),
                        true.into(),
                    ])
                    .values_panic([
                        "author".into(),
                        "Author - Create and manage own content".into(),
                        true.into(),
                    ])
                    .values_panic([
                        "viewer".into(),
                        "Viewer - Read-only access".into(),
                        true.into(),
                    ])
                    .to_owned(),
            )
            .await?;

        // 7. Insert default permissions
        let permissions = vec![
            // Content permissions
            ("content.view", "View content", "content"),
            ("content.create", "Create content", "content"),
            ("content.edit.own", "Edit own content", "content"),
            ("content.edit.any", "Edit any content", "content"),
            ("content.delete.own", "Delete own content", "content"),
            ("content.delete.any", "Delete any content", "content"),
            ("content.publish", "Publish content", "content"),
            // Blog specific
            ("blog.view", "View blog posts", "content"),
            ("blog.create", "Create blog posts", "content"),
            ("blog.edit.own", "Edit own blog posts", "content"),
            ("blog.edit.any", "Edit any blog posts", "content"),
            ("blog.delete.own", "Delete own blog posts", "content"),
            ("blog.delete.any", "Delete any blog posts", "content"),
            ("blog.publish", "Publish blog posts", "content"),
            // News specific
            ("news.view", "View news", "content"),
            ("news.create", "Create news", "content"),
            ("news.edit.own", "Edit own news", "content"),
            ("news.edit.any", "Edit any news", "content"),
            ("news.delete.own", "Delete own news", "content"),
            ("news.delete.any", "Delete any news", "content"),
            ("news.publish", "Publish news", "content"),
            // Page specific
            ("page.view", "View pages", "content"),
            ("page.create", "Create pages", "content"),
            ("page.edit.own", "Edit own pages", "content"),
            ("page.edit.any", "Edit any pages", "content"),
            ("page.delete.own", "Delete own pages", "content"),
            ("page.delete.any", "Delete any pages", "content"),
            ("page.publish", "Publish pages", "content"),
            // Product specific
            ("product.view", "View products", "content"),
            ("product.create", "Create products", "content"),
            ("product.edit.own", "Edit own products", "content"),
            ("product.edit.any", "Edit any products", "content"),
            ("product.delete.own", "Delete own products", "content"),
            ("product.delete.any", "Delete any products", "content"),
            ("product.publish", "Publish products", "content"),
            // User permissions
            ("user.view", "View users", "user"),
            ("user.create", "Create users", "user"),
            ("user.edit.own", "Edit own profile", "user"),
            ("user.edit.any", "Edit any user", "user"),
            ("user.delete", "Delete users", "user"),
            ("user.manage_roles", "Manage user roles", "user"),
            ("user.manage_permissions", "Manage user permissions", "user"),
            // Media permissions
            ("media.view", "View media", "media"),
            ("media.upload", "Upload media", "media"),
            ("media.edit.own", "Edit own media", "media"),
            ("media.edit.any", "Edit any media", "media"),
            ("media.delete.own", "Delete own media", "media"),
            ("media.delete.any", "Delete any media", "media"),
            // Taxonomy permissions
            ("taxonomy.view", "View taxonomy", "taxonomy"),
            ("taxonomy.create", "Create taxonomy", "taxonomy"),
            ("taxonomy.edit", "Edit taxonomy", "taxonomy"),
            ("taxonomy.delete", "Delete taxonomy", "taxonomy"),
            // System permissions
            ("system.admin_access", "Access admin panel", "system"),
            ("system.settings", "Manage system settings", "system"),
            ("system.logs", "View system logs", "system"),
        ];

        for (name, description, module) in permissions {
            manager
                .exec_stmt(
                    Query::insert()
                        .into_table(Permissions::Table)
                        .columns([
                            Permissions::Name,
                            Permissions::Description,
                            Permissions::Module,
                        ])
                        .values_panic([name.into(), description.into(), module.into()])
                        .to_owned(),
                )
                .await?;
        }

        // 8. Assign all permissions to super_admin role
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(RolePermissions::Table)
                    .columns([RolePermissions::RoleId, RolePermissions::PermissionId])
                    .select_from(
                        Query::select()
                            .expr(Expr::cust("1"))
                            .column(Permissions::Id)
                            .from(Permissions::Table)
                            .to_owned(),
                    )
                    .unwrap()
                    .to_owned(),
            )
            .await?;

        // 9. Assign super_admin role to all existing users (mevcut kullanıcılar full yetkili)
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(UserRoles::Table)
                    .columns([UserRoles::UserId, UserRoles::RoleId])
                    .select_from(
                        Query::select()
                            .column(Users::Id)
                            .expr(Expr::cust("1"))
                            .from(Users::Table)
                            .to_owned(),
                    )
                    .unwrap()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserPermissions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserRoles::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RolePermissions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Permissions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Roles::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Roles {
    Table,
    Id,
    Name,
    Description,
    IsSystem,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Permissions {
    Table,
    Id,
    Name,
    Description,
    Module,
    CreatedAt,
}

#[derive(DeriveIden)]
enum RolePermissions {
    Table,
    RoleId,
    PermissionId,
}

#[derive(DeriveIden)]
enum UserRoles {
    Table,
    UserId,
    RoleId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum UserPermissions {
    Table,
    UserId,
    PermissionId,
    IsGranted,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

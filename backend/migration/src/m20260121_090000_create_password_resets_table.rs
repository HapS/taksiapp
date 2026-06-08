use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PasswordResets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PasswordResets::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PasswordResets::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PasswordResets::Token).string().not_null())
                    .col(ColumnDef::new(PasswordResets::Email).string().not_null())
                    .col(
                        ColumnDef::new(PasswordResets::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PasswordResets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-password_resets-user_id")
                            .from(PasswordResets::Table, PasswordResets::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index on token
        manager
            .create_index(
                Index::create()
                    .name("idx-password_resets-token")
                    .table(PasswordResets::Table)
                    .col(PasswordResets::Token)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PasswordResets::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PasswordResets {
    Table,
    Id,
    UserId,
    Token,
    Email,
    ExpiresAt,
    CreatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

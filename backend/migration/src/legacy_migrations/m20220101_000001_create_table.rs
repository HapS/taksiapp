use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create users table
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(big_integer(Users::Id).auto_increment().primary_key())
                    .col(string_uniq(Users::Username))
                    .col(string_null(Users::FirstName))
                    .col(string_null(Users::LastName))
                    .col(timestamp_with_time_zone_null(Users::BirthDate))
                    .col(string_uniq(Users::Email))
                    .col(string(Users::Password))
                    .col(boolean(Users::IsAdmin).default(false))
                    .col(timestamp_with_time_zone_null(Users::CreatedAt))
                    .col(timestamp_with_time_zone_null(Users::UpdatedAt))
                    .to_owned(),
            )
            .await?;

        // Create sessions table
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(string(Sessions::Id).primary_key())
                    .col(big_integer(Sessions::UserId))
                    .col(json(Sessions::Data))
                    .col(timestamp_with_time_zone(Sessions::ExpiresAt))
                    .col(timestamp_with_time_zone_null(Sessions::CreatedAt))
                    .col(timestamp_with_time_zone_null(Sessions::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sessions_user_id")
                            .from(Sessions::Table, Sessions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_expires_at")
                    .table(Sessions::Table)
                    .col(Sessions::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sessions_user_id")
                    .table(Sessions::Table)
                    .col(Sessions::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Username,
    FirstName,
    LastName,
    BirthDate,
    Email,
    Password,
    IsAdmin,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    UserId,
    Data,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}

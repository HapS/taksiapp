use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add guest support fields to users table
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    // Guest flag - true if this is a guest user
                    .add_column(
                        ColumnDef::new(Users::IsGuest)
                            .boolean()
                            .not_null()
                            .default(false)
                    )
                    // Guest session ID - for linking with session
                    .add_column(
                        ColumnDef::new(Users::GuestSessionId)
                            .string()
                            .null()
                    )
                    // Phone number - for guest users (and regular users)
                    .add_column(
                        ColumnDef::new(Users::PhoneNumber)
                            .string()
                            .null()
                    )
                    .to_owned(),
            )
            .await?;

        // Add index for guest session ID lookup
        manager
            .create_index(
                Index::create()
                    .name("idx_users_guest_session_id")
                    .table(Users::Table)
                    .col(Users::GuestSessionId)
                    .to_owned(),
            )
            .await?;

        // Add index for is_guest flag
        manager
            .create_index(
                Index::create()
                    .name("idx_users_is_guest")
                    .table(Users::Table)
                    .col(Users::IsGuest)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes
        manager
            .drop_index(
                Index::drop()
                    .name("idx_users_guest_session_id")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_users_is_guest")
                    .to_owned(),
            )
            .await?;

        // Drop columns
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::IsGuest)
                    .drop_column(Users::GuestSessionId)
                    .drop_column(Users::PhoneNumber)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    IsGuest,
    GuestSessionId,
    PhoneNumber,
}
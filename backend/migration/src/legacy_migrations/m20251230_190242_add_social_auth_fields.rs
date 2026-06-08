use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(ColumnDef::new(Users::GoogleId).string().unique_key().null())
                    .add_column(ColumnDef::new(Users::AppleId).string().unique_key().null())
                    .modify_column(ColumnDef::new(Users::Password).string().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::GoogleId)
                    .drop_column(Users::AppleId)
                    .modify_column(ColumnDef::new(Users::Password).string().not_null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Password,
    GoogleId,
    AppleId,
}

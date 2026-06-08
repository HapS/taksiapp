use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Cart tablosuna sipariş alanlarını ekle
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(
                        ColumnDef::new(Carts::Notes)
                            .text()
                            .null()
                    )
                    .add_column(
                        ColumnDef::new(Carts::TotalAmount)
                            .decimal()
                            .null()
                    )
                    .add_column(
                        ColumnDef::new(Carts::CompletedAt)
                            .timestamp_with_time_zone()
                            .null()
                    )
                    .to_owned(),
            )
            .await?;

        // Index'ler ekle
        manager
            .create_index(
                Index::create()
                    .name("idx_carts_status_user_id")
                    .table(Carts::Table)
                    .col(Carts::Status)
                    .col(Carts::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_carts_completed_at")
                    .table(Carts::Table)
                    .col(Carts::CompletedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Index'leri kaldır
        manager
            .drop_index(
                Index::drop()
                    .name("idx_carts_status_user_id")
                    .table(Carts::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_carts_completed_at")
                    .table(Carts::Table)
                    .to_owned(),
            )
            .await?;

        // Alanları kaldır
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::Notes)
                    .drop_column(Carts::TotalAmount)
                    .drop_column(Carts::CompletedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    Status,
    UserId,
    Notes,
    TotalAmount,
    CompletedAt,
}
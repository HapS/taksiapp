use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Cart tablosuna yeni alanları ekle
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(
                        ColumnDef::new(Carts::OrderId)
                            .string()
                            .null()
                    )
                    .add_column(
                        ColumnDef::new(Carts::PaymentUrl)
                            .string()
                            .null()
                    )
                    .add_column(
                        ColumnDef::new(Carts::Status)
                            .string()
                            .not_null()
                            .default("open_cart")
                    )
                    .add_column(
                        ColumnDef::new(Carts::CallbackData)
                            .json_binary()
                            .null()
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Alanları kaldır
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::OrderId)
                    .drop_column(Carts::PaymentUrl)
                    .drop_column(Carts::Status)
                    .drop_column(Carts::CallbackData)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    OrderId,
    PaymentUrl,
    Status,
    CallbackData,
}
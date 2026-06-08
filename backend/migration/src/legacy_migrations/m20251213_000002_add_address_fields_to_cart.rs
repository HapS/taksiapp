use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(
                        ColumnDef::new(Carts::AddressId)
                            .big_integer()
                            .null()
                    )
                    .add_column(
                        ColumnDef::new(Carts::InvoiceId)
                            .big_integer()
                            .null()
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::AddressId)
                    .drop_column(Carts::InvoiceId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    AddressId,
    InvoiceId,
}
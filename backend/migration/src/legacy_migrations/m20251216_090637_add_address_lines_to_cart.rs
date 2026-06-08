use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Cart::Table)
                    .add_column(
                        ColumnDef::new(Cart::AddressLine)
                            .text()
                            .null()
                    )
                    .add_column(
                        ColumnDef::new(Cart::InvoiceAddressLine)
                            .text()
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
                    .table(Cart::Table)
                    .drop_column(Cart::AddressLine)
                    .drop_column(Cart::InvoiceAddressLine)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Cart {
    #[sea_orm(iden = "carts")]
    Table,
    #[sea_orm(iden = "address_line")]
    AddressLine,
    #[sea_orm(iden = "invoice_address_line")]
    InvoiceAddressLine,
}
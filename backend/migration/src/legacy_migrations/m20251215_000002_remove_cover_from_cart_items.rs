use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .drop_column(CartItems::Cover)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .add_column(
                        ColumnDef::new(CartItems::Cover)
                            .string()
                            .null()
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CartItems {
    Table,
    Cover,
}
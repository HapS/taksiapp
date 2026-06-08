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
                        ColumnDef::new(Cart::OrderDate)
                            .timestamp_with_time_zone()
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
                    .drop_column(Cart::OrderDate)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Cart {
    #[sea_orm(iden = "carts")]
    Table,
    #[sea_orm(iden = "order_date")]
    OrderDate,
}
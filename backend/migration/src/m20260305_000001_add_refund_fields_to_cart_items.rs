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
                    .add_column(ColumnDef::new(CartItems::RefundStatus).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .add_column(ColumnDef::new(CartItems::RefundAmount).decimal().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .add_column(
                        ColumnDef::new(CartItems::RefundDate)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .add_column(ColumnDef::new(CartItems::RefundMethod).string().null())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .add_column(
                        ColumnDef::new(CartItems::RefundCreditId)
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_items_refund_status")
                    .table(CartItems::Table)
                    .col(CartItems::RefundStatus)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cart_items_refund_status")
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .drop_column(CartItems::RefundCreditId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .drop_column(CartItems::RefundMethod)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .drop_column(CartItems::RefundDate)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .drop_column(CartItems::RefundAmount)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .drop_column(CartItems::RefundStatus)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum CartItems {
    Table,
    RefundStatus,
    RefundAmount,
    RefundDate,
    RefundMethod,
    RefundCreditId,
}

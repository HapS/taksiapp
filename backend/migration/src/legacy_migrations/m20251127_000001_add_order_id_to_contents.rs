use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add order_id column to contents table
        manager
            .alter_table(
                Table::alter()
                    .table(Contents::Table)
                    .add_column(
                        ColumnDef::new(Contents::OrderId)
                            .integer()
                            .null()
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for order_id
        manager
            .create_index(
                Index::create()
                    .name("idx_contents_order_id")
                    .table(Contents::Table)
                    .col(Contents::OrderId)
                    .to_owned(),
            )
            .await?;

        // Create composite index for content_type + order_id
        manager
            .create_index(
                Index::create()
                    .name("idx_contents_type_order")
                    .table(Contents::Table)
                    .col(Contents::ContentType)
                    .col(Contents::OrderId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_contents_type_order")
                    .table(Contents::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_contents_order_id")
                    .table(Contents::Table)
                    .to_owned(),
            )
            .await?;

        // Drop column
        manager
            .alter_table(
                Table::alter()
                    .table(Contents::Table)
                    .drop_column(Contents::OrderId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Contents {
    Table,
    OrderId,
    ContentType,
}

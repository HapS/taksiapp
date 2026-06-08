use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add order_id to terms table
        manager
            .alter_table(
                Table::alter()
                    .table(Terms::Table)
                    .add_column(ColumnDef::new(Terms::OrderId).integer())
                    .to_owned(),
            )
            .await?;

        // Add order_id to vocabularies table
        manager
            .alter_table(
                Table::alter()
                    .table(Vocabularies::Table)
                    .add_column(ColumnDef::new(Vocabularies::OrderId).integer())
                    .to_owned(),
            )
            .await?;

        // Create index for terms.order_id
        manager
            .create_index(
                Index::create()
                    .name("idx_terms_order_id")
                    .table(Terms::Table)
                    .col(Terms::OrderId)
                    .to_owned(),
            )
            .await?;

        // Create index for vocabularies.order_id
        manager
            .create_index(
                Index::create()
                    .name("idx_vocabularies_order_id")
                    .table(Vocabularies::Table)
                    .col(Vocabularies::OrderId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes
        manager
            .drop_index(Index::drop().name("idx_terms_order_id").to_owned())
            .await?;

        manager
            .drop_index(Index::drop().name("idx_vocabularies_order_id").to_owned())
            .await?;

        // Drop columns
        manager
            .alter_table(
                Table::alter()
                    .table(Terms::Table)
                    .drop_column(Terms::OrderId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Vocabularies::Table)
                    .drop_column(Vocabularies::OrderId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Terms {
    Table,
    OrderId,
}

#[derive(DeriveIden)]
enum Vocabularies {
    Table,
    OrderId,
}

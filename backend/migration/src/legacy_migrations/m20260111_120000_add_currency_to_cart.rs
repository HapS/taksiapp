use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // cart_items tablosuna currency ve original_price alanları ekle
        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .add_column(
                        ColumnDef::new(CartItems::Currency)
                            .string()
                            .null()
                            .default("TRY"),
                    )
                    .add_column(
                        ColumnDef::new(CartItems::OriginalPrice)
                            .decimal_len(10, 2)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // carts tablosuna currency alanı ekle (sipariş para birimi)
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(
                        ColumnDef::new(Carts::Currency)
                            .string()
                            .null()
                            .default("TRY"),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CartItems::Table)
                    .drop_column(CartItems::Currency)
                    .drop_column(CartItems::OriginalPrice)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::Currency)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum CartItems {
    Table,
    Currency,
    OriginalPrice,
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    Currency,
}

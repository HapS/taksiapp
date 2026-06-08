use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add cart_type column to carts table
        // Values: 'b2c' (default) or 'b2b'
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(
                        ColumnDef::new(Carts::CartType)
                            .string()
                            .string_len(10)
                            .not_null()
                            .default("b2c")
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
                    .drop_column(Carts::CartType)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    CartType,
}

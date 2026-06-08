use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // payment_method field'ını enum'dan string'e çevir
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .modify_column(
                        ColumnDef::new(Carts::PaymentMethod)
                            .string()
                            .null()
                    )
                    .to_owned(),
            )
            .await?;

        // Enum type'ını kaldır
        manager
            .get_connection()
            .execute_unprepared("DROP TYPE IF EXISTS payment_method")
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Enum type'ını yeniden oluştur
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TYPE payment_method AS ENUM ('credit_card', 'bank_transfer', 'cash_on_delivery', 'pickup')"
            )
            .await?;

        // Field'ı enum'a çevir
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .modify_column(
                        ColumnDef::new(Carts::PaymentMethod)
                            .custom(Alias::new("payment_method"))
                            .null()
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    PaymentMethod,
}
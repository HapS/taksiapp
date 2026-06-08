use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ExchangeRates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExchangeRates::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ExchangeRates::UsdTry).decimal_len(10, 4))
                    .col(ColumnDef::new(ExchangeRates::EurTry).decimal_len(10, 4))
                    .col(ColumnDef::new(ExchangeRates::GbpTry).decimal_len(10, 4))
                    .col(ColumnDef::new(ExchangeRates::ChfTry).decimal_len(10, 4))
                    .col(ColumnDef::new(ExchangeRates::AudTry).decimal_len(10, 4))
                    .col(ColumnDef::new(ExchangeRates::CadTry).decimal_len(10, 4))
                    .col(ColumnDef::new(ExchangeRates::EurUsd).decimal_len(10, 4))
                    .col(ColumnDef::new(ExchangeRates::Source).string_len(50))
                    .col(
                        ColumnDef::new(ExchangeRates::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Index on created_at for faster queries
        manager
            .create_index(
                Index::create()
                    .name("idx_exchange_rates_created_at")
                    .table(ExchangeRates::Table)
                    .col(ExchangeRates::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ExchangeRates::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ExchangeRates {
    Table,
    Id,
    UsdTry,
    EurTry,
    GbpTry,
    ChfTry,
    AudTry,
    CadTry,
    EurUsd,
    Source,
    CreatedAt,
}

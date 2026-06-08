use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("exchange_rates"))
                    .add_column(ColumnDef::new(Alias::new("azn_try")).decimal_len(10, 4).null())
                    .add_column(ColumnDef::new(Alias::new("jpy_try")).decimal_len(10, 4).null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("exchange_rates"))
                    .drop_column(Alias::new("azn_try"))
                    .drop_column(Alias::new("jpy_try"))
                    .to_owned(),
            )
            .await
    }
}

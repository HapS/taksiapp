use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260224_000001_add_cargo_price_to_carts"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .add_column(ColumnDef::new(Carts::CargoPrice).double().null())
                    .add_column(ColumnDef::new(Carts::CargoCurrency).string().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Carts::Table)
                    .drop_column(Carts::CargoPrice)
                    .drop_column(Carts::CargoCurrency)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Carts {
    Table,
    CargoPrice,
    CargoCurrency,
}

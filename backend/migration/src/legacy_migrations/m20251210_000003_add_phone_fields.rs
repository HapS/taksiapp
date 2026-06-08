use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Address::Table)
                    .add_column(
                        ColumnDef::new(Address::PhoneCountryCode)
                            .string()
                            .not_null()
                            .default("+90"),
                    )
                    .add_column(
                        ColumnDef::new(Address::PhoneNumber)
                            .string()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Address::Table)
                    .drop_column(Address::PhoneCountryCode)
                    .drop_column(Address::PhoneNumber)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Address {
    #[sea_orm(iden = "addresses")]
    Table,
    PhoneCountryCode,
    PhoneNumber,
}

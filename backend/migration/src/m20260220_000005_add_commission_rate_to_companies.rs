use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Companies::Table)
                    .add_column(
                        ColumnDef::new(Companies::RepresentativeCommissionRate)
                            .decimal_len(5, 2)
                            .default(0.00)
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Companies::Table)
                    .drop_column(Companies::RepresentativeCommissionRate)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Companies {
    Table,
    RepresentativeCommissionRate,
}

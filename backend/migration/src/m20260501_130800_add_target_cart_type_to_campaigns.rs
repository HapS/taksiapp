use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Campaigns::Table)
                    .add_column(
                        ColumnDef::new(Campaigns::TargetCartType)
                            .string()
                            .not_null()
                            .default("both"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Campaigns::Table)
                    .drop_column(Campaigns::TargetCartType)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Campaigns {
    Table,
    TargetCartType,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add gcx column to vocabularies table
        manager
            .alter_table(
                Table::alter()
                    .table(Vocabularies::Table)
                    .add_column(
                        ColumnDef::new(Vocabularies::Gcx)
                            .boolean()
                            .not_null()
                            .default(false)
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove gcx column from vocabularies table
        manager
            .alter_table(
                Table::alter()
                    .table(Vocabularies::Table)
                    .drop_column(Vocabularies::Gcx)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Vocabularies {
    Table,
    Gcx,
}
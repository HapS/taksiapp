use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Vocabularies::Table)
                    .add_column(
                        ColumnDef::new(Vocabularies::Lock)
                            .boolean()
                            .default(false)
                            .not_null(),
                    )
                    .add_column(
                        ColumnDef::new(Vocabularies::Hide)
                            .boolean()
                            .default(false)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .alter_table(
                Table::alter()
                    .table(Vocabularies::Table)
                    .drop_column(Vocabularies::Lock)
                    .drop_column(Vocabularies::Hide)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Vocabularies {
    Table,
    Lock,
    Hide,
}

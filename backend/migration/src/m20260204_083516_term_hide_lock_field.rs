use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Terms::Table)
                    .add_column(
                        ColumnDef::new(Terms::Lock)
                            .boolean()
                            .default(false)
                            .not_null(),
                    )
                    .add_column(
                        ColumnDef::new(Terms::Hide)
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
        manager
            .alter_table(
                Table::alter()
                    .table(Terms::Table)
                    .drop_column(Terms::Lock)
                    .drop_column(Terms::Hide)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Terms {
    Table,
    Lock,
    Hide,
}

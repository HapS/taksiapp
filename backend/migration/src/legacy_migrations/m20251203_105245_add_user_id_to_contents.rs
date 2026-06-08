use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add user_id column to contents table
        manager
            .alter_table(
                Table::alter()
                    .table(Contents::Table)
                    .add_column(
                        ColumnDef::new(Contents::UserId)
                            .big_integer()
                            .null(), // Nullable for existing records
                    )
                    .to_owned(),
            )
            .await?;

        // Add foreign key constraint
        manager
            .alter_table(
                Table::alter()
                    .table(Contents::Table)
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk_contents_user_id")
                            .from_tbl(Contents::Table)
                            .from_col(Contents::UserId)
                            .to_tbl(Users::Table)
                            .to_col(Users::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Set user_id to first admin user for existing contents
        manager
            .exec_stmt(
                Query::update()
                    .table(Contents::Table)
                    .value(
                        Contents::UserId,
                        Expr::cust("(SELECT id FROM users WHERE is_admin = true LIMIT 1)"),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop foreign key
        manager
            .alter_table(
                Table::alter()
                    .table(Contents::Table)
                    .drop_foreign_key(Alias::new("fk_contents_user_id"))
                    .to_owned(),
            )
            .await?;

        // Drop column
        manager
            .alter_table(
                Table::alter()
                    .table(Contents::Table)
                    .drop_column(Contents::UserId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Contents {
    Table,
    UserId,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

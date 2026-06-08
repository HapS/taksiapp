use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Homepage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Homepage::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Homepage::Data)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .col(
                        ColumnDef::new(Homepage::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Homepage::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // İlk boş homepage kaydını oluştur
        manager
            .exec_stmt(
                Query::insert()
                    .into_table(Homepage::Table)
                    .columns([Homepage::Data])
                    .values_panic([Expr::val("[]").cast_as(Alias::new("jsonb"))])
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Homepage::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Homepage {
    Table,
    Id,
    Data,
    CreatedAt,
    UpdatedAt,
}
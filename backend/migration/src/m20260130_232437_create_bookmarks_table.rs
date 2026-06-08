use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Bookmarks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Bookmarks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Bookmarks::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Bookmarks::ModuleName).string().not_null())
                    .col(ColumnDef::new(Bookmarks::ContentType).string().not_null())
                    .col(
                        ColumnDef::new(Bookmarks::ContentId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Bookmarks::Title).string().not_null())
                    .col(ColumnDef::new(Bookmarks::Price).double())
                    .col(
                        ColumnDef::new(Bookmarks::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Bookmarks::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-bookmarks-user_id")
                            .from(Bookmarks::Table, Bookmarks::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-bookmarks-user_id")
                    .table(Bookmarks::Table)
                    .col(Bookmarks::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-bookmarks-content_id")
                    .table(Bookmarks::Table)
                    .col(Bookmarks::ContentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Bookmarks::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Bookmarks {
    Table,
    Id,
    UserId,
    ModuleName,
    ContentType,
    ContentId,
    Title,
    Price,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

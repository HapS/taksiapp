use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Comments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Comments::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Comments::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Comments::Lang).string().not_null())
                    .col(ColumnDef::new(Comments::ContentType).string().not_null())
                    .col(ColumnDef::new(Comments::ContentId).big_integer().not_null())
                    .col(ColumnDef::new(Comments::Content).text().not_null())
                    .col(
                        ColumnDef::new(Comments::Star)
                            .integer()
                            .not_null()
                            .default(5),
                    )
                    .col(ColumnDef::new(Comments::Publish).boolean().not_null().default(true))
                    .col(
                        ColumnDef::new(Comments::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Comments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Comments::DeletedAt).timestamp_with_time_zone().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-comments-user_id")
                            .from(Comments::Table, Comments::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-comments-content")
                    .table(Comments::Table)
                    .col(Comments::ContentType)
                    .col(Comments::ContentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-comments-user_id")
                    .table(Comments::Table)
                    .col(Comments::UserId)
                    .to_owned(),
            )
            .await?;

        // Add ip_address column for existing tables
        manager
            .alter_table(
                Table::alter()
                    .table(Comments::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Comments::IpAddress)
                            .string_len(45)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Comments::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Comments {
    Table,
    Id,
    UserId,
    Lang,
    ContentType,
    ContentId,
    Content,
    Star,
    Publish,
    IpAddress,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FormSubmissions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FormSubmissions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FormSubmissions::FormId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FormSubmissions::Data)
                            .json_binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FormSubmissions::Ip).string())
                    .col(ColumnDef::new(FormSubmissions::UserId).big_integer())
                    .col(
                        ColumnDef::new(FormSubmissions::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FormSubmissions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form_submissions-form_id")
                            .from(FormSubmissions::Table, FormSubmissions::FormId)
                            .to(Contents::Table, Contents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-form_submissions-user_id")
                            .from(FormSubmissions::Table, FormSubmissions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index on form_id for faster lookups
        manager
            .create_index(
                Index::create()
                    .name("idx-form_submissions-form_id")
                    .table(FormSubmissions::Table)
                    .col(FormSubmissions::FormId)
                    .to_owned(),
            )
            .await?;

        // Add index on created_at for sorting
        manager
            .create_index(
                Index::create()
                    .name("idx-form_submissions-created_at")
                    .table(FormSubmissions::Table)
                    .col(FormSubmissions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FormSubmissions::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum FormSubmissions {
    Table,
    Id,
    FormId,
    Data,
    Ip,
    UserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Contents {
    Table,
    Id,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}

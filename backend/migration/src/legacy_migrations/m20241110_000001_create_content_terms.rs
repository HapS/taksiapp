use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Content-Term many-to-many ilişki tablosu
        manager
            .create_table(
                Table::create()
                    .table(ContentTerms::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentTerms::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContentTerms::ContentId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentTerms::TermId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContentTerms::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_content_terms_content")
                            .from(ContentTerms::Table, ContentTerms::ContentId)
                            .to(Contents::Table, Contents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_content_terms_term")
                            .from(ContentTerms::Table, ContentTerms::TermId)
                            .to(Terms::Table, Terms::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint - aynı content ve term çifti bir kez olabilir
        manager
            .create_index(
                Index::create()
                    .name("idx_content_terms_unique")
                    .table(ContentTerms::Table)
                    .col(ContentTerms::ContentId)
                    .col(ContentTerms::TermId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContentTerms::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ContentTerms {
    Table,
    Id,
    ContentId,
    TermId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Contents {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Terms {
    Table,
    Id,
}

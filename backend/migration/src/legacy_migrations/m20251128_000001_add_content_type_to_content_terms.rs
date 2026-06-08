use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add content_type column
        manager
            .alter_table(
                Table::alter()
                    .table(ContentTerms::Table)
                    .add_column(
                        ColumnDef::new(ContentTerms::ContentType)
                            .string_len(50)
                            .not_null()
                            .default("page"),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_content_terms_content_type")
                    .table(ContentTerms::Table)
                    .col(ContentTerms::ContentType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_content_terms_content_id_type")
                    .table(ContentTerms::Table)
                    .col(ContentTerms::ContentId)
                    .col(ContentTerms::ContentType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes
        manager
            .drop_index(
                Index::drop()
                    .name("idx_content_terms_content_id_type")
                    .table(ContentTerms::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_content_terms_content_type")
                    .table(ContentTerms::Table)
                    .to_owned(),
            )
            .await?;

        // Drop column
        manager
            .alter_table(
                Table::alter()
                    .table(ContentTerms::Table)
                    .drop_column(ContentTerms::ContentType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ContentTerms {
    Table,
    ContentId,
    ContentType,
}

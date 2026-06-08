use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Vocabularies table
        manager
            .create_table(
                Table::create()
                    .table(Vocabularies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Vocabularies::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Vocabularies::Data).json().not_null())
                    .col(
                        ColumnDef::new(Vocabularies::VocabularyType)
                            .string()
                            .not_null()
                            .default("category"),
                    )
                    .col(
                        ColumnDef::new(Vocabularies::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Vocabularies::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Terms table
        manager
            .create_table(
                Table::create()
                    .table(Terms::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Terms::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Terms::VocabularyId).big_integer().not_null())
                    .col(ColumnDef::new(Terms::Data).json().not_null())
                    .col(ColumnDef::new(Terms::ParentId).big_integer())
                    .col(
                        ColumnDef::new(Terms::Publish)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Terms::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Terms::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_terms_vocabulary")
                            .from(Terms::Table, Terms::VocabularyId)
                            .to(Vocabularies::Table, Vocabularies::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_terms_parent")
                            .from(Terms::Table, Terms::ParentId)
                            .to(Terms::Table, Terms::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_vocabularies_type")
                    .table(Vocabularies::Table)
                    .col(Vocabularies::VocabularyType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_terms_vocabulary")
                    .table(Terms::Table)
                    .col(Terms::VocabularyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_terms_parent")
                    .table(Terms::Table)
                    .col(Terms::ParentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_terms_publish")
                    .table(Terms::Table)
                    .col(Terms::Publish)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Terms::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Vocabularies::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Vocabularies {
    Table,
    Id,
    Data,
    VocabularyType,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Terms {
    Table,
    Id,
    VocabularyId,
    Data,
    ParentId,
    Publish,
    CreatedAt,
    UpdatedAt,
}

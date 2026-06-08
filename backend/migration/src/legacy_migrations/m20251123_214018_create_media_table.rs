use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Media::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Media::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(integer(Media::UserId).not_null()) // Uploader user ID
                    .col(string(Media::FileName).not_null()) // Original file name
                    .col(string(Media::MediaType).not_null()) // e.g., image, video, audio, document
                    .col(string(Media::MimeType).not_null()) // e.g., image/png, video/mp4
                    .col(string(Media::FilePath).not_null()) // Relative path from media root
                    .col(big_integer(Media::FileSize).not_null()) // File size in bytes
                    .col(ColumnDef::new(Media::Title).string().null()) // Optional title
                    .col(ColumnDef::new(Media::Description).text().null()) // Optional description
                    .col(ColumnDef::new(Media::ContentType).string().null()) // e.g., "pages", "product"
                    .col(ColumnDef::new(Media::ContentId).big_integer().null()) // Related content ID
                    .col(
                        ColumnDef::new(Media::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(Media::UpdatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for content relationship
        manager
            .create_index(
                Index::create()
                    .name("idx_media_content")
                    .table(Media::Table)
                    .col(Media::ContentType)
                    .col(Media::ContentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {

        manager
            .drop_table(Table::drop().table(Media::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Media {
    Table,
    Id,
    UserId,
    FileName,
    MediaType,
    MimeType,
    FilePath,
    FileSize,
    Title,
    Description,
    ContentType,
    ContentId,
    CreatedAt,
    UpdatedAt,
}

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MailQueue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MailQueue::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MailQueue::TemplateName).string()) // Template adı (NULL olabilir custom mail için)
                    .col(ColumnDef::new(MailQueue::ToEmail).string().not_null())
                    .col(ColumnDef::new(MailQueue::ToName).string())
                    .col(ColumnDef::new(MailQueue::Subject).string().not_null())
                    .col(ColumnDef::new(MailQueue::Body).text().not_null())
                    .col(ColumnDef::new(MailQueue::Variables).json()) // Template değişkenleri
                    .col(ColumnDef::new(MailQueue::Language).string().default("tr")) // Hangi dilde gönderilecek
                    .col(ColumnDef::new(MailQueue::Status).string().default("pending")) // pending, sent, failed, retry
                    .col(ColumnDef::new(MailQueue::Attempts).integer().default(0))
                    .col(ColumnDef::new(MailQueue::MaxAttempts).integer().default(3))
                    .col(ColumnDef::new(MailQueue::ErrorMessage).text())
                    .col(ColumnDef::new(MailQueue::ScheduledAt).timestamp_with_time_zone()) // Zamanlanmış gönderim
                    .col(ColumnDef::new(MailQueue::SentAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(MailQueue::CreatedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(MailQueue::UpdatedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        // Index'ler
        manager
            .create_index(
                Index::create()
                    .name("idx_mail_queue_status")
                    .table(MailQueue::Table)
                    .col(MailQueue::Status)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_mail_queue_scheduled")
                    .table(MailQueue::Table)
                    .col(MailQueue::ScheduledAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_mail_queue_template_name")
                    .table(MailQueue::Table)
                    .col(MailQueue::TemplateName)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MailQueue::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MailQueue {
    Table,
    Id,
    TemplateName,
    ToEmail,
    ToName,
    Subject,
    Body,
    Variables,
    Language,
    Status,
    Attempts,
    MaxAttempts,
    ErrorMessage,
    ScheduledAt,
    SentAt,
    CreatedAt,
    UpdatedAt,
}
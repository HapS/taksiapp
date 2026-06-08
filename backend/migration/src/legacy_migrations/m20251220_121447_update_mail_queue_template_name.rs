use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Foreign key constraint'i kaldır
        manager
            .drop_foreign_key(
                ForeignKey::drop()
                    .name("fk_mail_queue_template")
                    .table(MailQueue::Table)
                    .to_owned(),
            )
            .await?;

        // template_id alanını kaldır
        manager
            .alter_table(
                Table::alter()
                    .table(MailQueue::Table)
                    .drop_column(MailQueue::TemplateId)
                    .to_owned(),
            )
            .await?;

        // template_name alanını ekle
        manager
            .alter_table(
                Table::alter()
                    .table(MailQueue::Table)
                    .add_column(ColumnDef::new(MailQueue::TemplateName).string())
                    .to_owned(),
            )
            .await?;

        // template_name için index ekle
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
        // template_name index'ini kaldır
        manager
            .drop_index(
                Index::drop()
                    .name("idx_mail_queue_template_name")
                    .table(MailQueue::Table)
                    .to_owned(),
            )
            .await?;

        // template_name alanını kaldır
        manager
            .alter_table(
                Table::alter()
                    .table(MailQueue::Table)
                    .drop_column(MailQueue::TemplateName)
                    .to_owned(),
            )
            .await?;

        // template_id alanını geri ekle
        manager
            .alter_table(
                Table::alter()
                    .table(MailQueue::Table)
                    .add_column(ColumnDef::new(MailQueue::TemplateId).big_integer())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum MailQueue {
    Table,
    TemplateId,
    TemplateName,
}
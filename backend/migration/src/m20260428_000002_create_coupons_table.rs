use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Coupons::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Coupons::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Coupons::CampaignId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Coupons::Code)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Coupons::UserId).big_integer().null())
                    .col(ColumnDef::new(Coupons::MaxUsage).integer().null())
                    .col(
                        ColumnDef::new(Coupons::UsageCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Coupons::ValidUntil)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Coupons::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Coupons::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Coupons::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_coupons_campaign")
                            .from(Coupons::Table, Coupons::CampaignId)
                            .to(Campaigns::Table, Campaigns::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_coupons_code_unique")
                    .table(Coupons::Table)
                    .col(Coupons::Code)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_coupons_campaign_id")
                    .table(Coupons::Table)
                    .col(Coupons::CampaignId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_coupons_user_id")
                    .table(Coupons::Table)
                    .col(Coupons::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_coupons_is_active")
                    .table(Coupons::Table)
                    .col(Coupons::IsActive)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Coupons::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Coupons {
    Table,
    Id,
    CampaignId,
    Code,
    UserId,
    MaxUsage,
    UsageCount,
    ValidUntil,
    IsActive,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Campaigns {
    Table,
    Id,
}
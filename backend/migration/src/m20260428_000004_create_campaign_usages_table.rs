use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CampaignUsages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CampaignUsages::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CampaignUsages::CampaignId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CampaignUsages::CouponId).big_integer().null())
                    .col(
                        ColumnDef::new(CampaignUsages::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CampaignUsages::CartId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CampaignUsages::UsedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_campaign_usages_campaign")
                            .from(CampaignUsages::Table, CampaignUsages::CampaignId)
                            .to(Campaigns::Table, Campaigns::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_campaign_usages_coupon")
                            .from(CampaignUsages::Table, CampaignUsages::CouponId)
                            .to(Coupons::Table, Coupons::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_campaign_usages_campaign_id")
                    .table(CampaignUsages::Table)
                    .col(CampaignUsages::CampaignId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_campaign_usages_user_id")
                    .table(CampaignUsages::Table)
                    .col(CampaignUsages::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_campaign_usages_cart_id")
                    .table(CampaignUsages::Table)
                    .col(CampaignUsages::CartId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CampaignUsages::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CampaignUsages {
    Table,
    Id,
    CampaignId,
    CouponId,
    UserId,
    CartId,
    UsedAt,
}

#[derive(DeriveIden)]
enum Campaigns {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Coupons {
    Table,
    Id,
}
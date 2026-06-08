use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Campaigns::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Campaigns::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Campaigns::Name).string().not_null())
                    .col(ColumnDef::new(Campaigns::Description).text().null())
                    .col(
                        ColumnDef::new(Campaigns::ScenarioType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Campaigns::Params).json().not_null())
                    .col(
                        ColumnDef::new(Campaigns::CampaignType)
                            .string()
                            .not_null()
                            .default("automatic"),
                    )
                    .col(
                        ColumnDef::new(Campaigns::StartsAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Campaigns::EndsAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Campaigns::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(Campaigns::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Campaigns::Stackable)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Campaigns::MaxUses).integer().null())
                    .col(ColumnDef::new(Campaigns::MaxUsesPerUser).integer().null())
                    .col(
                        ColumnDef::new(Campaigns::UsageCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Campaigns::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Campaigns::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_campaigns_scenario_type")
                    .table(Campaigns::Table)
                    .col(Campaigns::ScenarioType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_campaigns_is_active")
                    .table(Campaigns::Table)
                    .col(Campaigns::IsActive)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_campaigns_dates")
                    .table(Campaigns::Table)
                    .col(Campaigns::StartsAt)
                    .col(Campaigns::EndsAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Campaigns::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Campaigns {
    Table,
    Id,
    Name,
    Description,
    ScenarioType,
    Params,
    CampaignType,
    StartsAt,
    EndsAt,
    IsActive,
    Priority,
    Stackable,
    MaxUses,
    MaxUsesPerUser,
    UsageCount,
    CreatedAt,
    UpdatedAt,
}
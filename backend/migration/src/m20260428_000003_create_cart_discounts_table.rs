use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CartDiscounts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartDiscounts::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CartDiscounts::CartId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartDiscounts::CampaignId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CartDiscounts::CouponId).big_integer().null())
                    .col(
                        ColumnDef::new(CartDiscounts::ScenarioType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CartDiscounts::DiscountType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CartDiscounts::Scope).string().not_null())
                    .col(ColumnDef::new(CartDiscounts::CartItemId).big_integer().null())
                    .col(
                        ColumnDef::new(CartDiscounts::Amount)
                            .decimal_len(12, 2)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CartDiscounts::Currency)
                            .string()
                            .not_null()
                            .default("TRY"),
                    )
                    .col(ColumnDef::new(CartDiscounts::Description).string().not_null())
                    .col(
                        ColumnDef::new(CartDiscounts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_discounts_cart")
                            .from(CartDiscounts::Table, CartDiscounts::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_discounts_campaign")
                            .from(CartDiscounts::Table, CartDiscounts::CampaignId)
                            .to(Campaigns::Table, Campaigns::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_discounts_coupon")
                            .from(CartDiscounts::Table, CartDiscounts::CouponId)
                            .to(Coupons::Table, Coupons::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_discounts_cart_item")
                            .from(CartDiscounts::Table, CartDiscounts::CartItemId)
                            .to(CartItems::Table, CartItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_discounts_cart_id")
                    .table(CartDiscounts::Table)
                    .col(CartDiscounts::CartId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cart_discounts_campaign_id")
                    .table(CartDiscounts::Table)
                    .col(CartDiscounts::CampaignId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CartDiscounts::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CartDiscounts {
    Table,
    Id,
    CartId,
    CampaignId,
    CouponId,
    ScenarioType,
    DiscountType,
    Scope,
    CartItemId,
    Amount,
    Currency,
    Description,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    Id,
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

#[derive(DeriveIden)]
enum CartItems {
    Table,
    Id,
}
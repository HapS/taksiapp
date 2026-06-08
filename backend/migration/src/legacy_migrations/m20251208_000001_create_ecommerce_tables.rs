use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create carts table
        manager
            .create_table(
                Table::create()
                    .table(Carts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Carts::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(big_integer(Carts::UserId).not_null())
                    .col(
                        ColumnDef::new(Carts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(Carts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create cart_items table
        manager
            .create_table(
                Table::create()
                    .table(CartItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CartItems::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(big_integer(CartItems::CartId).not_null())
                    .col(big_integer(CartItems::ProductId).not_null())
                    .col(ColumnDef::new(CartItems::VariantKey).string().null())
                    .col(ColumnDef::new(CartItems::VariantDisplay).string().null())
                    .col(integer(CartItems::Quantity).not_null().default(1))
                    .col(double(CartItems::Price).not_null())
                    .col(
                        ColumnDef::new(CartItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(CartItems::UpdatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cart_items_cart")
                            .from(CartItems::Table, CartItems::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for cart lookup by user_id
        manager
            .create_index(
                Index::create()
                    .name("idx_carts_user_id")
                    .table(Carts::Table)
                    .col(Carts::UserId)
                    .to_owned(),
            )
            .await?;

        // Index for cart_items lookup by cart_id
        manager
            .create_index(
                Index::create()
                    .name("idx_cart_items_cart_id")
                    .table(CartItems::Table)
                    .col(CartItems::CartId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CartItems::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(Carts::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    Id,
    UserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum CartItems {
    Table,
    Id,
    CartId,
    ProductId,
    VariantKey,
    VariantDisplay,
    Quantity,
    Price,
    CreatedAt,
    UpdatedAt,
}

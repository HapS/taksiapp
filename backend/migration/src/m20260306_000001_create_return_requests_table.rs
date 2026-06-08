use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ReturnRequests::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReturnRequests::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::CartId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::CartItemId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::Quantity)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    // requested, approved, rejected, shipped, received, completed, cancelled
                    .col(
                        ColumnDef::new(ReturnRequests::Status)
                            .string()
                            .not_null()
                            .default("requested"),
                    )
                    // defective, wrong_product, not_as_described, unwanted, damaged_in_shipping, other
                    .col(ColumnDef::new(ReturnRequests::Reason).string().not_null())
                    .col(ColumnDef::new(ReturnRequests::ReasonText).text().null())
                    // JSON array of photo URLs uploaded by customer
                    .col(ColumnDef::new(ReturnRequests::Photos).json().null())
                    // Admin response/notes
                    .col(ColumnDef::new(ReturnRequests::AdminNotes).text().null())
                    // Rejection reason shown to customer
                    .col(
                        ColumnDef::new(ReturnRequests::RejectionReason)
                            .text()
                            .null(),
                    )
                    // Return shipping tracking number (entered by customer)
                    .col(
                        ColumnDef::new(ReturnRequests::ReturnCargoTrackingNo)
                            .string()
                            .null(),
                    )
                    // Return shipping company
                    .col(
                        ColumnDef::new(ReturnRequests::ReturnCargoCompany)
                            .string()
                            .null(),
                    )
                    // Refund amount (may differ from original price — partial refund, deductions etc.)
                    .col(
                        ColumnDef::new(ReturnRequests::RefundAmount)
                            .decimal()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::RefundCurrency)
                            .string()
                            .null(),
                    )
                    // Timestamps
                    .col(
                        ColumnDef::new(ReturnRequests::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::UpdatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::ApprovedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::ShippedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::ReceivedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ReturnRequests::CompletedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    // Foreign keys
                    .foreign_key(
                        ForeignKey::create()
                            .from(ReturnRequests::Table, ReturnRequests::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ReturnRequests::Table, ReturnRequests::CartItemId)
                            .to(CartItems::Table, CartItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(ReturnRequests::Table, ReturnRequests::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_return_requests_cart_id")
                    .table(ReturnRequests::Table)
                    .col(ReturnRequests::CartId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_return_requests_cart_item_id")
                    .table(ReturnRequests::Table)
                    .col(ReturnRequests::CartItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_return_requests_user_id")
                    .table(ReturnRequests::Table)
                    .col(ReturnRequests::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_return_requests_status")
                    .table(ReturnRequests::Table)
                    .col(ReturnRequests::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ReturnRequests::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum ReturnRequests {
    Table,
    Id,
    CartId,
    CartItemId,
    UserId,
    Quantity,
    Status,
    Reason,
    ReasonText,
    Photos,
    AdminNotes,
    RejectionReason,
    ReturnCargoTrackingNo,
    ReturnCargoCompany,
    RefundAmount,
    RefundCurrency,
    CreatedAt,
    UpdatedAt,
    ApprovedAt,
    ShippedAt,
    ReceivedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum CartItems {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

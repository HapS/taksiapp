use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // B2B Kredi İşlemleri Tablosu
        manager
            .create_table(
                Table::create()
                    .table(B2bCreditTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(B2bCreditTransactions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::CompanyId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::CartId)
                            .big_integer()
                            .null(), // Ödeme yapıldığında cart_id olmayabilir
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::TransactionType)
                            .string()
                            .not_null(), // 'purchase', 'payment', 'adjustment', 'refund'
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::Amount)
                            .decimal_len(15, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::Currency)
                            .string()
                            .not_null()
                            .default("TRY"),
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::BalanceBefore)
                            .decimal_len(15, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::BalanceAfter)
                            .decimal_len(15, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::Description)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::ReferenceNumber)
                            .string()
                            .null(), // Ödeme referans numarası
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::CreatedBy)
                            .big_integer()
                            .null(), // Admin user_id (manuel işlemler için)
                    )
                    .col(
                        ColumnDef::new(B2bCreditTransactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_b2b_credit_transactions_company")
                            .from(B2bCreditTransactions::Table, B2bCreditTransactions::CompanyId)
                            .to(Companies::Table, Companies::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_b2b_credit_transactions_cart")
                            .from(B2bCreditTransactions::Table, B2bCreditTransactions::CartId)
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // İndeksler
        manager
            .create_index(
                Index::create()
                    .name("idx_b2b_credit_transactions_company_id")
                    .table(B2bCreditTransactions::Table)
                    .col(B2bCreditTransactions::CompanyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_b2b_credit_transactions_cart_id")
                    .table(B2bCreditTransactions::Table)
                    .col(B2bCreditTransactions::CartId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_b2b_credit_transactions_type")
                    .table(B2bCreditTransactions::Table)
                    .col(B2bCreditTransactions::TransactionType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_b2b_credit_transactions_created_at")
                    .table(B2bCreditTransactions::Table)
                    .col(B2bCreditTransactions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(B2bCreditTransactions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum B2bCreditTransactions {
    Table,
    Id,
    CompanyId,
    CartId,
    TransactionType,
    Amount,
    Currency,
    BalanceBefore,
    BalanceAfter,
    Description,
    ReferenceNumber,
    CreatedBy,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Companies {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    Id,
}

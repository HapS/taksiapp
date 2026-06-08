use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RepresentativeCommissionTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::RepresentativeId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::CompanyId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::CartId)
                            .big_integer(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::TransactionType)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::Amount)
                            .decimal_len(15, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::OrderAmount)
                            .decimal_len(15, 2),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::CommissionRate)
                            .decimal_len(5, 2),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::Currency)
                            .string_len(3)
                            .not_null()
                            .default("TRY"),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::BalanceBefore)
                            .decimal_len(15, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::BalanceAfter)
                            .decimal_len(15, 2)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::Description)
                            .text(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::ReferenceNumber)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::CreatedBy)
                            .big_integer(),
                    )
                    .col(
                        ColumnDef::new(RepresentativeCommissionTransactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                RepresentativeCommissionTransactions::Table,
                                RepresentativeCommissionTransactions::RepresentativeId,
                            )
                            .to(CompanyRepresentatives::Table, CompanyRepresentatives::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                RepresentativeCommissionTransactions::Table,
                                RepresentativeCommissionTransactions::CompanyId,
                            )
                            .to(Companies::Table, Companies::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                RepresentativeCommissionTransactions::Table,
                                RepresentativeCommissionTransactions::CartId,
                            )
                            .to(Carts::Table, Carts::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                RepresentativeCommissionTransactions::Table,
                                RepresentativeCommissionTransactions::CreatedBy,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // İndeksler
        manager
            .create_index(
                Index::create()
                    .name("idx_rep_comm_trans_representative_id")
                    .table(RepresentativeCommissionTransactions::Table)
                    .col(RepresentativeCommissionTransactions::RepresentativeId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rep_comm_trans_company_id")
                    .table(RepresentativeCommissionTransactions::Table)
                    .col(RepresentativeCommissionTransactions::CompanyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rep_comm_trans_cart_id")
                    .table(RepresentativeCommissionTransactions::Table)
                    .col(RepresentativeCommissionTransactions::CartId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rep_comm_trans_type")
                    .table(RepresentativeCommissionTransactions::Table)
                    .col(RepresentativeCommissionTransactions::TransactionType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rep_comm_trans_created_at")
                    .table(RepresentativeCommissionTransactions::Table)
                    .col(RepresentativeCommissionTransactions::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RepresentativeCommissionTransactions::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RepresentativeCommissionTransactions {
    Table,
    Id,
    RepresentativeId,
    CompanyId,
    CartId,
    TransactionType,
    Amount,
    OrderAmount,
    CommissionRate,
    Currency,
    BalanceBefore,
    BalanceAfter,
    Description,
    ReferenceNumber,
    CreatedBy,
    CreatedAt,
}

#[derive(DeriveIden)]
enum CompanyRepresentatives {
    Table,
    Id,
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

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

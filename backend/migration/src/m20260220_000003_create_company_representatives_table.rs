use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CompanyRepresentatives::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CompanyRepresentatives::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CompanyRepresentatives::CompanyId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CompanyRepresentatives::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CompanyRepresentatives::CommissionRate)
                            .decimal_len(5, 2)
                            .not_null()
                            .default(5.00),
                    )
                    .col(
                        ColumnDef::new(CompanyRepresentatives::AccumulatedCommission)
                            .decimal_len(15, 2)
                            .default(0.00),
                    )
                    .col(
                        ColumnDef::new(CompanyRepresentatives::TotalSalesAmount)
                            .decimal_len(15, 2)
                            .default(0.00),
                    )
                    .col(
                        ColumnDef::new(CompanyRepresentatives::IsActive)
                            .boolean()
                            .default(true),
                    )
                    .col(ColumnDef::new(CompanyRepresentatives::Notes).text())
                    .col(
                        ColumnDef::new(CompanyRepresentatives::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CompanyRepresentatives::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_company_reps_company_id")
                            .from(CompanyRepresentatives::Table, CompanyRepresentatives::CompanyId)
                            .to(Companies::Table, Companies::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_company_reps_user_id")
                            .from(CompanyRepresentatives::Table, CompanyRepresentatives::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_company_reps_company_id")
                    .table(CompanyRepresentatives::Table)
                    .col(CompanyRepresentatives::CompanyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_company_reps_user_id")
                    .table(CompanyRepresentatives::Table)
                    .col(CompanyRepresentatives::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_company_reps_unique")
                    .table(CompanyRepresentatives::Table)
                    .col(CompanyRepresentatives::CompanyId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CompanyRepresentatives::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CompanyRepresentatives {
    Table,
    Id,
    CompanyId,
    UserId,
    CommissionRate,
    AccumulatedCommission,
    TotalSalesAmount,
    IsActive,
    Notes,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Companies {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

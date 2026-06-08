use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CompanyUsers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CompanyUsers::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CompanyUsers::CompanyId).big_integer().not_null())
                    .col(ColumnDef::new(CompanyUsers::UserId).big_integer().not_null())
                    .col(
                        ColumnDef::new(CompanyUsers::Role)
                            .string_len(50)
                            .default("member"),
                    )
                    .col(
                        ColumnDef::new(CompanyUsers::DiscountAdjustment)
                            .decimal_len(5, 2)
                            .default(0.00),
                    )
                    .col(ColumnDef::new(CompanyUsers::IsActive).boolean().default(true))
                    .col(
                        ColumnDef::new(CompanyUsers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CompanyUsers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_company_users_company_id")
                            .from(CompanyUsers::Table, CompanyUsers::CompanyId)
                            .to(Companies::Table, Companies::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_company_users_user_id")
                            .from(CompanyUsers::Table, CompanyUsers::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_company_users_company_id")
                    .table(CompanyUsers::Table)
                    .col(CompanyUsers::CompanyId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_company_users_user_id")
                    .table(CompanyUsers::Table)
                    .col(CompanyUsers::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_company_users_unique")
                    .table(CompanyUsers::Table)
                    .col(CompanyUsers::CompanyId)
                    .col(CompanyUsers::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CompanyUsers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CompanyUsers {
    Table,
    Id,
    CompanyId,
    UserId,
    Role,
    DiscountAdjustment,
    IsActive,
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

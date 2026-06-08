use sea_orm_migration::prelude::*;

#[derive(Iden)]
enum Addresses {
    Table,
    AddressType,
    CompanyName,
    TaxOffice,
    TaxNumber,
    IdNumber,
}

#[derive(Iden)]
enum CorporateInfos {
    Table,
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add columns to addresses table
        manager
            .alter_table(
                Table::alter()
                    .table(Addresses::Table)
                    .add_column(
                        ColumnDef::new(Addresses::AddressType)
                            .string()
                            .not_null()
                            .default("individual"),
                    )
                    .add_column(ColumnDef::new(Addresses::CompanyName).string().null())
                    .add_column(ColumnDef::new(Addresses::TaxOffice).string().null())
                    .add_column(ColumnDef::new(Addresses::TaxNumber).string().null())
                    .add_column(ColumnDef::new(Addresses::IdNumber).string().null())
                    .to_owned(),
            )
            .await?;

        // Drop corporate_infos table
        manager
            .drop_table(Table::drop().table(CorporateInfos::Table).to_owned())
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Recreate corporate_infos table (Simplified, you might want to restore original schema if needed)
        manager
            .create_table(
                Table::create()
                    .table(CorporateInfos::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("user_id"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("title")).string().not_null())
                    .col(ColumnDef::new(Alias::new("tax_office")).string().not_null())
                    .col(ColumnDef::new(Alias::new("tax_number")).string().not_null())
                    .col(
                        ColumnDef::new(Alias::new("company_name"))
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("created_at")).timestamp_with_time_zone())
                    .col(ColumnDef::new(Alias::new("updated_at")).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        // Remove columns from addresses table
        manager
            .alter_table(
                Table::alter()
                    .table(Addresses::Table)
                    .drop_column(Addresses::AddressType)
                    .drop_column(Addresses::CompanyName)
                    .drop_column(Addresses::TaxOffice)
                    .drop_column(Addresses::TaxNumber)
                    .drop_column(Addresses::IdNumber)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

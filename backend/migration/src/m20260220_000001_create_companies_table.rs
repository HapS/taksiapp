use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Companies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Companies::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // İlişkiler
                    .col(
                        ColumnDef::new(Companies::UserId)
                            .big_integer()
                            .not_null()
                            .unique_key(),
                    )
                    // Şirket Bilgileri
                    .col(ColumnDef::new(Companies::CompanyName).string_len(255).not_null())
                    .col(ColumnDef::new(Companies::TaxOffice).string_len(255))
                    .col(ColumnDef::new(Companies::TaxNumber).string_len(50))
                    .col(ColumnDef::new(Companies::TradeRegistryNo).string_len(50))
                    // İletişim (mevcut country/city/district tablolarıyla ilişkili)
                    .col(ColumnDef::new(Companies::CountryId).big_integer())
                    .col(ColumnDef::new(Companies::CityId).big_integer())
                    .col(ColumnDef::new(Companies::DistrictId).big_integer())
                    .col(ColumnDef::new(Companies::AddressLine).text())
                    .col(ColumnDef::new(Companies::PostalCode).string_len(20))
                    .col(ColumnDef::new(Companies::Phone).string_len(50))
                    .col(ColumnDef::new(Companies::Email).string_len(255))
                    .col(ColumnDef::new(Companies::Website).string_len(255))
                    // B2B Özellikleri
                    .col(
                        ColumnDef::new(Companies::DiscountPercentage)
                            .decimal_len(5, 2)
                            .default(0.00),
                    )
                    .col(
                        ColumnDef::new(Companies::CreditLimit)
                            .decimal_len(15, 2)
                            .default(0.00),
                    )
                    .col(
                        ColumnDef::new(Companies::UsedCredit)
                            .decimal_len(15, 2)
                            .default(0.00),
                    )
                    .col(ColumnDef::new(Companies::PaymentTermDays).integer().default(0))
                    .col(
                        ColumnDef::new(Companies::MinOrderAmount)
                            .decimal_len(15, 2)
                            .default(0.00),
                    )
                    // Durum
                    .col(ColumnDef::new(Companies::IsActive).boolean().default(false))
                    .col(ColumnDef::new(Companies::Notes).text())
                    // Zaman Damgaları
                    .col(
                        ColumnDef::new(Companies::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Companies::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Companies::ApprovedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Companies::ApprovedBy).big_integer())
                    // Hiyerarşi ve Temsilci
                    .col(ColumnDef::new(Companies::ParentCompanyId).big_integer())
                    .col(ColumnDef::new(Companies::RepresentativeUserId).big_integer())
                    // Foreign Keys
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_companies_user_id")
                            .from(Companies::Table, Companies::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_companies_approved_by")
                            .from(Companies::Table, Companies::ApprovedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_companies_parent_company_id")
                            .from(Companies::Table, Companies::ParentCompanyId)
                            .to(Companies::Table, Companies::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_companies_representative_user_id")
                            .from(Companies::Table, Companies::RepresentativeUserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_companies_country_id")
                            .from(Companies::Table, Companies::CountryId)
                            .to(Countries::Table, Countries::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_companies_city_id")
                            .from(Companies::Table, Companies::CityId)
                            .to(Cities::Table, Cities::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_companies_district_id")
                            .from(Companies::Table, Companies::DistrictId)
                            .to(Districts::Table, Districts::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // Indexler
        manager
            .create_index(
                Index::create()
                    .name("idx_companies_user_id")
                    .table(Companies::Table)
                    .col(Companies::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_companies_is_active")
                    .table(Companies::Table)
                    .col(Companies::IsActive)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_companies_company_name")
                    .table(Companies::Table)
                    .col(Companies::CompanyName)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_companies_parent_company_id")
                    .table(Companies::Table)
                    .col(Companies::ParentCompanyId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Companies::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Companies {
    Table,
    Id,
    UserId,
    CompanyName,
    TaxOffice,
    TaxNumber,
    TradeRegistryNo,
    CountryId,
    CityId,
    DistrictId,
    AddressLine,
    PostalCode,
    Phone,
    Email,
    Website,
    DiscountPercentage,
    CreditLimit,
    UsedCredit,
    PaymentTermDays,
    MinOrderAmount,
    IsActive,
    Notes,
    CreatedAt,
    UpdatedAt,
    ApprovedAt,
    ApprovedBy,
    ParentCompanyId,
    RepresentativeUserId,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Countries {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Cities {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Districts {
    Table,
    Id,
}

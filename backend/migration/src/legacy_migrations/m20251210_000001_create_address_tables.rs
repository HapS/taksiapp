use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create Countries Table
        manager
            .create_table(
                Table::create()
                    .table(Countries::Table)
                    .if_not_exists()
                    .col(big_integer(Countries::Id).auto_increment().primary_key())
                    .col(string(Countries::Name))
                    .col(string_null(Countries::Code))
                    .col(string_null(Countries::PhoneCode))
                    .to_owned(),
            )
            .await?;

        // Create Cities Table
        manager
            .create_table(
                Table::create()
                    .table(Cities::Table)
                    .if_not_exists()
                    .col(big_integer(Cities::Id).auto_increment().primary_key())
                    .col(big_integer(Cities::CountryId))
                    .col(string(Cities::Name))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cities_country_id")
                            .from(Cities::Table, Cities::CountryId)
                            .to(Countries::Table, Countries::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create Districts Table
        manager
            .create_table(
                Table::create()
                    .table(Districts::Table)
                    .if_not_exists()
                    .col(big_integer(Districts::Id).auto_increment().primary_key())
                    .col(big_integer(Districts::CityId))
                    .col(string(Districts::Name))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_districts_city_id")
                            .from(Districts::Table, Districts::CityId)
                            .to(Cities::Table, Cities::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create Addresses Table (Physical addresses only)
        manager
            .create_table(
                Table::create()
                    .table(Addresses::Table)
                    .if_not_exists()
                    .col(big_integer(Addresses::Id).auto_increment().primary_key())
                    .col(big_integer(Addresses::UserId))
                    //.col(string(Addresses::Title)) // User didn't object to Title here, probably still useful like "Home", "Office"
                    // Actually user said for the new one "also user_id create update vs." implying implied fields.
                    // I'll keep Title in Address as it's standard.
                    .col(string(Addresses::Title))
                    .col(big_integer(Addresses::CountryId))
                    .col(big_integer(Addresses::CityId))
                    .col(big_integer(Addresses::DistrictId))
                    .col(text(Addresses::AddressLine))
                    .col(boolean(Addresses::IsDefault).default(false))
                    .col(timestamp_with_time_zone_null(Addresses::CreatedAt))
                    .col(timestamp_with_time_zone_null(Addresses::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_addresses_user_id")
                            .from(Addresses::Table, Addresses::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_addresses_country_id")
                            .from(Addresses::Table, Addresses::CountryId)
                            .to(Countries::Table, Countries::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_addresses_city_id")
                            .from(Addresses::Table, Addresses::CityId)
                            .to(Cities::Table, Cities::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_addresses_district_id")
                            .from(Addresses::Table, Addresses::DistrictId)
                            .to(Districts::Table, Districts::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // Create CorporateInfos Table (Separate corporate details)
        manager
            .create_table(
                Table::create()
                    .table(CorporateInfos::Table)
                    .if_not_exists()
                    .col(
                        big_integer(CorporateInfos::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(big_integer(CorporateInfos::UserId))
                    .col(string(CorporateInfos::Title)) // e.g. "My Company Info"
                    .col(string(CorporateInfos::TaxOffice))
                    .col(string(CorporateInfos::TaxNumber))
                    .col(string(CorporateInfos::CompanyName))
                    .col(timestamp_with_time_zone_null(CorporateInfos::CreatedAt))
                    .col(timestamp_with_time_zone_null(CorporateInfos::UpdatedAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_corporate_infos_user_id")
                            .from(CorporateInfos::Table, CorporateInfos::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CorporateInfos::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Addresses::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Districts::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Cities::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Countries::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Countries {
    Table,
    Id,
    Name,
    Code,
    PhoneCode,
}

#[derive(DeriveIden)]
enum Cities {
    Table,
    Id,
    CountryId,
    Name,
}

#[derive(DeriveIden)]
enum Districts {
    Table,
    Id,
    CityId,
    Name,
}

#[derive(DeriveIden)]
enum Addresses {
    Table,
    Id,
    UserId,
    Title,
    CountryId,
    CityId,
    DistrictId,
    AddressLine,
    IsDefault,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum CorporateInfos {
    Table,
    Id,
    UserId,
    Title,
    TaxOffice,
    TaxNumber,
    CompanyName,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

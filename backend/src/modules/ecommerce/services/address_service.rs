use crate::modules::ecommerce::models::{
    address::{self, Entity as Address, Model as AddressModel},
    city::{self, Entity as City, Model as CityModel},
    country::{self, Entity as Country, Model as CountryModel},
    district::{self, Entity as District, Model as DistrictModel},
};
use anyhow::Result;
use sea_orm::sea_query::Expr;
use sea_orm::*;
use serde::Serialize;

#[derive(Debug, FromQueryResult, Serialize)]
pub struct AddressDTO {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub address_line: String,
    pub is_default: bool,
    pub phone_country_code: String,
    pub phone_number: String,
    pub country_id: i64,
    pub city_id: i64,
    pub district_id: i64,
    pub address_type: String,
    pub company_name: Option<String>,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub id_number: Option<String>,
    pub country_name: Option<String>,
    pub city_name: Option<String>,
    pub district_name: Option<String>,
}

pub struct AddressService;

impl AddressService {
    /// List user's addresses with country, city, and district names
    pub async fn list_addresses(db: &DatabaseConnection, user_id: i64) -> Result<Vec<AddressDTO>> {
        let addresses = Address::find()
            .filter(address::Column::UserId.eq(user_id))
            .join(JoinType::LeftJoin, address::Relation::Country.def())
            .join(JoinType::LeftJoin, address::Relation::City.def())
            .join(JoinType::LeftJoin, address::Relation::District.def())
            .select_only()
            // Select Address Columns
            .column(address::Column::Id)
            .column(address::Column::UserId)
            .column(address::Column::Title)
            .column(address::Column::AddressLine)
            .column(address::Column::IsDefault)
            .column(address::Column::PhoneCountryCode)
            .column(address::Column::PhoneNumber)
            .column(address::Column::CountryId)
            .column(address::Column::CityId)
            .column(address::Column::DistrictId)
            .column(address::Column::AddressType)
            .column(address::Column::CompanyName)
            .column(address::Column::TaxOffice)
            .column(address::Column::TaxNumber)
            .column(address::Column::IdNumber)
            // Select Joined Names with Alias
            .column_as(country::Column::Name, "country_name")
            .column_as(city::Column::Name, "city_name")
            .column_as(district::Column::Name, "district_name")
            .into_model::<AddressDTO>()
            .all(db)
            .await?;

        Ok(addresses)
    }

    /// Create new address
    pub async fn create_address(
        db: &DatabaseConnection,
        user_id: i64,
        title: String,
        country_id: i64,
        city_id: i64,
        district_id: i64,
        address_line: String,
        is_default: bool,
        phone_country_code: String,
        phone_number: String,
        address_type: String,
        company_name: Option<String>,
        tax_office: Option<String>,
        tax_number: Option<String>,
        id_number: Option<String>,
    ) -> Result<AddressModel> {
        // If is_default is true, unset other defaults
        if is_default {
            Address::update_many()
                .col_expr(address::Column::IsDefault, Expr::value(false))
                .filter(address::Column::UserId.eq(user_id))
                .exec(db)
                .await?;
        }

        let new_address = address::ActiveModel {
            user_id: Set(user_id),
            title: Set(title),
            country_id: Set(country_id),
            city_id: Set(city_id),
            district_id: Set(district_id),
            address_line: Set(address_line),
            is_default: Set(is_default),
            phone_country_code: Set(phone_country_code),
            phone_number: Set(phone_number),
            address_type: Set(address_type),
            company_name: Set(company_name),
            tax_office: Set(tax_office),
            tax_number: Set(tax_number),
            id_number: Set(id_number),
            ..Default::default()
        };

        let result = new_address.insert(db).await?;
        Ok(result)
    }

    /// Update address
    pub async fn update_address(
        db: &DatabaseConnection,
        id: i64,
        user_id: i64,
        title: Option<String>,
        country_id: Option<i64>,
        city_id: Option<i64>,
        district_id: Option<i64>,
        address_line: Option<String>,
        is_default: Option<bool>,
        phone_country_code: Option<String>,
        phone_number: Option<String>,
        address_type: Option<String>,
        company_name: Option<String>,
        tax_office: Option<String>,
        tax_number: Option<String>,
        id_number: Option<String>,
    ) -> Result<AddressModel> {
        let address = Address::find_by_id(id)
            .filter(address::Column::UserId.eq(user_id))
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Address not found"))?;

        let mut address: address::ActiveModel = address.into();

        if let Some(t) = title {
            address.title = Set(t);
        }
        if let Some(c) = country_id {
            address.country_id = Set(c);
        }
        if let Some(c) = city_id {
            address.city_id = Set(c);
        }
        if let Some(d) = district_id {
            address.district_id = Set(d);
        }
        if let Some(a) = address_line {
            address.address_line = Set(a);
        }
        if let Some(pcc) = phone_country_code {
            address.phone_country_code = Set(pcc);
        }
        if let Some(pn) = phone_number {
            address.phone_number = Set(pn);
        }
        if let Some(at) = address_type {
            address.address_type = Set(at);
        }
        if company_name.is_some() {
            address.company_name = Set(company_name);
        }
        if tax_office.is_some() {
            address.tax_office = Set(tax_office);
        }
        if tax_number.is_some() {
            address.tax_number = Set(tax_number);
        }
        if id_number.is_some() {
            address.id_number = Set(id_number);
        }

        if let Some(default) = is_default {
            address.is_default = Set(default);
            if default {
                // Unset other defaults
                Address::update_many()
                    .col_expr(address::Column::IsDefault, Expr::value(false))
                    .filter(address::Column::UserId.eq(user_id))
                    .filter(address::Column::Id.ne(id))
                    .exec(db)
                    .await?;
            }
        }

        let result = address.update(db).await?;
        Ok(result)
    }

    /// Delete address
    pub async fn delete_address(db: &DatabaseConnection, id: i64, user_id: i64) -> Result<()> {
        let result = Address::delete_many()
            .filter(address::Column::Id.eq(id))
            .filter(address::Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        if result.rows_affected == 0 {
            return Err(anyhow::anyhow!("Address not found"));
        }
        Ok(())
    }

    /// List all countries
    pub async fn list_countries(db: &DatabaseConnection) -> Result<Vec<CountryModel>> {
        let countries = Country::find()
            .order_by_asc(country::Column::Name)
            .all(db)
            .await?;
        Ok(countries)
    }

    /// List cities by country id
    pub async fn list_cities(db: &DatabaseConnection, country_id: i64) -> Result<Vec<CityModel>> {
        let cities = City::find()
            .filter(city::Column::CountryId.eq(country_id))
            .order_by_asc(city::Column::Name)
            .all(db)
            .await?;
        Ok(cities)
    }

    /// List districts by city id
    pub async fn list_districts(
        db: &DatabaseConnection,
        city_id: i64,
    ) -> Result<Vec<DistrictModel>> {
        let districts = District::find()
            .filter(district::Column::CityId.eq(city_id))
            .order_by_asc(district::Column::Name)
            .all(db)
            .await?;
        Ok(districts)
    }
}

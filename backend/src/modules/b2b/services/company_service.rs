use crate::modules::b2b::dto::company_dto::*;
use crate::modules::b2b::entities::{companies, company_users};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::*;

pub struct CompanyService;

#[derive(Debug)]
pub enum CompanyError {
    NotFound,
    AlreadyExists,
    DatabaseError(String),
    ValidationError(String),
}

impl std::fmt::Display for CompanyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompanyError::NotFound => write!(f, "Company not found"),
            CompanyError::AlreadyExists => write!(f, "Company already exists for this user"),
            CompanyError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            CompanyError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for CompanyError {}

impl From<DbErr> for CompanyError {
    fn from(err: DbErr) -> Self {
        CompanyError::DatabaseError(err.to_string())
    }
}

impl CompanyService {
    /// Yeni şirket oluştur
    pub async fn create_company(
        db: &DatabaseConnection,
        req: CompanyCreateRequest,
        _admin_user_id: i64,
    ) -> Result<CompanyResponse, CompanyError> {
        // Kullanıcının zaten şirketi var mı kontrol et
        let existing = companies::Entity::find()
            .filter(companies::Column::UserId.eq(req.user_id))
            .one(db)
            .await?;

        if existing.is_some() {
            return Err(CompanyError::AlreadyExists);
        }

        let now = Utc::now();
        let company = companies::ActiveModel {
            user_id: Set(req.user_id),
            company_name: Set(req.company_name),
            tax_office: Set(req.tax_office),
            tax_number: Set(req.tax_number),
            trade_registry_no: Set(req.trade_registry_no),
            country_id: Set(req.country_id),
            city_id: Set(req.city_id),
            district_id: Set(req.district_id),
            address_line: Set(req.address_line),
            postal_code: Set(req.postal_code),
            phone: Set(req.phone),
            email: Set(req.email),
            website: Set(req.website),
            logo: Set(req.logo),
            discount_percentage: Set(Decimal::ZERO),
            credit_limit: Set(Decimal::ZERO),
            used_credit: Set(Decimal::ZERO),
            payment_term_days: Set(0),
            min_order_amount: Set(Decimal::ZERO),
            currency: Set(req.currency),
            is_active: Set(false), // Admin onayı bekliyor
            notes: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            approved_at: Set(None),
            approved_by: Set(None),
            parent_company_id: Set(None),
            ..Default::default()
        };

        let result = company.insert(db).await?;

        // Kullanıcıyı company_users tablosuna ekle
        let company_user = company_users::ActiveModel {
            company_id: Set(result.id),
            user_id: Set(req.user_id),
            role: Set("admin".to_string()),
            discount_adjustment: Set(Decimal::ZERO),
            is_active: Set(true),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        company_user.insert(db).await?;

        Self::get_company_by_id(db, result.id)
            .await?
            .ok_or(CompanyError::NotFound)
    }

    /// Şirket detayını getir
    pub async fn get_company_by_id(
        db: &DatabaseConnection,
        id: i64,
    ) -> Result<Option<CompanyResponse>, CompanyError> {
        let company = companies::Entity::find_by_id(id)
            .find_also_related(crate::modules::ecommerce::models::country::Entity)
            .one(db)
            .await?;

        if let Some((company, country)) = company {
            let city = if let Some(city_id) = company.city_id {
                crate::modules::ecommerce::models::city::Entity::find_by_id(city_id)
                    .one(db)
                    .await?
            } else {
                None
            };

            let district = if let Some(district_id) = company.district_id {
                crate::modules::ecommerce::models::district::Entity::find_by_id(district_id)
                    .one(db)
                    .await?
            } else {
                None
            };

            let parent_company = if let Some(parent_id) = company.parent_company_id {
                companies::Entity::find_by_id(parent_id).one(db).await?
            } else {
                None
            };

            // Get user email
            let user = crate::modules::auth::models::user::Entity::find_by_id(company.user_id)
                .one(db)
                .await?;

            let available_credit = company.available_credit();
            
            // Format prices
            let currency = company.currency.as_deref().unwrap_or("TRY");
            let credit_limit_formatted = crate::modules::utils::format_price::format_price(
                company.credit_limit.to_string().parse::<f64>().unwrap_or(0.0),
                currency
            );
            let used_credit_formatted = crate::modules::utils::format_price::format_price(
                company.used_credit.to_string().parse::<f64>().unwrap_or(0.0),
                currency
            );
            let available_credit_formatted = crate::modules::utils::format_price::format_price(
                available_credit.to_string().parse::<f64>().unwrap_or(0.0),
                currency
            );
            let min_order_amount_formatted = crate::modules::utils::format_price::format_price(
                company.min_order_amount.to_string().parse::<f64>().unwrap_or(0.0),
                currency
            );

            Ok(Some(CompanyResponse {
                id: company.id,
                user_id: company.user_id,
                user_email: user.map(|u| u.email),
                company_name: company.company_name.clone(),
                tax_office: company.tax_office.clone(),
                tax_number: company.tax_number.clone(),
                trade_registry_no: company.trade_registry_no.clone(),
                country_id: company.country_id,
                city_id: company.city_id,
                district_id: company.district_id,
                address_line: company.address_line.clone(),
                postal_code: company.postal_code.clone(),
                phone: company.phone.clone(),
                email: company.email.clone(),
                website: company.website.clone(),
                logo: company.logo.clone(),
                discount_percentage: company.discount_percentage,
                credit_limit: company.credit_limit,
                used_credit: company.used_credit,
                available_credit,
                credit_limit_formatted,
                used_credit_formatted,
                available_credit_formatted,
                payment_term_days: company.payment_term_days,
                min_order_amount: company.min_order_amount,
                min_order_amount_formatted,
                is_active: company.is_active,
                notes: company.notes.clone(),
                created_at: company.created_at.to_string(),
                updated_at: company.updated_at.to_string(),
                approved_at: company.approved_at.map(|d| d.to_string()),
                approved_by: company.approved_by,
                parent_company_id: company.parent_company_id,
                currency: company.currency.clone(),
                country_name: country.map(|c| c.name),
                city_name: city.map(|c| c.name),
                district_name: district.map(|d| d.name),
                parent_company_name: parent_company.map(|p| p.company_name),
            }))
        } else {
            Ok(None)
        }
    }

    /// Kullanıcıya göre şirket getir
    pub async fn get_company_by_user_id(
        db: &DatabaseConnection,
        user_id: i64,
    ) -> Result<Option<CompanyResponse>, CompanyError> {
        let company = companies::Entity::find()
            .filter(companies::Column::UserId.eq(user_id))
            .one(db)
            .await?;

        if let Some(company) = company {
            Self::get_company_by_id(db, company.id).await
        } else {
            Ok(None)
        }
    }

    /// Şirket listesi (pagination ile)
    pub async fn list_companies(
        db: &DatabaseConnection,
        page: u64,
        per_page: u64,
        search: Option<String>,
        is_active: Option<bool>,
    ) -> Result<(Vec<CompanyListResponse>, u64), CompanyError> {
        use crate::modules::utils::format_price::format_price;
        use rust_decimal::prelude::ToPrimitive;

        let mut query = companies::Entity::find();

        if let Some(search_term) = search {
            query = query.filter(
                Condition::any()
                    .add(companies::Column::CompanyName.contains(&search_term))
                    .add(companies::Column::TaxNumber.contains(&search_term))
                    .add(companies::Column::Email.contains(&search_term)),
            );
        }

        if let Some(active) = is_active {
            query = query.filter(companies::Column::IsActive.eq(active));
        }

        let paginator = query
            .order_by_desc(companies::Column::CreatedAt)
            .paginate(db, per_page);

        let total = paginator.num_items().await?;
        let companies = paginator.fetch_page(page - 1).await?;

        let mut results = Vec::new();
        for company in companies {
            let city = if let Some(city_id) = company.city_id {
                crate::modules::ecommerce::models::city::Entity::find_by_id(city_id)
                    .one(db)
                    .await?
            } else {
                None
            };

            // Kredi bilgilerini hesapla
            let available_credit = company.credit_limit - company.used_credit;

            let currency = company.currency.as_deref().unwrap_or("TRY");
            let credit_limit_f64 = company.credit_limit.to_f64().unwrap_or(0.0);
            let used_credit_f64 = company.used_credit.to_f64().unwrap_or(0.0);
            let available_credit_f64 = available_credit.to_f64().unwrap_or(0.0);

            results.push(CompanyListResponse {
                id: company.id,
                company_name: company.company_name,
                tax_number: company.tax_number,
                email: company.email,
                phone: company.phone,
                discount_percentage: company.discount_percentage,
                is_active: company.is_active,
                created_at: company.created_at.to_string(),
                city_name: city.map(|c| c.name),
                credit_limit: company.credit_limit,
                used_credit: company.used_credit,
                available_credit,
                credit_limit_formatted: format_price(credit_limit_f64, currency),
                used_credit_formatted: format_price(used_credit_f64, currency),
                available_credit_formatted: format_price(available_credit_f64, currency),
                currency: company.currency.clone(),
            });
        }

        Ok((results, total))
    }

    /// Şirket güncelle
    pub async fn update_company(
        db: &DatabaseConnection,
        id: i64,
        req: CompanyUpdateRequest,
    ) -> Result<CompanyResponse, CompanyError> {
        let company = companies::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(CompanyError::NotFound)?;

        let mut active_model: companies::ActiveModel = company.into();

        if let Some(name) = req.company_name {
            active_model.company_name = Set(name);
        }
        if let Some(tax_office) = req.tax_office {
            active_model.tax_office = Set(Some(tax_office));
        }
        if let Some(tax_number) = req.tax_number {
            active_model.tax_number = Set(Some(tax_number));
        }
        if let Some(trade_registry_no) = req.trade_registry_no {
            active_model.trade_registry_no = Set(Some(trade_registry_no));
        }
        if let Some(country_id) = req.country_id {
            active_model.country_id = Set(Some(country_id));
        }
        if let Some(city_id) = req.city_id {
            active_model.city_id = Set(Some(city_id));
        }
        if let Some(district_id) = req.district_id {
            active_model.district_id = Set(Some(district_id));
        }
        if let Some(address_line) = req.address_line {
            active_model.address_line = Set(Some(address_line));
        }
        if let Some(postal_code) = req.postal_code {
            active_model.postal_code = Set(Some(postal_code));
        }
        if let Some(phone) = req.phone {
            active_model.phone = Set(Some(phone));
        }
        if let Some(email) = req.email {
            active_model.email = Set(Some(email));
        }
        if let Some(website) = req.website {
            active_model.website = Set(Some(website));
        }
        if let Some(logo) = req.logo {
            active_model.logo = Set(Some(logo));
        }
        if let Some(currency) = req.currency {
            active_model.currency = Set(Some(currency));
        }

        active_model.updated_at = Set(Utc::now().into());

        let updated = active_model.update(db).await?;

        Self::get_company_by_id(db, updated.id)
            .await?
            .ok_or(CompanyError::NotFound)
    }

    /// Admin tarafından şirket güncelle
    pub async fn admin_update_company(
        db: &DatabaseConnection,
        id: i64,
        req: CompanyAdminUpdateRequest,
    ) -> Result<CompanyResponse, CompanyError> {
        let company = companies::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(CompanyError::NotFound)?;

        // Kredi limiti düşürülüyorsa, kullanılan krediden az olamaz kontrolü
        if let Some(new_credit_limit) = req.credit_limit {
            if new_credit_limit < company.used_credit {
                return Err(CompanyError::ValidationError(format!(
                    "Kredi limiti kullanılan krediden ({}) az olamaz. Yeni limit: {}",
                    company.used_credit, new_credit_limit
                )));
            }
        }

        let mut active_model: companies::ActiveModel = company.into();

        if let Some(discount) = req.discount_percentage {
            active_model.discount_percentage = Set(discount);
        }
        if let Some(credit_limit) = req.credit_limit {
            active_model.credit_limit = Set(credit_limit);
        }
        if let Some(payment_term) = req.payment_term_days {
            active_model.payment_term_days = Set(payment_term);
        }
        if let Some(min_order) = req.min_order_amount {
            active_model.min_order_amount = Set(min_order);
        }
        if let Some(is_active) = req.is_active {
            active_model.is_active = Set(is_active);
        }
        if let Some(notes) = req.notes {
            active_model.notes = Set(Some(notes));
        }
        if let Some(parent_id) = req.parent_company_id {
            active_model.parent_company_id = Set(Some(parent_id));
        }
        if let Some(currency) = req.currency {
            active_model.currency = Set(Some(currency));
        }

        active_model.updated_at = Set(Utc::now().into());

        let updated = active_model.update(db).await?;

        Self::get_company_by_id(db, updated.id)
            .await?
            .ok_or(CompanyError::NotFound)
    }

    /// Şirket onayla
    pub async fn approve_company(
        db: &DatabaseConnection,
        id: i64,
        approved_by: i64,
    ) -> Result<CompanyResponse, CompanyError> {
        let company = companies::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(CompanyError::NotFound)?;

        let mut active_model: companies::ActiveModel = company.into();
        let now = Utc::now();

        active_model.is_active = Set(true);
        active_model.approved_at = Set(Some(now.into()));
        active_model.approved_by = Set(Some(approved_by));
        active_model.updated_at = Set(now.into());

        let updated = active_model.update(db).await?;

        Self::get_company_by_id(db, updated.id)
            .await?
            .ok_or(CompanyError::NotFound)
    }

    /// Şirket aktif/pasif değiştir
    pub async fn toggle_active(
        db: &DatabaseConnection,
        id: i64,
    ) -> Result<CompanyResponse, CompanyError> {
        let company = companies::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(CompanyError::NotFound)?;

        let mut active_model: companies::ActiveModel = company.into();
        active_model.is_active = Set(!active_model.is_active.unwrap());
        active_model.updated_at = Set(Utc::now().into());

        let updated = active_model.update(db).await?;

        Self::get_company_by_id(db, updated.id)
            .await?
            .ok_or(CompanyError::NotFound)
    }

    /// Şirket sil
    pub async fn delete_company(
        db: &DatabaseConnection,
        id: i64,
    ) -> Result<(), CompanyError> {
        let company = companies::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or(CompanyError::NotFound)?;

        company.delete(db).await?;

        Ok(())
    }
}

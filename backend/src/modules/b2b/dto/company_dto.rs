use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

//b2b için sadece company_create_request ve company_update_request dto'ları kullanılacak

#[derive(Debug, Deserialize)]
pub struct CompanyCreateRequest {
    pub user_id: i64,
    pub company_name: String,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub trade_registry_no: Option<String>,
    pub country_id: Option<i64>,
    pub city_id: Option<i64>,
    pub district_id: Option<i64>,
    pub address_line: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub logo: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyUpdateRequest {
    pub company_name: Option<String>,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub trade_registry_no: Option<String>,
    pub country_id: Option<i64>,
    pub city_id: Option<i64>,
    pub district_id: Option<i64>,
    pub address_line: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub logo: Option<String>,
    pub currency: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompanyAdminUpdateRequest {
    pub discount_percentage: Option<Decimal>,
    pub credit_limit: Option<Decimal>,
    pub payment_term_days: Option<i32>,
    pub min_order_amount: Option<Decimal>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
    pub parent_company_id: Option<i64>,
    pub currency: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompanyResponse {
    pub id: i64,
    pub user_id: i64,
    pub user_email: Option<String>,
    pub company_name: String,
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub trade_registry_no: Option<String>,
    pub country_id: Option<i64>,
    pub city_id: Option<i64>,
    pub district_id: Option<i64>,
    pub address_line: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub website: Option<String>,
    pub logo: Option<String>,
    pub discount_percentage: Decimal,
    pub credit_limit: Decimal,
    pub used_credit: Decimal,
    pub available_credit: Decimal,
    pub credit_limit_formatted: String,
    pub used_credit_formatted: String,
    pub available_credit_formatted: String,
    pub payment_term_days: i32,
    pub min_order_amount: Decimal,
    pub min_order_amount_formatted: String,
    pub is_active: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub approved_at: Option<String>,
    pub approved_by: Option<i64>,
    pub parent_company_id: Option<i64>,
    pub currency: Option<String>,
    // İlişkili veriler
    pub country_name: Option<String>,
    pub city_name: Option<String>,
    pub district_name: Option<String>,
    pub parent_company_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompanyListResponse {
    pub id: i64,
    pub company_name: String,
    pub tax_number: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub discount_percentage: Decimal,
    pub is_active: bool,
    pub created_at: String,
    pub city_name: Option<String>,
    // Kredi bilgileri
    pub credit_limit: Decimal,
    pub used_credit: Decimal,
    pub available_credit: Decimal,
    pub credit_limit_formatted: String,
    pub used_credit_formatted: String,
    pub available_credit_formatted: String,
    pub currency: Option<String>,
}

// #[derive(Debug, Deserialize)]
// pub struct CompanyUserCreateRequest {
//     pub company_id: i64,
//     pub user_id: i64,
//     pub role: Option<String>,
//     pub discount_adjustment: Option<Decimal>,
// }

// #[derive(Debug, Serialize)]
// pub struct CompanyUserResponse {
//     pub id: i64,
//     pub company_id: i64,
//     pub user_id: i64,
//     pub role: String,
//     pub discount_adjustment: Decimal,
//     pub is_active: bool,
//     pub user_name: Option<String>,
//     pub user_email: Option<String>,
// }

// #[derive(Debug, Deserialize)]
// pub struct RepresentativeCreateRequest {
//     pub company_id: i64,
//     pub user_id: i64,
//     pub commission_rate: Decimal,
//     pub notes: Option<String>,
// }

// #[derive(Debug, Serialize)]
// pub struct RepresentativeResponse {
//     pub id: i64,
//     pub company_id: i64,
//     pub user_id: i64,
//     pub commission_rate: Decimal,
//     pub accumulated_commission: Decimal,
//     pub total_sales_amount: Decimal,
//     pub is_active: bool,
//     pub notes: Option<String>,
//     pub user_name: Option<String>,
//     pub company_name: Option<String>,
// }

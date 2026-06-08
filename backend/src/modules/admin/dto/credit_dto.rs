use rust_decimal::Decimal;
use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreditTransactionResponse {
    pub id: i64,
    pub company_id: i64,
    pub company_name: String,
    pub cart_id: Option<i64>,
    pub order_id: Option<String>,
    pub transaction_type: String,
    pub transaction_type_display: String,
    pub amount: String, // Formatlanmış
    pub amount_raw: f64,
    pub currency: String,
    pub balance_before: String, // Formatlanmış
    pub balance_after: String,  // Formatlanmış
    pub description: Option<String>,
    pub reference_number: Option<String>,
    pub created_by: Option<i64>,
    pub created_by_name: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub created_at_formatted: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    pub company_id: i64,
    pub amount: Decimal,
    pub reference_number: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAdjustmentRequest {
    pub company_id: i64,
    pub amount: Decimal, // Pozitif veya negatif
    pub currency: Option<String>,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct CompanyCreditSummary {
    pub company_id: i64,
    pub company_name: String,
    pub tax_number: Option<String>,
    pub contact_person: Option<String>,
    pub contact_email: Option<String>,
    pub credit_limit: String,
    pub credit_limit_raw: f64,
    pub used_credit: String,
    pub used_credit_raw: f64,
    pub available_credit: String,
    pub available_credit_raw: f64,
    pub currency: String,
    pub total_purchases: String,
    pub total_payments: String,
    pub total_transactions: i64,
    pub total_adjustments: String,
}

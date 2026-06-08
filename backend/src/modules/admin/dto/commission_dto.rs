use rust_decimal::Decimal;
use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommissionTransactionResponse {
    pub id: i64,
    pub representative_id: i64,
    pub representative_name: String,
    pub company_id: i64,
    pub company_name: String,
    pub cart_id: Option<i64>,
    pub order_id: Option<String>,
    pub transaction_type: String,
    pub transaction_type_display: String,
    pub amount: String, // Formatlanmış
    pub amount_raw: f64,
    pub order_amount: Option<String>,
    pub commission_rate: Option<String>,
    pub currency: String,
    pub balance_before: String,
    pub balance_after: String,
    pub description: Option<String>,
    pub reference_number: Option<String>,
    pub created_by: Option<i64>,
    pub created_by_name: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub created_at_formatted: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommissionPaymentRequest {
    pub representative_id: i64,
    pub amount: Decimal,
    pub reference_number: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommissionAdjustmentRequest {
    pub representative_id: i64,
    pub amount: Decimal, // Pozitif veya negatif
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeCommissionSummary {
    pub representative_id: i64,
    pub representative_name: String,
    pub representative_email: Option<String>,
    pub company_id: i64,
    pub company_name: String,
    pub commission_rate: String,
    pub commission_rate_raw: f64,
    pub accumulated_commission: String,
    pub accumulated_commission_raw: f64,
    pub total_sales_amount: String,
    pub total_sales_amount_raw: f64,
    pub currency: String,
    pub total_earned: String,
    pub total_paid: String,
    pub pending_commission: String,
    pub pending_commission_raw: f64,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeListResponse {
    pub id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub user_email: String,
    pub company_id: i64,
    pub company_name: String,
    pub commission_rate: Decimal,
    pub commission_rate_formatted: String,
    pub accumulated_commission: Decimal,
    pub accumulated_commission_formatted: String,
    pub total_sales_amount: Decimal,
    pub total_sales_amount_formatted: String,
    pub is_active: bool,
    pub created_at: String,
}

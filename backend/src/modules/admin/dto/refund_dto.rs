use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Toplu B2B krediye iade request
#[derive(Debug, Deserialize)]
pub struct BulkRefundToB2BCreditRequest {
    pub cart_id: i64,
    pub description: Option<String>,
}

/// Toplu banka iadesi request
#[derive(Debug, Deserialize)]
pub struct BulkMarkBankRefundedRequest {
    pub cart_id: i64,
    pub payment_method: String, // bank_transfer, credit_card
    #[allow(dead_code)]
    pub description: Option<String>,
}

/// Toplu iade response
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct BulkRefundResponse {
    pub refunded_count: i32,
    pub total_refunded: Decimal,
    pub failed_items: Vec<BulkRefundFailedItem>,
}

/// Toplu iadede başarısız olan item
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct BulkRefundFailedItem {
    pub cart_item_id: i64,
    pub error: String,
}

/// Kullanıcı kredisi response
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct UserCreditResponse {
    pub id: i64,
    pub user_id: i64,
    pub credit_type: String,
    pub amount: String, // Formatted
    pub amount_raw: f64,
    pub remaining_amount: String, // Formatted
    pub remaining_amount_raw: f64,
    pub currency: String,
    pub description: Option<String>,
    pub min_order_amount: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    pub status: String,
    pub created_at: Option<String>,
}

/// Sepet için uygulanabilir krediler
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ApplicableCreditResponse {
    pub id: i64,
    pub amount: String,
    pub amount_raw: f64,
    pub description: Option<String>,
    pub credit_type: String,
}

/// Sepet toplam hesaplama (kredilerle)
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CartTotalWithCreditsResponse {
    pub original_total: String,
    pub original_total_raw: f64,
    pub applied_credits: Vec<ApplicableCreditResponse>,
    pub total_credit_discount: String,
    pub total_credit_discount_raw: f64,
    pub final_total: String,
    pub final_total_raw: f64,
}

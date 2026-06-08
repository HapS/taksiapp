use serde::{Deserialize, Serialize};

/// Payment Provider türleri
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PaymentProviderType {
    #[serde(rename = "iyzico")]
    Iyzico,
    #[serde(rename = "garanti")]
    Garanti,
    #[serde(rename = "paytr")]
    PayTR,
}

impl PaymentProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentProviderType::Iyzico => "iyzico",
            PaymentProviderType::Garanti => "garanti",
            PaymentProviderType::PayTR => "paytr",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "iyzico" => Some(PaymentProviderType::Iyzico),
            "garanti" => Some(PaymentProviderType::Garanti),
            "paytr" => Some(PaymentProviderType::PayTR),
            _ => None,
        }
    }
}

/// Payment Provider konfigürasyonu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentProviderConfig {
    pub provider_type: PaymentProviderType,
    pub enabled: bool,
    pub test_mode: bool,
    pub config: serde_json::Value, // Provider-specific configuration
}

/// İyzico konfigürasyonu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IyzicoConfig {
    pub api_key: String,
    pub secret_key: String,
    pub base_url: String, // Test: https://sandbox-api.iyzipay.com, Live: https://api.iyzipay.com
}

/// Garanti Bankası konfigürasyonu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GarantiConfig {
    pub terminal_id: String,
    pub merchant_id: String,
    pub user_id: String,   // provUserID
    pub password: String,  // provUserPassword
    pub store_key: String, // storeKey
    pub base_url: String,  // Test ve live URL'ler
}

/// PayTR konfigürasyonu
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaytrConfig {
    pub merchant_id: String,
    pub merchant_key: String,
    pub merchant_salt: String,
    pub base_url: String, // https://www.paytr.com/odeme/api
}

/// Payment request data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequest {
    pub order_id: String,
    pub amount: f64,
    pub currency: String,
    pub customer_name: String,
    pub customer_email: String,
    pub customer_phone: String,
    pub customer_id: String,
    pub customer_ip: String,
    pub customer_identity_number: Option<String>,
    pub customer_city: Option<String>,
    pub customer_country: Option<String>,
    pub customer_address: Option<String>,
    pub customer_zip_code: Option<String>,
    // Corporate info
    pub invoice_type: Option<String>, // "individual" or "corporate"
    pub tax_office: Option<String>,
    pub tax_number: Option<String>,
    pub company_name: Option<String>,

    pub success_url: String,
    pub failure_url: String,
    pub callback_url: String,
    pub basket_items: Vec<BasketItem>,
}

/// Sepet ürünü
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasketItem {
    pub id: String,
    pub name: String,
    pub category1: String,
    pub category2: Option<String>,
    pub item_type: String, // PHYSICAL, VIRTUAL
    pub price: String,
}

/// Payment response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentResponse {
    pub success: bool,
    pub payment_url: Option<String>,
    pub token: Option<String>,
    pub error_message: Option<String>,
    pub provider_response: serde_json::Value,
}

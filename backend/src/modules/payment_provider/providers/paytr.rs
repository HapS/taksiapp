use crate::modules::payment_provider::models::*;
use crate::modules::payment_provider::services::PaymentProviderError;
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use reqwest;
use serde_json::{self, json, Value};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct PaytrProvider;

impl PaytrProvider {
    /// PayTR iframe token oluştur
    pub async fn initiate_payment(
        config: PaytrConfig,
        request: PaymentRequest,
        test_mode: bool,
    ) -> Result<PaymentResponse, PaymentProviderError> {
        let client = reqwest::Client::new();

        // merchant_oid = order_id (zaten alfanümerik olmalı)
        let merchant_oid = &request.order_id;

        // User basket oluştur - base64 encoded JSON array
        let user_basket = Self::create_user_basket(&request.basket_items);
        
        // Tutarı kuruş cinsine çevir (9.99 -> 999)
        let payment_amount = ((request.amount * 100.0) as i64).to_string();
        
        // Para birimi
        let currency = match request.currency.to_uppercase().as_str() {
            "TRY" | "TL" => "TL",
            "USD" => "USD",
            "EUR" => "EUR",
            "GBP" => "GBP",
            _ => "TL",
        };

        // Taksit ve test mode ayarları
        let no_installment = "0"; // Taksit açık
        let max_installment = "0"; // Maksimum taksit sınırı yok
        let test_mode_str = if test_mode { "1" } else { "0" };
        let timeout_limit = "30"; // 30 dakika
        let debug_on = if test_mode { "1" } else { "0" };

        // Hash string oluştur - PayTR dokümantasyonuna göre
        let hash_str = format!(
            "{}{}{}{}{}{}{}{}{}{}",
            config.merchant_id,
            request.customer_ip,
            merchant_oid,
            request.customer_email,
            payment_amount,
            user_basket,
            no_installment,
            max_installment,
            currency,
            test_mode_str
        );

        eprintln!("PayTR hash string: {}", hash_str);

        // HMAC-SHA256 hash oluştur
        let paytr_token = Self::create_token(&hash_str, &config.merchant_salt, &config.merchant_key)?;

        eprintln!("PayTR token created: {}", paytr_token);

        // POST parametreleri hazırla
        let params = [
            ("merchant_id", config.merchant_id.as_str()),
            ("user_ip", request.customer_ip.as_str()),
            ("merchant_oid", merchant_oid.as_str()),
            ("email", request.customer_email.as_str()),
            ("payment_amount", payment_amount.as_str()),
            ("paytr_token", paytr_token.as_str()),
            ("user_basket", user_basket.as_str()),
            ("debug_on", debug_on),
            ("no_installment", no_installment),
            ("max_installment", max_installment),
            ("user_name", request.customer_name.as_str()),
            ("user_address", request.customer_address.as_deref().unwrap_or("Adres bilgisi yok")),
            ("user_phone", request.customer_phone.as_str()),
            ("merchant_ok_url", request.success_url.as_str()),
            ("merchant_fail_url", request.failure_url.as_str()),
            ("timeout_limit", timeout_limit),
            ("currency", currency),
            ("test_mode", test_mode_str),
        ];

        eprintln!("PayTR request params: {:?}", params);

        // API URL
        let api_url = if config.base_url.is_empty() {
            "https://www.paytr.com/odeme/api/get-token".to_string()
        } else if config.base_url.ends_with("/get-token") {
            config.base_url.clone()
        } else {
            format!("{}/get-token", config.base_url.trim_end_matches('/'))
        };

        eprintln!("PayTR API URL: {}", api_url);

        // API çağrısı
        let response = match client
            .post(&api_url)
            .form(&params)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
        {
            Ok(resp) => {
                eprintln!("PayTR response status: {}", resp.status());
                resp
            }
            Err(e) => {
                eprintln!("PayTR request error: {:?}", e);
                return Err(PaymentProviderError::ProviderError(format!(
                    "PayTR API bağlantı hatası: {}",
                    e
                )));
            }
        };

        let response_text = match response.text().await {
            Ok(text) => {
                eprintln!("PayTR response text: {}", text);
                text
            }
            Err(e) => {
                eprintln!("PayTR response read error: {:?}", e);
                return Err(PaymentProviderError::ProviderError(format!(
                    "PayTR yanıt okuma hatası: {}",
                    e
                )));
            }
        };

        // JSON parse
        let response_json: Value = serde_json::from_str(&response_text).map_err(|e| {
            PaymentProviderError::ProviderError(format!(
                "PayTR JSON parse hatası: {} - Response: {}",
                e, response_text
            ))
        })?;

        // Status kontrolü
        if response_json.get("status").and_then(|s| s.as_str()) == Some("success") {
            let token = response_json
                .get("token")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());

            if let Some(ref t) = token {
                // iframe URL oluştur
                let iframe_url = format!("https://www.paytr.com/odeme/guvenli/{}", t);
                
                eprintln!("PayTR iframe URL: {}", iframe_url);

                Ok(PaymentResponse {
                    success: true,
                    payment_url: Some(iframe_url),
                    token: token,
                    error_message: None,
                    provider_response: response_json,
                })
            } else {
                Err(PaymentProviderError::ProviderError(
                    "PayTR token alınamadı".to_string(),
                ))
            }
        } else {
            let reason = response_json
                .get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("Bilinmeyen hata");

            eprintln!("PayTR error: {}", reason);

            Ok(PaymentResponse {
                success: false,
                payment_url: None,
                token: None,
                error_message: Some(reason.to_string()),
                provider_response: response_json,
            })
        }
    }

    /// HMAC-SHA256 token oluştur
    fn create_token(
        hash_str: &str,
        merchant_salt: &str,
        merchant_key: &str,
    ) -> Result<String, PaymentProviderError> {
        // hash_str + merchant_salt
        let data = format!("{}{}", hash_str, merchant_salt);

        // HMAC-SHA256
        let mut mac = HmacSha256::new_from_slice(merchant_key.as_bytes())
            .map_err(|e| PaymentProviderError::ProviderError(format!("HMAC key hatası: {}", e)))?;

        mac.update(data.as_bytes());
        let result = mac.finalize();
        let hash_bytes = result.into_bytes();

        // Base64 encode
        let token = STANDARD.encode(hash_bytes);

        Ok(token)
    }

    /// User basket JSON oluştur
    fn create_user_basket(items: &[BasketItem]) -> String {
        // Basket format: [[name, price, quantity], ...]
        let basket: Vec<Vec<Value>> = items
            .iter()
            .map(|item| {
                vec![
                    json!(item.name),
                    json!(item.price),
                    json!(1), // Quantity - her item 1 adet olarak kabul edilir
                ]
            })
            .collect();

        // Eğer basket boşsa varsayılan bir ürün ekle
        let basket = if basket.is_empty() {
            vec![vec![json!("Sipariş"), json!("0.00"), json!(1)]]
        } else {
            basket
        };

        // Base64 encode
        let json_str = serde_json::to_string(&basket).unwrap_or_else(|_| "[]".to_string());
        STANDARD.encode(json_str.as_bytes())
    }

    /// PayTR callback hash doğrulama
    pub fn verify_callback_hash(
        merchant_oid: &str,
        merchant_salt: &str,
        merchant_key: &str,
        status: &str,
        total_amount: &str,
        received_hash: &str,
    ) -> bool {
        // Hash string: merchant_oid + merchant_salt + status + total_amount
        let hash_str = format!("{}{}{}{}", merchant_oid, merchant_salt, status, total_amount);

        match Self::create_token(&hash_str, "", merchant_key) {
            Ok(calculated_hash) => {
                eprintln!("PayTR callback hash verification:");
                eprintln!("  Calculated: {}", calculated_hash);
                eprintln!("  Received: {}", received_hash);
                calculated_hash == received_hash
            }
            Err(e) => {
                eprintln!("Hash calculation error: {:?}", e);
                false
            }
        }
    }
}

use crate::modules::payment_provider::models::*;
use crate::modules::payment_provider::services::PaymentProviderError;
use rand::Rng;
use reqwest;
use serde_json::{self, json, Value};

pub struct IyzicoProvider;

impl IyzicoProvider {
    /// İyzico checkout form başlat
    pub async fn initiate_payment(
        config: IyzicoConfig,
        request: PaymentRequest,
        _card_data: Option<std::collections::HashMap<String, String>>,
    ) -> Result<PaymentResponse, PaymentProviderError> {
        let client = reqwest::Client::new();

        // Random string oluştur (8 karakter, harf ve rakam)
        let random_string: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();

        // Request body oluştur - gerçek verilerle
        let body_str = Self::build_json_body(&request);

        eprintln!("İyzico request body: {}", body_str);
        eprintln!("Body length: {}", body_str.len());

        // URL
        let url = "/payment/iyzipos/checkoutform/initialize/auth/ecom";

        // V2 Authorization header oluştur
        let auth_string = Self::create_auth_v2(&config, url, &random_string, &body_str);

        eprintln!("Authorization: {}", auth_string);
        eprintln!("x-iyzi-rnd: {}", random_string);

        // İyzico API çağrısı - base_url zaten https:// içeriyorsa ekleme
        let api_url = if config.base_url.starts_with("http") {
            format!("{}{}", config.base_url, url)
        } else {
            format!("https://{}{}", config.base_url, url)
        };

        eprintln!("İyzico API URL: {}", api_url);
        eprintln!("Sending request...");

        let response = match client
            .post(&api_url)
            .header("Authorization", &auth_string)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("x-iyzi-rnd", &random_string)
            .header("x-iyzi-client-version", "iyzipay-rust-1.0.0")
            .body(body_str)
            .send()
            .await
        {
            Ok(resp) => {
                eprintln!("Response status: {}", resp.status());
                resp
            }
            Err(e) => {
                eprintln!("Request error: {:?}", e);
                return Err(PaymentProviderError::ProviderError(format!(
                    "İyzico API hatası: {}",
                    e
                )));
            }
        };

        let response_text = match response.text().await {
            Ok(text) => {
                eprintln!("Response text received, length: {}", text.len());
                text
            }
            Err(e) => {
                eprintln!("Response read error: {:?}", e);
                return Err(PaymentProviderError::ProviderError(format!(
                    "Response okuma hatası: {}",
                    e
                )));
            }
        };

        eprintln!("İyzico API response: {}", response_text);

        let response_json: Value = serde_json::from_str(&response_text).map_err(|e| {
            PaymentProviderError::ProviderError(format!("JSON parse hatası: {}", e))
        })?;

        if response_json.get("status").and_then(|s| s.as_str()) == Some("success") {
            let checkout_form_content = response_json
                .get("checkoutFormContent")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());

            let token = response_json
                .get("token")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string());

            eprintln!("İyzico checkout form başarılı, token: {:?}", token);

            Ok(PaymentResponse {
                success: true,
                payment_url: None,
                token,
                error_message: None,
                provider_response: json!({
                    "form_html": checkout_form_content,
                    "status": "success",
                    "raw_response": response_json
                }),
            })
        } else {
            let error_message = response_json
                .get("errorMessage")
                .and_then(|e| e.as_str())
                .unwrap_or("Bilinmeyen hata")
                .to_string();

            let error_code = response_json
                .get("errorCode")
                .and_then(|e| e.as_str())
                .unwrap_or("?");

            eprintln!("İyzico hata: {} (kod: {})", error_message, error_code);

            Ok(PaymentResponse {
                success: false,
                payment_url: None,
                token: None,
                error_message: Some(error_message),
                provider_response: response_json,
            })
        }
    }

    /// JSON body oluştur - gerçek verilerle
    fn build_json_body(request: &PaymentRequest) -> String {
        // Fiyatı string formatına çevir
        let price_str = format!("{:.2}", request.amount);

        // TEST: paidPrice 500 TL daha az gösterilecek (test amaçlı)
        // let paid_price_value = (request.amount - 500.0).max(0.0);
        let paid_price_value = request.amount;
        let paid_price_str = format!("{:.2}", paid_price_value);

        // Basket items JSON oluştur
        let basket_items_json: Vec<String> = request.basket_items.iter().map(|item| {
            let category2 = item.category2.as_deref().unwrap_or("");
            format!(
                r#"{{"id":"{}","name":"{}","category1":"{}","category2":"{}","itemType":"{}","price":"{}"}}"#,
                Self::escape_json(&item.id),
                Self::escape_json(&item.name),
                Self::escape_json(&item.category1),
                Self::escape_json(category2),
                item.item_type,
                item.price
            )
        }).collect();

        let basket_items_str = basket_items_json.join(",");

        // Müşteri bilgileri
        let customer_address = request
            .customer_address
            .as_deref()
            .unwrap_or("Adres bilgisi yok");
        let customer_city = request.customer_city.as_deref().unwrap_or("Istanbul");
        let customer_country = request.customer_country.as_deref().unwrap_or("Turkey");
        let customer_zip = request.customer_zip_code.as_deref().unwrap_or("34000");

        // TCKN: request'ten geleni kullan, yoksa default
        let identity_number = request
            .customer_identity_number
            .as_deref()
            .unwrap_or("11111111111");

        // İsim ve soyisim ayır (Buyer için her zaman birey ismini kullan)
        let name_parts: Vec<&str> = request.customer_name.split_whitespace().collect();
        let first_name = name_parts.first().unwrap_or(&"Müşteri");
        let last_name = if name_parts.len() > 1 {
            name_parts[1..].join(" ")
        } else {
            "".to_string()
        };
        let last_name = if last_name.is_empty() {
            "Müşteri"
        } else {
            &last_name
        };

        // Fatura Adresi Mantığı
        let is_corporate = request.invoice_type.as_deref() == Some("corporate");

        // Fatura kimin adına? Kurumsal ise Firma Adı, değilse Şahıs Adı
        let billing_contact_name = if is_corporate {
            request
                .company_name
                .as_deref()
                .unwrap_or(&request.customer_name)
        } else {
            &request.customer_name
        };

        // Fatura adresi detaylandırma
        let billing_address_full = if is_corporate {
            let tax_office = request.tax_office.as_deref().unwrap_or("-");
            let tax_number = request.tax_number.as_deref().unwrap_or("-");
            format!(
                "{} (VD: {} VN: {})",
                customer_address, tax_office, tax_number
            )
        } else {
            customer_address.to_string()
        };

        // Tarih formatları
        let now = chrono::Utc::now();
        let registration_date = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let last_login_date = now.format("%Y-%m-%d %H:%M:%S").to_string();

        format!(
            r#"{{"locale":"tr","conversationId":"{}","price":"{}","paidPrice":"{}","currency":"{}","basketId":"{}","paymentGroup":"PRODUCT","callbackUrl":"{}","enabledInstallments":["1","2","3","6","9","12"],"buyer":{{"id":"{}","name":"{}","surname":"{}","gsmNumber":"{}","email":"{}","identityNumber":"{}","lastLoginDate":"{}","registrationDate":"{}","registrationAddress":"{}","ip":"{}","city":"{}","country":"{}","zipCode":"{}"}},"shippingAddress":{{"contactName":"{} {}","city":"{}","country":"{}","address":"{}","zipCode":"{}"}},"billingAddress":{{"contactName":"{}","city":"{}","country":"{}","address":"{}","zipCode":"{}"}},"basketItems":[{}]}}"#,
            Self::escape_json(&request.order_id),
            price_str,
            paid_price_str,
            request.currency,
            Self::escape_json(&request.order_id),
            Self::escape_json(&request.callback_url),
            Self::escape_json(&request.customer_id),
            Self::escape_json(first_name),
            Self::escape_json(last_name),
            Self::escape_json(&request.customer_phone),
            Self::escape_json(&request.customer_email),
            identity_number,
            last_login_date,
            registration_date,
            Self::escape_json(customer_address),
            Self::escape_json(&request.customer_ip),
            Self::escape_json(customer_city),
            Self::escape_json(customer_country),
            Self::escape_json(customer_zip),
            // Shipping Address
            Self::escape_json(first_name),
            Self::escape_json(last_name),
            Self::escape_json(customer_city),
            Self::escape_json(customer_country),
            Self::escape_json(customer_address),
            Self::escape_json(customer_zip),
            // Billing Address
            Self::escape_json(billing_contact_name),
            Self::escape_json(customer_city),
            Self::escape_json(customer_country),
            Self::escape_json(&billing_address_full),
            Self::escape_json(customer_zip),
            basket_items_str
        )
    }

    /// JSON string escape
    fn escape_json(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                '"' => "\\\"".to_string(),
                '\\' => "\\\\".to_string(),
                '\n' => "\\n".to_string(),
                '\r' => "\\r".to_string(),
                '\t' => "\\t".to_string(),
                c if c.is_ascii() => c.to_string(),
                c => format!("\\u{:04x}", c as u32),
            })
            .collect()
    }

    /// V2 Authorization header oluştur - Python SDK ile aynı format
    fn create_auth_v2(
        config: &IyzicoConfig,
        url: &str,
        random_str: &str,
        body_str: &str,
    ) -> String {
        use base64::{engine::general_purpose, Engine as _};
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        // V2 format: random_str + url + body_str
        let data_to_sign = format!("{}{}{}", random_str, url, body_str);

        // HMAC SHA256
        let mut mac = HmacSha256::new_from_slice(config.secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(data_to_sign.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        // Authorization params: apiKey:XXX&randomKey:XXX&signature:XXX
        let auth_params = format!(
            "apiKey:{}&randomKey:{}&signature:{}",
            config.api_key, random_str, signature
        );

        let auth_base64 = general_purpose::STANDARD.encode(auth_params.as_bytes());

        format!("IYZWSv2 {}", auth_base64)
    }

    /// Token ile ödeme sonucunu sorgula (checkout form callback sonrası)
    pub async fn retrieve_payment(
        config: IyzicoConfig,
        token: &str,
    ) -> Result<PaymentRetrieveResponse, PaymentProviderError> {
        let client = reqwest::Client::new();

        // Random string oluştur
        let random_string: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();

        // Request body - sadece token
        let body_str = format!(r#"{{"locale":"tr","token":"{}"}}"#, token);

        eprintln!("İyzico retrieve payment request: {}", body_str);

        // URL
        let url = "/payment/iyzipos/checkoutform/auth/ecom/detail";

        // V2 Authorization header
        let auth_string = Self::create_auth_v2(&config, url, &random_string, &body_str);

        // API URL
        let api_url = if config.base_url.starts_with("http") {
            format!("{}{}", config.base_url, url)
        } else {
            format!("https://{}{}", config.base_url, url)
        };

        eprintln!("İyzico retrieve URL: {}", api_url);

        let response = client
            .post(&api_url)
            .header("Authorization", &auth_string)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("x-iyzi-rnd", &random_string)
            .body(body_str)
            .send()
            .await
            .map_err(|e| {
                PaymentProviderError::ProviderError(format!("İyzico API hatası: {}", e))
            })?;

        let response_text = response.text().await.map_err(|e| {
            PaymentProviderError::ProviderError(format!("Response okuma hatası: {}", e))
        })?;

        eprintln!("İyzico retrieve response: {}", response_text);

        let response_json: Value = serde_json::from_str(&response_text).map_err(|e| {
            PaymentProviderError::ProviderError(format!("JSON parse hatası: {}", e))
        })?;

        let status = response_json
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("failure")
            .to_string();

        let payment_status = response_json
            .get("paymentStatus")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());

        let payment_id = response_json
            .get("paymentId")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());

        let paid_price = response_json
            .get("paidPrice")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());

        let error_message = response_json
            .get("errorMessage")
            .and_then(|e| e.as_str())
            .map(|s| s.to_string());

        let conversation_id = response_json
            .get("conversationId")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        Ok(PaymentRetrieveResponse {
            success: status == "success" && payment_status.as_deref() == Some("SUCCESS"),
            status,
            payment_status,
            payment_id,
            paid_price,
            error_message,
            conversation_id,
            raw_response: response_json,
        })
    }
}

/// İyzico ödeme sorgulama response
#[derive(Debug)]
#[allow(dead_code)]
pub struct PaymentRetrieveResponse {
    pub success: bool,
    pub status: String,
    pub payment_status: Option<String>,
    pub payment_id: Option<String>,
    pub paid_price: Option<String>,
    pub error_message: Option<String>,
    pub conversation_id: Option<String>,
    pub raw_response: Value,
}

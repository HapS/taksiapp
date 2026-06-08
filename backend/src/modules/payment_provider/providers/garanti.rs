use crate::modules::payment_provider::models::*;
use crate::modules::payment_provider::services::PaymentProviderError;
use reqwest;
use serde_json;
use std::collections::HashMap;

pub struct GarantiProvider;

impl GarantiProvider {
    /// Garanti Bankası ödeme başlat
    pub async fn initiate_payment(
        config: GarantiConfig,
        request: PaymentRequest,
        card_data: Option<std::collections::HashMap<String, String>>, // Kredi kartı bilgileri
    ) -> Result<PaymentResponse, PaymentProviderError> {
        // Garanti Bankası API entegrasyonu burada yapılacak
        // Şimdilik mock response döndürüyoruz
        
        let _client = reqwest::Client::new();
        
        // Garanti API request body hazırla - Python koduna göre düzeltildi
        let mut body = HashMap::new();
        
        // Temel alanlar
        body.insert("companyname".to_string(), "Test Company".to_string());
        body.insert("apiversion".to_string(), "512".to_string());
        body.insert("version".to_string(), "v0.01".to_string());
        body.insert("mode".to_string(), "TEST".to_string());
        body.insert("terminalprovuserid".to_string(), config.user_id.clone());
        body.insert("terminaluserid".to_string(), config.user_id.clone());
        body.insert("terminalmerchantid".to_string(), config.merchant_id.clone());
        body.insert("terminalid".to_string(), config.terminal_id.clone());
        
        // Sipariş bilgileri
        body.insert("orderid".to_string(), request.order_id.clone());
        body.insert("txnamount".to_string(), ((request.amount * 100.0) as i64).to_string());
        body.insert("txncurrencycode".to_string(), "949".to_string()); // TRY
        body.insert("txninstallmentcount".to_string(), "".to_string()); // Tek çekim
        body.insert("txntype".to_string(), "sales".to_string());
        
        // URL'ler - Garanti callback URL'leri kullan
        body.insert("successurl".to_string(), request.callback_url.clone());
        body.insert("errorurl".to_string(), request.callback_url.clone());
        
        // Müşteri bilgileri
        body.insert("customeremailaddress".to_string(), request.customer_email.clone());
        body.insert("customeripaddress".to_string(), "127.0.0.1".to_string()); // Test için
        
        // Zaman bilgileri
        let now = chrono::Utc::now();
        body.insert("txntimestamp".to_string(), now.format("%H:%M:%S").to_string());
        body.insert("txntimeoutperiod".to_string(), "900".to_string());
        body.insert("refreshtime".to_string(), "3".to_string());
        
        // 3D Secure
        body.insert("secure3dsecuritylevel".to_string(), "3D_PAY".to_string());
        body.insert("lang".to_string(), "tr".to_string());
        
        eprintln!("Garanti request data (Python format):");
        eprintln!("  terminalid: {}", config.terminal_id);
        eprintln!("  terminalmerchantid: {}", config.merchant_id);
        eprintln!("  orderid: {}", request.order_id);
        eprintln!("  txnamount: {}", ((request.amount * 100.0) as i64));
        eprintln!("  successurl: {}", request.success_url);
        eprintln!("  errorurl: {}", request.failure_url);
        eprintln!("  terminalprovuserid: {}", config.user_id);
        eprintln!("  store_key: {}", config.store_key);
        
        // Tüm parametreleri yazdır
        eprintln!("All Garanti parameters (Python format):");
        for (key, value) in &body {
            if key.contains("password") || key == "secure3dhash" {
                eprintln!("  {}: [HIDDEN]", key);
            } else {
                eprintln!("  {}: {}", key, value);
            }
        }
        
        // Hash oluştur - Python kodundaki get_hash_data fonksiyonuna göre
        let hash = Self::create_hash(
            &config.password,           // provision_password
            &config.terminal_id,        // terminal_id  
            &request.order_id,          // order_id
            &((request.amount * 100.0) as i64).to_string(), // amount
            "949",                      // currency_code
            &request.callback_url,      // success_url (callback olarak kullan)
            &request.callback_url,      // error_url (callback olarak kullan)
            "sales",                    // type
            "",                         // installment_count (boş)
            &config.store_key          // store_key
        );
        
        body.insert("secure3dhash".to_string(), hash.clone());
        
        eprintln!("Generated hash: {}", hash);
        
        // DÜZELTME: POST request olarak gönder, Python kodundaki gibi
        let form_url = format!("{}/servlet/gt3dengine", config.base_url);
        
        // HTML form oluştur - Kredi kartı bilgileri ile birlikte
        let mut form_html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Garanti Bankası Güvenli Ödeme</title>
</head>
<body onload="document.forms[0].submit();">
    <form method="POST" action="{}">
"#, form_url);

        // Hidden alanları ekle
        for (key, value) in &body {
            form_html.push_str(&format!(r#"        <input type="hidden" name="{}" value="{}" />
"#, key, value));
        }

        // Kredi kartı bilgilerini ekle (eğer varsa)
        if let Some(card_info) = card_data {
            if let Some(cardnumber) = card_info.get("cardnumber") {
                form_html.push_str(&format!(r#"        <input type="hidden" name="cardnumber" value="{}" />
"#, cardnumber));
            }
            if let Some(month) = card_info.get("cardexpiredatemonth") {
                form_html.push_str(&format!(r#"        <input type="hidden" name="cardexpiredatemonth" value="{}" />
"#, month));
            }
            if let Some(year) = card_info.get("cardexpiredateyear") {
                form_html.push_str(&format!(r#"        <input type="hidden" name="cardexpiredateyear" value="{}" />
"#, year));
            }
            if let Some(cvv) = card_info.get("cardcvv2") {
                form_html.push_str(&format!(r#"        <input type="hidden" name="cardcvv2" value="{}" />
"#, cvv));
            }
            if let Some(name) = card_info.get("cardholdername") {
                form_html.push_str(&format!(r#"        <input type="hidden" name="cardholdername" value="{}" />
"#, name));
            }
        }

        form_html.push_str(r#"        <input type="submit" value="Garanti Bankası ile Güvenli Ödeme" />
    </form>
    <script>
        document.forms[0].submit();
    </script>
</body>
</html>"#);
        
        eprintln!("Garanti form HTML generated for URL: {}", form_url);
        
        Ok(PaymentResponse {
            success: true,
            payment_url: None, // HTML form döndürüyoruz
            token: Some(request.order_id),
            error_message: None,
            provider_response: serde_json::json!({
                "form_html": form_html,
                "form_url": form_url,
                "form_data": body
            }),
        })
    }
    
    /// Garanti hash oluştur - Python kodundaki get_hash_data fonksiyonuna göre
    fn create_hash(password: &str, terminal_id: &str, order_id: &str, amount: &str, currency_code: &str, success_url: &str, error_url: &str, type_: &str, installment_count: &str, store_key: &str) -> String {
        use sha1::{Sha1, Digest as Sha1Digest};
        use sha2::{Sha512};
        
        // 1. Önce password'ü hash'le: SHA1(password + "0" + terminal_id)
        let password_data = format!("{}0{}", password, terminal_id);
        let mut sha1_hasher = Sha1::new();
        sha1_hasher.update(password_data.as_bytes());
        let hashed_password = format!("{:X}", sha1_hasher.finalize()); // Uppercase hex
        
        // 2. Ana data'yı oluştur
        let data = format!("{}{}{}{}{}{}{}{}{}{}",
            terminal_id,
            order_id,
            amount,
            currency_code,
            success_url,
            error_url,
            type_,
            installment_count,
            store_key,
            hashed_password
        );
        
        eprintln!("Hash components:");
        eprintln!("  password_data: {}", password_data);
        eprintln!("  hashed_password: {}", hashed_password);
        eprintln!("  final_data: {}", data);
        
        // 3. SHA512 ile final hash
        let mut sha512_hasher = Sha512::new();
        sha512_hasher.update(data.as_bytes());
        let final_hash = format!("{:X}", sha512_hasher.finalize()); // Uppercase hex
        
        final_hash
    }
    
    /// Garanti callback hash doğrula - Python'daki check_hash_data fonksiyonuna göre
    pub fn verify_callback_hash(callback_data: &std::collections::HashMap<String, String>, store_key: &str) -> bool {
        use sha2::{Sha512, Digest};
        
        // Response hash'i al
        let response_hash = match callback_data.get("hash") {
            Some(hash) => hash,
            None => {
                eprintln!("Hash field not found in callback data");
                return false;
            }
        };
        
        // Hash params'ı al ve parse et
        let hash_params = match callback_data.get("hashparams") {
            Some(params) => params,
            None => {
                eprintln!("Hashparams field not found in callback data");
                return false;
            }
        };
        
        // Parametreleri ayır (: ile ayrılmış)
        let param_list: Vec<&str> = hash_params.split(':').collect();
        let param_list = &param_list[..param_list.len()-1]; // Son boş elemanı çıkar
        
        // Digest data oluştur
        let mut digest_data = String::new();
        for param in param_list {
            if let Some(value) = callback_data.get(*param) {
                digest_data.push_str(value);
            }
        }
        digest_data.push_str(store_key);
        
        eprintln!("Hash verification:");
        eprintln!("  Hash params: {}", hash_params);
        eprintln!("  Digest data: {}", digest_data);
        eprintln!("  Response hash: {}", response_hash);
        
        // SHA512 hash hesapla
        let mut hasher = Sha512::new();
        hasher.update(digest_data.as_bytes());
        let calculated_hash = format!("{:X}", hasher.finalize());
        
        eprintln!("  Calculated hash: {}", calculated_hash);
        
        let is_valid = response_hash == &calculated_hash;
        if is_valid {
            eprintln!("!!!!!MESAJ BANKADAN GELİYOR!!!!!");
        } else {
            eprintln!("!!!!!MESAJ BANKADAN GELMİYOR!!!!!");
        }
        
        is_valid
    }
}
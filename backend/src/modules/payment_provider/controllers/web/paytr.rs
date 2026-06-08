use crate::app_state::AppState;
use crate::middleware::global_context::ViewContext;
use crate::modules::ecommerce::models::cart::{self, Entity as Cart};
use crate::modules::ecommerce::services::cart_service;
use crate::modules::payment_provider::models::{PaymentProviderType, PaytrConfig};
use crate::modules::payment_provider::providers::paytr::PaytrProvider;
use crate::modules::payment_provider::services::PaymentProviderService;
use crate::modules::utils::format_price::format_price;
use axum::{
    extract::{Form, Path, State},
    response::{Html, IntoResponse, Redirect},
    Extension,
};
use sea_orm::sea_query::Expr;
use sea_orm::*;
use serde::Deserialize;
use std::collections::HashMap;

/// PayTR callback form data
/// PayTR ödeme sonucunu bu formatta gönderir
#[derive(Deserialize, Debug)]
pub struct PaytrCallbackForm {
    /// Sipariş numarası (merchant_oid)
    pub merchant_oid: Option<String>,
    /// Ödeme durumu: "success" veya "failed"
    pub status: Option<String>,
    /// Toplam tutar (kuruş cinsinden)
    pub total_amount: Option<String>,
    /// Hash doğrulama
    pub hash: Option<String>,
    /// Hata mesajı (başarısız ödemelerde)
    pub failed_reason_code: Option<String>,
    pub failed_reason_msg: Option<String>,
    /// Ödeme ID
    pub payment_type: Option<String>,
    pub currency: Option<String>,
    /// Diğer alanlar
    #[serde(flatten)]
    pub other: HashMap<String, String>,
}

/// POST /payment-provider/paytr/callback - PayTR bildirim URL'i (server-to-server)
/// PayTR bu URL'e ödeme sonucunu POST eder
pub async fn paytr_callback(
    State(state): State<AppState>,
    Form(callback_data): Form<PaytrCallbackForm>,
) -> impl IntoResponse {
    eprintln!("========================================");
    eprintln!("PayTR callback received");
    eprintln!("Callback data: {:?}", callback_data);
    eprintln!("========================================");

    // merchant_oid = cart.order_id (alfanümerik)
    let merchant_oid = match &callback_data.merchant_oid {
        Some(oid) => oid.clone(),
        None => {
            eprintln!("PayTR callback: No merchant_oid received!");
            return "FAIL".to_string();
        }
    };

    // Cart'ı order_id ile bul
    let cart = match Cart::find()
        .filter(cart::Column::OrderId.eq(&merchant_oid))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => {
            eprintln!(
                "Cart not found for order_id (merchant_oid): {}",
                merchant_oid
            );
            return "FAIL".to_string();
        }
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            return "FAIL".to_string();
        }
    };

    let cart_id = cart.id;
    let user_id = cart.user_id;
    let payment_url = cart.payment_url.clone().unwrap_or_default();

    // PayTR config'i yükle ve hash doğrula
    let paytr_config =
        match PaymentProviderService::get_provider_config(&state.db, PaymentProviderType::PayTR)
            .await
        {
            Ok(config) => match serde_json::from_value::<PaytrConfig>(config.config) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to parse PayTR config: {:?}", e);
                    return "FAIL".to_string();
                }
            },
            Err(e) => {
                eprintln!("Failed to load PayTR config: {:?}", e);
                return "FAIL".to_string();
            }
        };

    // Hash doğrulama
    let status = callback_data.status.as_deref().unwrap_or("");
    let total_amount = callback_data.total_amount.as_deref().unwrap_or("");
    let received_hash = callback_data.hash.as_deref().unwrap_or("");

    let is_valid = PaytrProvider::verify_callback_hash(
        &merchant_oid,
        &paytr_config.merchant_salt,
        &paytr_config.merchant_key,
        status,
        total_amount,
        received_hash,
    );

    if !is_valid {
        eprintln!("PayTR callback: Hash verification failed!");
        // Hash doğrulama başarısız, loglama için callback_data'yı kaydet
        let mut callback_json = serde_json::Map::new();
        
        // Temel bilgiler
        callback_json.insert("hash_verified".to_string(), serde_json::Value::Bool(false));
        callback_json.insert("merchant_oid".to_string(), serde_json::Value::String(merchant_oid.clone()));
        callback_json.insert("status".to_string(), serde_json::Value::String(status.to_string()));
        callback_json.insert("total_amount".to_string(), serde_json::Value::String(total_amount.to_string()));
        callback_json.insert("received_hash".to_string(), serde_json::Value::String(received_hash.to_string()));
        
        // Diğer callback alanları
        if let Some(ref code) = callback_data.failed_reason_code {
            callback_json.insert("failed_reason_code".to_string(), serde_json::Value::String(code.clone()));
        }
        if let Some(ref msg) = callback_data.failed_reason_msg {
            callback_json.insert("failed_reason_msg".to_string(), serde_json::Value::String(msg.clone()));
        }
        if let Some(ref ptype) = callback_data.payment_type {
            callback_json.insert("payment_type".to_string(), serde_json::Value::String(ptype.clone()));
        }
        if let Some(ref curr) = callback_data.currency {
            callback_json.insert("currency".to_string(), serde_json::Value::String(curr.clone()));
        }
        
        // Diğer tüm alanlar
        for (key, value) in &callback_data.other {
            callback_json.insert(key.clone(), serde_json::Value::String(value.clone()));
        }
        
        callback_json.insert("callback_timestamp".to_string(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));
        callback_json.insert("provider".to_string(), serde_json::Value::String("paytr".to_string()));

        let _ = cart::Entity::update_many()
            .col_expr(
                cart::Column::CallbackData,
                Expr::value(sea_orm::prelude::Json::from(serde_json::Value::Object(callback_json))),
            )
            .col_expr(cart::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(cart::Column::Id.eq(cart_id))
            .exec(&state.db)
            .await;

        return "FAIL".to_string();
    }

    eprintln!("PayTR callback: Hash verification successful");

    // Callback data'yı JSON'a çevir ve kaydet
    let callback_json = {
        let mut json_data = serde_json::Map::new();

        // Hash doğrulama bilgileri
        json_data.insert("hash_verified".to_string(), serde_json::Value::Bool(true));
        json_data.insert("received_hash".to_string(), serde_json::Value::String(received_hash.to_string()));
        
        // Temel callback verileri
        if let Some(ref s) = callback_data.status {
            json_data.insert("status".to_string(), serde_json::Value::String(s.clone()));
        }
        if let Some(ref oid) = callback_data.merchant_oid {
            json_data.insert(
                "merchant_oid".to_string(),
                serde_json::Value::String(oid.clone()),
            );
        }
        if let Some(ref amount) = callback_data.total_amount {
            json_data.insert(
                "total_amount".to_string(),
                serde_json::Value::String(amount.clone()),
            );
        }
        if let Some(ref code) = callback_data.failed_reason_code {
            json_data.insert(
                "failed_reason_code".to_string(),
                serde_json::Value::String(code.clone()),
            );
        }
        if let Some(ref msg) = callback_data.failed_reason_msg {
            json_data.insert(
                "failed_reason_msg".to_string(),
                serde_json::Value::String(msg.clone()),
            );
        }
        if let Some(ref ptype) = callback_data.payment_type {
            json_data.insert(
                "payment_type".to_string(),
                serde_json::Value::String(ptype.clone()),
            );
        }
        if let Some(ref curr) = callback_data.currency {
            json_data.insert(
                "currency".to_string(),
                serde_json::Value::String(curr.clone()),
            );
        }

        // Diğer tüm callback alanları
        for (key, value) in &callback_data.other {
            if !json_data.contains_key(key) {
                json_data.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }

        json_data.insert(
            "callback_timestamp".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );
        json_data.insert(
            "provider".to_string(),
            serde_json::Value::String("paytr".to_string()),
        );

        serde_json::Value::Object(json_data)
    };

    // Callback data'yı cart'a kaydet
    let _ = cart::Entity::update_many()
        .col_expr(
            cart::Column::CallbackData,
            Expr::value(sea_orm::prelude::Json::from(callback_json)),
        )
        .col_expr(cart::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
        .filter(cart::Column::Id.eq(cart_id))
        .exec(&state.db)
        .await;

    // Ödeme durumunu kontrol et
    if status == "success" {
        eprintln!("PayTR payment successful! Completing order...");

        // Ödeme notu oluştur (iyzico dizaynına benzer)
        let payment_note = format!("Sipariş alındı ve ödeme başarıyla tamamlandı (PayTR). Merchant OID: {}", merchant_oid);

        // complete_order_from_payment fonksiyonunu kullan
        match cart_service::complete_order_from_payment(
            &state.db,
            payment_url.clone(),
            user_id,
            Some(payment_note),
            None,
        )
        .await
        {
            Ok(_) => {
                eprintln!("Order completed successfully via complete_order_from_payment");

                // Status'u "confirmed" olarak güncelle
                let _ = cart::Entity::update_many()
                    .col_expr(cart::Column::Status, Expr::value("confirmed".to_string()))
                    .col_expr(cart::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
                    .filter(cart::Column::Id.eq(cart_id))
                    .exec(&state.db)
                    .await;

                eprintln!("Status updated to 'confirmed'");

                // PayTR'ye "OK" yanıtı dön - önemli!
                return "OK".to_string();
            }
            Err(e) => {
                eprintln!("Failed to complete order: {:?}", e);

                // Sipariş zaten tamamlanmış olabilir
                if let Ok(Some(c)) = Cart::find()
                    .filter(cart::Column::PaymentUrl.eq(&payment_url))
                    .one(&state.db)
                    .await
                {
                    if c.status != "open_cart" {
                        eprintln!("Order already processed, status: {}", c.status);
                        return "OK".to_string();
                    }
                }

                return "OK".to_string(); // Yine de OK dön, tekrar denemeyi engellemek için
            }
        }
    } else {
        eprintln!("PayTR payment failed - status: {}", status);
        if let Some(ref msg) = callback_data.failed_reason_msg {
            eprintln!("PayTR error message: {}", msg);
        }

        // Başarısız ödemeyi de logla
        let _ = cart::Entity::update_many()
            .col_expr(
                cart::Column::Status,
                Expr::value("payment_failed".to_string()),
            )
            .col_expr(cart::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(cart::Column::Id.eq(cart_id))
            .exec(&state.db)
            .await;

        // PayTR'ye "OK" dön (bildirimi aldık, başarısız olsa da)
        return "OK".to_string();
    }
}

/// GET /payment-provider/paytr/success/{payment_url} - PayTR ödeme başarılı sayfası
/// Kullanıcı ödeme sonrası bu sayfaya yönlendirilir
pub async fn paytr_payment_success(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    eprintln!("PayTR payment success page requested for: {}", payment_url);
    eprintln!("User ID from extension: {:?}", user_id);

    // Önce payment_url ile cart'ı bul (user_id kontrolü olmadan)
    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => {
            eprintln!("Cart found: id={}, status={}, user_id={}", cart.id, cart.status, cart.user_id);
            cart
        }
        Ok(None) => {
            eprintln!("Cart not found for payment_url: {}", payment_url);
            return Redirect::to("/my-cart").into_response();
        }
        Err(e) => {
            eprintln!("Database error finding cart: {:?}", e);
            return Redirect::to("/my-cart").into_response();
        }
    };

    // Cart'ın user_id'sini kullan
    let cart_user_id = cart.user_id;

    let cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(cart_user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            eprintln!("get_cart error: {:?}", e);
            return Redirect::to("/my-cart").into_response();
        }
    };

    // Ödeme sonucunu kontrol et ve siparişi işle
    if cart.status == crate::modules::ecommerce::models::cart::status::OPEN_CART {
        // Callback henüz gelmemiş - siparişi hemen işle (eski akış)
        eprintln!("PayTR success page: Sipariş hemen işleniyor (callback beklenmiyor)");
        let cargo_fee = cart_response.standart_cargo_fee.unwrap_or(0.0);
        
        // Önce kargo ücretini kaydet
        let _ = Cart::update_many()
            .col_expr(cart::Column::CargoPrice, Expr::value(Some(cargo_fee)))
            .col_expr(cart::Column::CargoCurrency, Expr::value(Some(cart_response.currency.clone())))
            .col_expr(cart::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(cart::Column::Id.eq(cart.id))
            .exec(&state.db)
            .await;
        
        // Siparişi tamamla - timeline, email vb. her şey işlenecek
        let payment_note = format!("PayTR ile ödeme yapıldı. Merchant OID: {}", cart.order_id.clone().unwrap_or_default());
        match cart_service::complete_order_from_payment(
            &state.db,
            payment_url.clone(),
            cart_user_id,
            Some(payment_note),
            None,
        )
        .await
        {
            Ok(_) => {
                eprintln!("Order completed successfully via complete_order_from_payment");
            }
            Err(e) => {
                eprintln!("Failed to complete order: {:?}", e);
                // Sipariş zaten tamamlanmış olabilir
                if let Ok(Some(c)) = Cart::find()
                    .filter(cart::Column::PaymentUrl.eq(&payment_url))
                    .one(&state.db)
                    .await
                {
                    if c.status != "open_cart" {
                        eprintln!("Order already processed, status: {}", c.status);
                    }
                }
            }
        }
    } else if cart.status == crate::modules::ecommerce::models::cart::status::CONFIRMED {
        // Callback zaten gelmiş ve sipariş onaylanmış
        eprintln!("PayTR success page: Sipariş zaten callback ile onaylanmış");
    }

    // cart_response'dan tutarları al (daha güvenilir)
    let cart_currency = cart_response.currency.clone();

    // Cart response total'ı baz al (muhtemelen ürün toplamıdır)
    let product_total = cart_response.total;

    // Kargo ücreti - cart henüz kaydedilmemişse standart_cargo_fee kullan
    let cargo_price = cart_response.cargo_price.unwrap_or_else(|| {
        cart_response.standart_cargo_fee.unwrap_or(0.0)
    });
    let is_free_shipping = cart_response.is_free_shipping || cargo_price == 0.0;

    // Genel toplam = ürün toplam + kargo
    let final_total = if is_free_shipping {
        product_total
    } else {
        product_total + cargo_price
    };

    // Adres bilgisi
    let address_line = cart_response
        .address_line
        .clone()
        .or(cart.address_line.clone())
        .unwrap_or_else(|| "Adres bilgisi yok".to_string());

    eprintln!("PayTR success - product_total: {}, cargo: {}, final: {}", product_total, cargo_price, final_total);

    context.0.insert("title", "PayTR - Ödeme Başarılı");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context.0.insert("total_amount", &format_price(final_total, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert("message", "PayTR ile ödemeniz başarıyla tamamlandı!");
    context.0.insert("provider_name", "PayTR");
    context.0.insert("cargo_price_formatted", &format_price(cargo_price, &cart_currency));
    context.0.insert("product_total_formatted", &format_price(product_total, &cart_currency));
    context.0.insert("is_free_shipping", &is_free_shipping);
    context.0.insert("address_line", &address_line);
    context.0.insert("payment_successful", &true);
    context.0.insert("payment_provider", "PayTR");

    match state.render_frontend_template("cart/payment/payment_success.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

/// GET /payment-provider/paytr/failure/{payment_url} - PayTR ödeme başarısız sayfası
pub async fn paytr_payment_failure(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    eprintln!("PayTR payment failure page requested for: {}", payment_url);

    let _user_id = match user_id {
        Some(id) => id,
        None => {
            return Redirect::to("/login").into_response();
        }
    };

    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        // user_id kontrolünü kaldır - payment_url benzersiz
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => {
            return Redirect::to("/my-cart").into_response();
        }
        Err(_) => {
            return Redirect::to("/my-cart").into_response();
        }
    };

    // Callback data'dan hata mesajını al
    let error_message = if let Some(callback_data) = &cart.callback_data {
        callback_data
            .get("failed_reason_msg")
            .and_then(|m| m.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                callback_data
                    .get("error_message")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
            })
    } else {
        None
    };

    context.0.insert("title", "PayTR - Ödeme Başarısız");
    context.0.insert("cart", &cart);
    context.0.insert("payment_successful", &false);
    context.0.insert("payment_provider", "PayTR");
    context.0.insert(
        "error_message",
        &error_message.unwrap_or_else(|| "Ödeme işlemi başarısız oldu.".to_string()),
    );
    context.0.insert("payment_url", &payment_url);

    match state.render_frontend_template("cart/payment/payment_failure.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

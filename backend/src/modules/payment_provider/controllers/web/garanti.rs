use crate::app_state::AppState;
use crate::middleware::global_context::ViewContext;
use crate::modules::ecommerce::models::cart::{self, Entity as Cart};
use crate::modules::ecommerce::services::cart_service;
use crate::modules::utils::format_price::format_price;
use axum::{
    extract::{Form, Path, State},
    response::{Html, IntoResponse, Redirect},
    Extension,
};
use sea_orm::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct GarantiPaymentCallbackForm {
    // Garanti callback alanları
    #[serde(rename = "oid")]
    pub order_id: Option<String>,
    #[serde(rename = "mdstatus")]
    pub md_status: Option<String>,
    #[serde(rename = "response")]
    pub response: Option<String>,
    #[serde(rename = "authcode")]
    pub auth_code: Option<String>,
    #[serde(rename = "txnamount")]
    pub amount: Option<String>,
    #[serde(rename = "errmsg")]
    pub error_message: Option<String>,
    #[serde(rename = "procreturncode")]
    pub proc_return_code: Option<String>,
    #[serde(rename = "hash")]
    pub hash: Option<String>,
    #[serde(rename = "hashparams")]
    pub hash_params: Option<String>,
    // Diğer tüm alanları da kabul et
    #[serde(flatten)]
    pub other: HashMap<String, String>,
}

/// POST /payment-provider/garanti/callback/{payment_url} - Garanti Bankası callback
pub async fn garanti_payment_callback(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Form(callback_data): Form<GarantiPaymentCallbackForm>,
) -> impl IntoResponse {
    eprintln!("Garanti payment callback received for {}", payment_url);
    eprintln!("Callback data: {:?}", callback_data);

    // Order ID'yi callback'ten al (Garanti'den gelen oid)
    let order_id = match &callback_data.order_id {
        Some(oid) => oid,
        None => {
            eprintln!("No order ID in callback data");
            return Redirect::to(&format!(
                "/payment-provider/garanti/failure/{}",
                payment_url
            ))
            .into_response();
        }
    };

    // Cart'ı order_id ile bul (user_id kontrolü yok çünkü banka POST ediyor)
    let cart = match Cart::find()
        .filter(cart::Column::OrderId.eq(order_id))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => {
            eprintln!("Cart not found for order_id: {}", order_id);
            return Redirect::to(&format!(
                "/payment-provider/garanti/failure/{}",
                payment_url
            ))
            .into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {:?}", e);
            return Redirect::to(&format!(
                "/payment-provider/garanti/failure/{}",
                payment_url
            ))
            .into_response();
        }
    };

    // Callback data'yı JSON'a çevir ve cart'a kaydet
    let callback_json = {
        let mut json_data = serde_json::Map::new();

        // GarantiPaymentCallbackForm alanlarını ekle
        if let Some(oid) = &callback_data.order_id {
            json_data.insert(
                "order_id".to_string(),
                serde_json::Value::String(oid.clone()),
            );
        }
        if let Some(md_status) = &callback_data.md_status {
            json_data.insert(
                "md_status".to_string(),
                serde_json::Value::String(md_status.clone()),
            );
        }
        if let Some(response) = &callback_data.response {
            json_data.insert(
                "response".to_string(),
                serde_json::Value::String(response.clone()),
            );
        }
        if let Some(auth_code) = &callback_data.auth_code {
            json_data.insert(
                "auth_code".to_string(),
                serde_json::Value::String(auth_code.clone()),
            );
        }
        if let Some(amount) = &callback_data.amount {
            json_data.insert(
                "amount".to_string(),
                serde_json::Value::String(amount.clone()),
            );
        }
        if let Some(error_message) = &callback_data.error_message {
            json_data.insert(
                "error_message".to_string(),
                serde_json::Value::String(error_message.clone()),
            );
        }
        if let Some(proc_return_code) = &callback_data.proc_return_code {
            json_data.insert(
                "proc_return_code".to_string(),
                serde_json::Value::String(proc_return_code.clone()),
            );
        }
        if let Some(hash) = &callback_data.hash {
            json_data.insert("hash".to_string(), serde_json::Value::String(hash.clone()));
        }
        if let Some(hash_params) = &callback_data.hash_params {
            json_data.insert(
                "hash_params".to_string(),
                serde_json::Value::String(hash_params.clone()),
            );
        }

        // Other alanlarını da ekle
        for (key, value) in &callback_data.other {
            json_data.insert(key.clone(), serde_json::Value::String(value.clone()));
        }

        // Timestamp ve provider bilgisi ekle
        json_data.insert(
            "callback_timestamp".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );
        json_data.insert(
            "provider".to_string(),
            serde_json::Value::String("garanti".to_string()),
        );

        serde_json::Value::Object(json_data)
    };

    // Cart'ı güncelle - callback_data'yı kaydet
    let mut cart_active: cart::ActiveModel = cart.clone().into();
    cart_active.callback_data = Set(Some(sea_orm::prelude::Json::from(callback_json)));
    cart_active.updated_at = Set(Some(chrono::Utc::now().into()));

    if let Err(e) = cart_active.update(&state.db).await {
        eprintln!("Failed to update cart with callback data: {:?}", e);
    } else {
        eprintln!("Garanti callback data saved to cart successfully");
    }

    // Garanti'ye özgü kontroller: procreturncode == '00' kontrolü
    if let Some(proc_return_code) = &callback_data.proc_return_code {
        if proc_return_code == "00" {
            // Hash doğrulaması yap
            let mut all_data = callback_data.other.clone();
            if let Some(oid) = &callback_data.order_id {
                all_data.insert("oid".to_string(), oid.clone());
            }
            if let Some(md) = &callback_data.md_status {
                all_data.insert("mdstatus".to_string(), md.clone());
            }
            if let Some(resp) = &callback_data.response {
                all_data.insert("response".to_string(), resp.clone());
            }
            if let Some(auth) = &callback_data.auth_code {
                all_data.insert("authcode".to_string(), auth.clone());
            }
            if let Some(amt) = &callback_data.amount {
                all_data.insert("txnamount".to_string(), amt.clone());
            }
            if let Some(err) = &callback_data.error_message {
                all_data.insert("errmsg".to_string(), err.clone());
            }
            if let Some(prc) = &callback_data.proc_return_code {
                all_data.insert("procreturncode".to_string(), prc.clone());
            }
            if let Some(hash) = &callback_data.hash {
                all_data.insert("hash".to_string(), hash.clone());
            }
            if let Some(hp) = &callback_data.hash_params {
                all_data.insert("hashparams".to_string(), hp.clone());
            }

            // Store key'i settings'ten al
            let settings =
                match crate::modules::admin::services::settings_service::get_settings(&state.db)
                    .await
                {
                    Ok(settings) => settings,
                    Err(_) => {
                        eprintln!("Failed to load settings for hash verification");
                        return Redirect::to(&format!(
                            "/payment-provider/garanti/failure/{}",
                            payment_url
                        ))
                        .into_response();
                    }
                };

            let store_key = settings
                .payment_providers
                .as_ref()
                .and_then(|pp| pp.get("garanti"))
                .and_then(|g| g.get("config"))
                .and_then(|c| c.get("store_key"))
                .and_then(|sk| sk.as_str())
                .unwrap_or("12345678")
                .to_string();

            // Garanti hash doğrulaması
            if crate::modules::payment_provider::providers::garanti::GarantiProvider::verify_callback_hash(&all_data, &store_key) {
                eprintln!("Garanti hash verification successful - Payment confirmed!");

                // complete_order_from_payment fonksiyonunu kullan (mail ve timeline için)
                let payment_note = "Garanti Bankası ile ödeme yapıldı.".to_string();
                if let Err(e) = cart_service::complete_order_from_payment(
                    &state.db,
                    payment_url.clone(),
                    cart.user_id,
                    Some(payment_note),
                    None,
                ).await {
                    eprintln!("Failed to complete order via service: {:?}", e);
                }

                // Status'u "completed" olarak güncelle (eğer servis PENDING yaptıysa Garanti completed istiyor olabilir)
                let _ = cart::Entity::update_many()
                    .col_expr(cart::Column::Status, sea_orm::sea_query::Expr::value("completed".to_string()))
                    .col_expr(cart::Column::UpdatedAt, sea_orm::sea_query::Expr::value(chrono::Utc::now()))
                    .filter(cart::Column::Id.eq(cart.id))
                    .exec(&state.db)
                    .await;

                // Garanti success sayfasına redirect et
                return Redirect::to(&format!("/payment-provider/garanti/success/{}", payment_url)).into_response();
            } else {
                eprintln!("Garanti hash verification failed - Possible fraud attempt!");
                return Redirect::to(&format!("/payment-provider/garanti/failure/{}", payment_url)).into_response();
            }
        } else {
            eprintln!(
                "Garanti payment failed - proc_return_code: {}",
                proc_return_code
            );
            return Redirect::to(&format!(
                "/payment-provider/garanti/failure/{}",
                payment_url
            ))
            .into_response();
        }
    } else {
        eprintln!("No proc_return_code in Garanti callback data");
        return Redirect::to(&format!(
            "/payment-provider/garanti/failure/{}",
            payment_url
        ))
        .into_response();
    }
}

/// GET /payment-provider/garanti/success/{payment_url} - Garanti ödeme başarılı sayfası
pub async fn garanti_payment_success(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    eprintln!(
        "Garanti payment success page requested for: {}",
        payment_url
    );

    // Kullanıcı giriş yapmış mı kontrol et
    let user_id = match user_id {
        Some(id) => id,
        None => {
            eprintln!("User not logged in for Garanti success page");
            return Redirect::to("/login").into_response();
        }
    };

    // Payment URL ile cart'ı bul ve kullanıcıya ait olduğunu doğrula
    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => {
            eprintln!(
                "Cart not found or not owned by user for payment_url: {}",
                payment_url
            );
            return Redirect::to("/my-cart").into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {:?}", e);
            return Redirect::to("/my-cart").into_response();
        }
    };

    // Cart items bilgilerini al
    let cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            eprintln!("Failed to get cart items for success page");
            return Redirect::to("/my-cart").into_response();
        }
    };

    // total_amount cart'tan al (ürün toplamı)
    let total_amount = cart
        .total_amount
        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0);

    let cart_currency = cart.currency.clone().unwrap_or_else(|| "TRY".to_string());

    // Kargo ücretini cart'tan al
    let cargo_price = cart.cargo_price.unwrap_or(0.0);
    let cargo_currency = cart
        .cargo_currency
        .clone()
        .unwrap_or_else(|| "TRY".to_string());

    // Genel toplam = ürün toplamı + kargo
    let final_total = total_amount + cargo_price;
    let product_total = total_amount;

    context
        .0
        .insert("title", "Garanti Bankası - Ödeme Başarılı");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(final_total, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert(
        "message",
        "Garanti Bankası ile ödemeniz başarıyla tamamlandı!",
    );
    context.0.insert("provider_name", "Garanti Bankası");
    context.0.insert(
        "cargo_price_formatted",
        &format_price(cargo_price, &cargo_currency),
    );
    context.0.insert(
        "product_total_formatted",
        &format_price(product_total, &cart_currency),
    );
    context.0.insert("is_free_shipping", &(cargo_price == 0.0));

    match state.render_frontend_template("cart/payment/payment_success.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

/// GET /payment-provider/garanti/failure/{payment_url} - Garanti ödeme başarısız sayfası
pub async fn garanti_payment_failure(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    eprintln!(
        "Garanti payment failure page requested for: {}",
        payment_url
    );

    // Kullanıcı giriş yapmış mı kontrol et
    let user_id = match user_id {
        Some(id) => id,
        None => {
            eprintln!("User not logged in for Garanti failure page");
            return Redirect::to("/login").into_response();
        }
    };

    // Payment URL ile cart'ı bul ve kullanıcıya ait olduğunu doğrula
    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => {
            eprintln!(
                "Cart not found or not owned by user for payment_url: {}",
                payment_url
            );
            return Redirect::to("/my-cart").into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {:?}", e);
            return Redirect::to("/my-cart").into_response();
        }
    };

    // Cart items bilgilerini al
    let cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            eprintln!("Failed to get cart items for failure page");
            return Redirect::to("/my-cart").into_response();
        }
    };

    let total_amount: f64 = cart_response.items.iter().map(|item| item.total).sum();
    let cart_currency = cart_response.currency.clone();

    // Callback data'dan hata mesajını al
    let error_details = cart
        .callback_data
        .as_ref()
        .and_then(|cd| cd.get("error_message"))
        .and_then(|em| em.as_str())
        .unwrap_or("Bilinmeyen hata");

    context
        .0
        .insert("title", "Garanti Bankası - Ödeme Başarısız");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(total_amount, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert(
        "message",
        "Garanti Bankası ile ödeme işlemi başarısız oldu. Lütfen tekrar deneyin.",
    );
    context.0.insert("error_details", error_details);
    context.0.insert("provider_name", "Garanti Bankası");

    match state.render_frontend_template("cart/payment/payment_failure.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

use crate::app_state::AppState;
use crate::middleware::global_context::ViewContext;
use crate::modules::ecommerce::models::cart::{self, Entity as Cart};
use crate::modules::ecommerce::services::cart_service;
use crate::modules::payment_provider::models::{IyzicoConfig, PaymentProviderType};
use crate::modules::payment_provider::providers::iyzico::IyzicoProvider;
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

#[derive(Deserialize, Debug)]
pub struct IyzicoPaymentCallbackForm {
    // İyzico callback alanları
    pub status: Option<String>,
    #[serde(rename = "conversationId")]
    pub conversation_id: Option<String>,
    #[serde(rename = "paymentId")]
    pub payment_id: Option<String>,
    #[serde(rename = "paidPrice")]
    pub paid_price: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
    pub token: Option<String>,
    // Diğer tüm alanları da kabul et
    #[serde(flatten)]
    pub other: HashMap<String, String>,
}

/// POST /payment-provider/iyzico/callback/{payment_url} - İyzico callback
pub async fn iyzico_payment_callback(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Form(callback_data): Form<IyzicoPaymentCallbackForm>,
) -> impl IntoResponse {
    eprintln!("========================================");
    eprintln!("İyzico payment callback received for {}", payment_url);
    eprintln!("Callback data: {:?}", callback_data);
    eprintln!("========================================");

    // Cart'ı payment_url ile bul
    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => {
            eprintln!("Cart not found for payment_url: {}", payment_url);
            return Redirect::to(&format!("/payment-provider/iyzico/failure/{}", payment_url))
                .into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {:?}", e);
            return Redirect::to(&format!("/payment-provider/iyzico/failure/{}", payment_url))
                .into_response();
        }
    };

    let cart_id = cart.id;
    let user_id = cart.user_id;

    // Callback data'yı JSON'a çevir
    let callback_json = {
        let mut json_data = serde_json::Map::new();

        if let Some(status) = &callback_data.status {
            json_data.insert(
                "status".to_string(),
                serde_json::Value::String(status.clone()),
            );
        }
        if let Some(cid) = &callback_data.conversation_id {
            json_data.insert(
                "conversation_id".to_string(),
                serde_json::Value::String(cid.clone()),
            );
        }
        if let Some(pid) = &callback_data.payment_id {
            json_data.insert(
                "payment_id".to_string(),
                serde_json::Value::String(pid.clone()),
            );
        }
        if let Some(price) = &callback_data.paid_price {
            json_data.insert(
                "paid_price".to_string(),
                serde_json::Value::String(price.clone()),
            );
        }
        if let Some(error) = &callback_data.error_message {
            json_data.insert(
                "error_message".to_string(),
                serde_json::Value::String(error.clone()),
            );
        }
        if let Some(token) = &callback_data.token {
            json_data.insert(
                "token".to_string(),
                serde_json::Value::String(token.clone()),
            );
        }

        for (key, value) in &callback_data.other {
            json_data.insert(key.clone(), serde_json::Value::String(value.clone()));
        }

        json_data.insert(
            "callback_timestamp".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );
        json_data.insert(
            "provider".to_string(),
            serde_json::Value::String("iyzico".to_string()),
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

    // Token varsa İyzico API'den ödeme sonucunu sorgula
    let token = match &callback_data.token {
        Some(t) => t.clone(),
        None => {
            eprintln!("İyzico callback: No token received!");
            return Redirect::to(&format!("/payment-provider/iyzico/failure/{}", payment_url))
                .into_response();
        }
    };

    eprintln!(
        "İyzico callback: Retrieving payment details for token: {}",
        token
    );

    // İyzico config'i yükle
    let iyzico_config =
        match PaymentProviderService::get_provider_config(&state.db, PaymentProviderType::Iyzico)
            .await
        {
            Ok(config) => {
                let iyzico_config: IyzicoConfig = match serde_json::from_value(config.config) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to parse iyzico config: {:?}", e);
                        return Redirect::to(&format!(
                            "/payment-provider/iyzico/failure/{}",
                            payment_url
                        ))
                        .into_response();
                    }
                };
                iyzico_config
            }
            Err(e) => {
                eprintln!("Failed to load iyzico config: {:?}", e);
                return Redirect::to(&format!("/payment-provider/iyzico/failure/{}", payment_url))
                    .into_response();
            }
        };

    // Token ile ödeme sonucunu sorgula
    let payment_result = match IyzicoProvider::retrieve_payment(iyzico_config, &token).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to retrieve payment: {:?}", e);
            return Redirect::to(&format!("/payment-provider/iyzico/failure/{}", payment_url))
                .into_response();
        }
    };

    eprintln!(
        "İyzico payment result: success={}, status={}, payment_status={:?}",
        payment_result.success, payment_result.status, payment_result.payment_status
    );

    // Ödeme sonucunu da callback_data'ya ekle
    let _ = cart::Entity::update_many()
        .col_expr(
            cart::Column::CallbackData,
            Expr::value(sea_orm::prelude::Json::from(
                payment_result.raw_response.clone(),
            )),
        )
        .col_expr(cart::Column::UpdatedAt, Expr::value(chrono::Utc::now()))
        .filter(cart::Column::Id.eq(cart_id))
        .exec(&state.db)
        .await;

    if payment_result.success {
        eprintln!("İyzico payment successful! Completing order...");

        // Ödeme notu oluştur
        let payment_note = format!(
            "İyzico ile ödeme yapıldı. Payment ID: {}",
            payment_result.payment_id.as_deref().unwrap_or("-")
        );

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

                eprintln!("Status updated to 'preparing'");
                return Redirect::to(&format!("/payment-provider/iyzico/success/{}", payment_url))
                    .into_response();
            }
            Err(e) => {
                eprintln!("Failed to complete order: {:?}", e);
                // Sipariş zaten tamamlanmış olabilir
                let existing_cart = cart::Entity::find()
                    .filter(cart::Column::PaymentUrl.eq(&payment_url))
                    .one(&state.db)
                    .await;

                if let Ok(Some(c)) = existing_cart {
                    if c.status != "open_cart" {
                        eprintln!("Order already processed, status: {}", c.status);
                        return Redirect::to(&format!(
                            "/payment-provider/iyzico/success/{}",
                            payment_url
                        ))
                        .into_response();
                    }
                }

                return Redirect::to(&format!("/payment-provider/iyzico/failure/{}", payment_url))
                    .into_response();
            }
        }
    } else {
        eprintln!(
            "İyzico payment failed - status: {}, payment_status: {:?}",
            payment_result.status, payment_result.payment_status
        );
        if let Some(error_msg) = &payment_result.error_message {
            eprintln!("İyzico error message: {}", error_msg);
        }
        return Redirect::to(&format!("/payment-provider/iyzico/failure/{}", payment_url))
            .into_response();
    }
}

/// GET /payment-provider/iyzico/success/{payment_url} - İyzico ödeme başarılı sayfası
pub async fn iyzico_payment_success(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    eprintln!("İyzico payment success page requested for: {}", payment_url);

    let user_id = match user_id {
        Some(id) => id,
        None => {
            return Redirect::to("/login").into_response();
        }
    };

    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
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
            return Redirect::to("/my-cart").into_response();
        }
    };

    // total_amount cart'tan al (ürün toplamı)
    let total_amount = cart
        .total_amount
        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0);

    // Currency'yi cart'tan al (sipariş tamamlandığında kaydedilmiş olmalı)
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

    context.0.insert("title", "İyzico - Ödeme Başarılı");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(final_total, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context
        .0
        .insert("message", "İyzico ile ödemeniz başarıyla tamamlandı!");
    context.0.insert("provider_name", "İyzico");
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

/// GET /payment-provider/iyzico/failure/{payment_url} - İyzico ödeme başarısız sayfası
pub async fn iyzico_payment_failure(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    eprintln!("İyzico payment failure page requested for: {}", payment_url);

    let user_id = match user_id {
        Some(id) => id,
        None => {
            return Redirect::to("/login").into_response();
        }
    };

    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
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
            return Redirect::to("/my-cart").into_response();
        }
    };

    // total_amount cart'tan al, yoksa hesapla
    let total_amount = cart
        .total_amount
        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
        .unwrap_or_else(|| cart_response.items.iter().map(|item| item.total).sum());

    let cart_currency = cart_response.currency.clone();

    let error_details = cart
        .callback_data
        .as_ref()
        .and_then(|cd| cd.get("errorMessage"))
        .and_then(|em| em.as_str())
        .unwrap_or("Bilinmeyen hata");

    context.0.insert("title", "İyzico - Ödeme Başarısız");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(total_amount, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert(
        "message",
        "İyzico ile ödeme işlemi başarısız oldu. Lütfen tekrar deneyin.",
    );
    context.0.insert("error_details", error_details);
    context.0.insert("provider_name", "İyzico");

    match state.render_frontend_template("cart/payment/payment_failure.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

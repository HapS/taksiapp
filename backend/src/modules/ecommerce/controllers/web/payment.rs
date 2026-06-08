use std::i64;

use crate::app_state::AppState;
use crate::middleware::global_context::ViewContext;
use crate::modules::ecommerce::models::cart::{self, Entity as Cart};
use crate::modules::ecommerce::services::cart_service;
use crate::modules::ecommerce::services::cart_service::CartResponse;
use crate::modules::payment_provider::models::{BasketItem, PaymentProviderType, PaymentRequest};
use crate::modules::payment_provider::services::PaymentProviderService;
use crate::modules::utils::format_price::format_price;
use axum::{
    extract::{Form, Path, State},
    response::{Html, IntoResponse, Redirect},
    Extension,
};
use rust_decimal::Decimal;
use sea_orm::*;
use serde::Deserialize;

/// GET /payment/credit-card/{payment_url} - Online kredi kartı ödeme sayfası
pub async fn credit_card_payment(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    //user_id bir Option tipinde Some(id) veya None olabilir
    //eğer Some(id) ise içindeki id değerini user_id değişkenine atar
    //ama None ise kullanıcıyı /login sayfasına yönlendirir
    // user_id yi axum un Extension ı ile alıyoruz böyle alınca fonksiyon boyunca user_id kullanabiliriz
    // çünkü axum bunu .clone olark veriyor ve işlem bitene kadar koruyor
    let user_id = match user_id {
        Some(id) => id,
        None => return Redirect::to("/login").into_response(),
    };

    // Payment URL ile cart'ı bul
    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq("open_cart"))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => return Redirect::to("/my-cart").into_response(),
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    //cart items ve total amount hesapla (kullanıcının B2B/B2C durumuna göre)
    let mut cart_response: CartResponse = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Kampanya motorunu çalıştır
    let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
    let applied_coupon_code = crate::modules::ecommerce::controllers::api::cart::get_applied_coupon_code(&state.db, cart.id).await;
    let raw_cargo_fee_decimal = Decimal::from_f64_retain(cart_response.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
    
    if let Ok(eval_result) = engine.evaluate(cart.id, user_id, applied_coupon_code.as_deref(), true, &cart_response.currency, raw_cargo_fee_decimal).await {
        let summary = eval_result.summary;
        cart_response.final_total = summary.total.to_string().parse::<f64>().unwrap_or(cart_response.final_total);
        cart_response.final_total_formatted = summary.total_formatted.clone();
        cart_response.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
        cart_response.standart_cargo_fee_formatted = Some(summary.cargo_fee_formatted.clone());
        cart_response.is_free_shipping = summary.free_shipping;
        cart_response.campaign_summary = Some(summary);
    }

    let cart_currency = cart_response.currency.clone();
    let total_amount: f64 = cart_response.final_total;
    let standart_cargo_fee = cart_response.standart_cargo_fee.unwrap_or(0.0);
    let standart_cargo_fee_formatted = cart_response
        .standart_cargo_fee_formatted
        .unwrap_or_else(|| format_price(0.0, &cart_currency));
    let is_free_shipping = cart_response.is_free_shipping;
    let total_discount = cart_response.campaign_summary.as_ref().map(|s| s.total_discount.to_string().parse::<f64>().unwrap_or(0.0)).unwrap_or(0.0);
    let final_total_formatted = cart_response.final_total_formatted;

    // Default payment provider'ı al
    let default_provider = PaymentProviderService::get_default_provider(&state.db)
        .await
        .unwrap_or(PaymentProviderType::Iyzico); //unwarap böyle kullanılır cloudflaredakiler unwrap demiş bırakmış program çöker olmuyorsa çöksün lazım olan bir şey aslında

    eprintln!("Using payment provider: {:?}", default_provider);

    // Cart bilgilerini context'e ekle
    context.0.insert("title", "Online Kredi Kartı Ödemesi");
    context.0.insert("cart", &cart);

    // Cart items'ları formatlanmış fiyatlarla birlikte hazırla
    let formatted_cart_items: Vec<serde_json::Value> = cart_response
        .items
        .iter()
        .map(|item| {
            serde_json::json!({
                "id": item.id,
                "product_title": item.product_title,
                "quantity": item.quantity,
                "price": item.price,
                "total": item.total,
                "formatted_total": format_price(item.total, &cart_currency),
                "product_id": item.product_id,
                "variant_key": item.variant_key
            })
        })
        .collect();

    context.0.insert("cart_items", &formatted_cart_items);
    context.0.insert("payment_url", &payment_url);
    context
        .0
        .insert("total_amount", &format_price(total_amount, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context
        .0
        .insert("payment_provider", &default_provider.as_str());
    context.0.insert(
        "standart_cargo_fee_formatted",
        &standart_cargo_fee_formatted,
    );
    context.0.insert("is_free_shipping", &is_free_shipping);
    context
        .0
        .insert("final_total_formatted", &final_total_formatted);
    context.0.insert(
        "product_total_formatted",
        &format_price(total_amount - standart_cargo_fee, &cart_currency),
    );

    // Provider'a göre template seç ve form HTML'ini hazırla
    let (template_path, form_html) = match default_provider {
        PaymentProviderType::Garanti => ("cart/payment/garanti/garanti_credit_card.html", None),

        //iyzico neden bu kadar uzun? çünkü form init işlemi için bi dünya bir şey istiyor
        // yarın öbür gün buraya paytr ve kripto ödemeleri gelecek, eth ile ödeyecem btc ile ödeyecem vs.
        PaymentProviderType::Iyzico => {
            // Kullanıcı bilgilerini al (users tablosu)
            let user = match crate::modules::auth::models::User::find_by_id(user_id)
                .one(&state.db)
                .await
            {
                Ok(Some(user)) => user,
                Ok(None) => return Redirect::to("/login").into_response(),
                Err(_) => return Redirect::to("/my-cart").into_response(),
            };

            // Müşteri adı (users tablosu)
            let customer_name = format!(
                "{} {}",
                user.first_name.as_deref().unwrap_or("Müşteri"),
                user.last_name.as_deref().unwrap_or("")
            )
            .trim()
            .to_string();
            let customer_name = if customer_name.is_empty() {
                "Müşteri".to_string()
            } else {
                customer_name
            };

            // Müşteri email (users tablosu)
            let customer_email = user.email.clone();

            // Adres bilgilerini al (addresses, cities, districts, countries tabloları)
            let (
                customer_phone,
                customer_address,
                customer_city,
                customer_country,
                customer_district,
                // New unified address fields
                invoice_type,
                tax_office,
                tax_number,
                company_name,
                id_number,
            ) = if let Some(address_id) = cart.address_id {
                use crate::modules::ecommerce::models::{
                    address::Entity as Address, city::Entity as City, country::Entity as Country,
                    district::Entity as District,
                };

                // Eğer invoice_id varsa (fatura adresi farklıysa), onu kullan. Yoksa address_id (teslimat adresi) kullan.
                let target_address_id = cart.invoice_id.unwrap_or(address_id);

                match Address::find_by_id(target_address_id).one(&state.db).await {
                    Ok(Some(address)) => {
                        // Telefon (addresses tablosu)
                        let phone =
                            format!("{}{}", address.phone_country_code, address.phone_number);

                        // Şehir adı (cities tablosu)
                        let city_name = match City::find_by_id(address.city_id).one(&state.db).await
                        {
                            Ok(Some(city)) => city.name,
                            _ => "Istanbul".to_string(),
                        };

                        // İlçe adı (districts tablosu)
                        let district_name = match District::find_by_id(address.district_id)
                            .one(&state.db)
                            .await
                        {
                            Ok(Some(district)) => Some(district.name),
                            _ => None,
                        };

                        // Ülke adı (countries tablosu)
                        let country_name =
                            match Country::find_by_id(address.country_id).one(&state.db).await {
                                Ok(Some(country)) => country.name,
                                _ => "Turkey".to_string(),
                            };

                        (
                            phone,
                            Some(address.address_line.clone()),
                            Some(city_name),
                            Some(country_name),
                            district_name,
                            Some(address.address_type),
                            address.tax_office,
                            address.tax_number,
                            address.company_name,
                            address.id_number,
                        )
                    }
                    _ => (
                        "+905001234567".to_string(),
                        None,
                        None,
                        Some("Turkey".to_string()),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ),
                }
            } else {
                (
                    "+905001234567".to_string(),
                    None,
                    None,
                    Some("Turkey".to_string()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            };

            // Tam adres oluştur (adres satırı + ilçe)
            let full_address = match (&customer_address, &customer_district) {
                (Some(addr), Some(dist)) => Some(format!("{}, {}", addr, dist)),
                (Some(addr), None) => Some(addr.clone()),
                _ => None,
            };

            // Total amount'ı cart_response.final_total'dan al (indirimli ve kargo dahil gerçek tutar)
            let total_amount: f64 = cart_response.final_total;
            
            // Ürünlerin ham toplamını hesapla (indirimleri dağıtmak için)
            let product_subtotal: f64 = cart_response.items.iter().map(|item| item.total).sum();
            
            // İndirim oranını hesapla
            let discount_ratio = if product_subtotal > 0.0 {
                (product_subtotal - total_discount) / product_subtotal
            } else {
                1.0
            };

            // Sepet ürünlerini BasketItem'a dönüştür ve indirimi yansıt
            let mut basket_items: Vec<BasketItem> = cart_response
                .items
                .iter()
                .map(|item| {
                    let product_name = match &item.variant_display {
                        Some(variant) if !variant.is_empty() => {
                            format!("{} - {}", item.product_title, variant)
                        }
                        _ => item.product_title.clone(),
                    };

                    // İndirimli birim fiyatı hesapla ve 2 ondalık basamağa yuvarla
                    // Not: (item.total * discount_ratio) / quantity = item.price * discount_ratio
                    let adjusted_total = (item.total * discount_ratio * 100.0).round() / 100.0;

                    BasketItem {
                        id: format!("BI{}", item.product_id),
                        name: product_name,
                        category1: "Ürün".to_string(),
                        category2: item.variant_display.clone(),
                        item_type: "PHYSICAL".to_string(),
                        price: format!("{:.2}", adjusted_total),
                    }
                })
                .collect();

            // Kargo ücretini basket item olarak ekle
            if !is_free_shipping && standart_cargo_fee > 0.0 {
                basket_items.push(BasketItem {
                    id: "KARGO".to_string(),
                    name: "Kargo Ücreti".to_string(),
                    category1: "Kargo".to_string(),
                    category2: None,
                    item_type: "VIRTUAL".to_string(),
                    price: format!("{:.2}", standart_cargo_fee),
                });
            }

            // Yuvarlama hatalarını önlemek için son bir kontrol: Basket item toplamı total_amount ile eşleşmeli
            let basket_sum: f64 = basket_items.iter().map(|item| item.price.parse::<f64>().unwrap_or(0.0)).sum();
            let diff = total_amount - basket_sum;
            if diff.abs() > 0.001 && !basket_items.is_empty() {
                // Farkı ilk fiziksel ürüne ekle/çıkar
                if let Some(first_item) = basket_items.iter_mut().find(|i| i.item_type == "PHYSICAL") {
                    let current_price = first_item.price.parse::<f64>().unwrap_or(0.0);
                    first_item.price = format!("{:.2}", current_price + diff);
                }
            }

            // Base URL'yi al ve logla
            let base_url = state.config.get_base_url();
            eprintln!("Base URL from config: {}", base_url);

            // Kullanıcının IP adresini al (user.ip veya varsayılan)
            let customer_ip = user.ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());

            // TCKN belirle: Adresten geleni kullan, yoksa request'ten (burada request yok ama struct'ta vardı), yoksa varsayılan
            let identity_number = id_number.or(Some("11111111111".to_string()));

            // İyzico için checkout form'u oluştur
            let payment_request = PaymentRequest {
                order_id: cart
                    .order_id
                    .clone()
                    .unwrap_or_else(|| format!("ORD{}", cart.id)),
                amount: total_amount,
                currency: cart_currency.clone(),
                customer_name,
                customer_email,
                customer_phone,
                customer_id: format!("USR{}", user.id),
                customer_ip,
                customer_identity_number: identity_number,
                customer_city,
                customer_country,
                customer_address: full_address,
                customer_zip_code: Some("34000".to_string()),
                // Unified Corporate Info
                invoice_type,
                tax_office,
                tax_number,
                company_name,

                success_url: format!(
                    "{}/payment-provider/iyzico/success/{}",
                    base_url, payment_url
                ),
                failure_url: format!(
                    "{}/payment-provider/iyzico/failure/{}",
                    base_url, payment_url
                ),
                callback_url: format!(
                    "{}/payment-provider/iyzico/callback/{}",
                    base_url, payment_url
                ),
                basket_items,
            };

            // eprintln!("Callback URL: {}", payment_request.callback_url);

            // println!("{:?}", &payment_request);

            // İyzico checkout form'u oluştur
            match PaymentProviderService::initiate_payment(
                &state.db,
                Some(PaymentProviderType::Iyzico),
                payment_request,
                None,
            )
            .await
            {
                Ok(response) if response.success => {
                    let form_html = response
                        .provider_response
                        .get("form_html")
                        .and_then(|f| f.as_str())
                        .map(|s| s.to_string());
                    ("cart/payment/iyzico/iyzico_credit_card.html", form_html)
                }
                Ok(_) | Err(_) => ("cart/payment/iyzico/iyzico_credit_card.html", None),
            }
        }
        PaymentProviderType::PayTR => {
            // PayTR için iframe token oluştur
            // Kullanıcı bilgilerini al
            let user = match crate::modules::auth::models::User::find_by_id(user_id)
                .one(&state.db)
                .await
            {
                Ok(Some(user)) => user,
                Ok(None) => return Redirect::to("/login").into_response(),
                Err(_) => return Redirect::to("/my-cart").into_response(),
            };

            // Müşteri adı
            let customer_name = format!(
                "{} {}",
                user.first_name.as_deref().unwrap_or("Müşteri"),
                user.last_name.as_deref().unwrap_or("")
            )
            .trim()
            .to_string();
            let customer_name = if customer_name.is_empty() {
                "Müşteri".to_string()
            } else {
                customer_name
            };

            let customer_email = user.email.clone();
            let customer_ip = user.ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());

            // Adres bilgilerini al
            let (customer_phone, customer_address) = if let Some(address_id) = cart.address_id {
                use crate::modules::ecommerce::models::address::Entity as Address;

                match Address::find_by_id(address_id).one(&state.db).await {
                    Ok(Some(address)) => {
                        let phone = format!("{}{}", address.phone_country_code, address.phone_number);
                        (phone, Some(address.address_line.clone()))
                    }
                    _ => ("+905001234567".to_string(), None),
                }
            } else {
                ("+905001234567".to_string(), None)
            };

            // Sepet ürünlerini basket item olarak hazırla
            let basket_items: Vec<BasketItem> = cart_response
                .items
                .iter()
                .map(|item| {
                    let product_name = match &item.variant_display {
                        Some(variant) if !variant.is_empty() => {
                            format!("{} - {}", item.product_title, variant)
                        }
                        _ => item.product_title.clone(),
                    };
                    let rounded_price = (item.total * 100.0).round() / 100.0;

                    BasketItem {
                        id: format!("BI{}", item.product_id),
                        name: product_name,
                        category1: "Ürün".to_string(),
                        category2: item.variant_display.clone(),
                        item_type: "PHYSICAL".to_string(),
                        price: format!("{:.2}", rounded_price),
                    }
                })
                .collect();

            // Base URL'yi al
            let base_url = state.config.get_base_url();

            // PayTR için payment request (merchant_oid olarak cart.order_id kullanılıyor)
            let merchant_oid = cart.order_id.clone().unwrap_or_else(|| format!("ORD{}", cart.id));
            
            let payment_request = PaymentRequest {
                order_id: merchant_oid, // PayTR için merchant_oid - alfanümerik
                amount: total_amount,
                currency: cart_currency.clone(),
                customer_name,
                customer_email,
                customer_phone,
                customer_id: format!("USR{}", user.id),
                customer_ip,
                customer_identity_number: None,
                customer_city: None,
                customer_country: None,
                customer_address,
                customer_zip_code: None,
                invoice_type: None,
                tax_office: None,
                tax_number: None,
                company_name: None,
                success_url: format!("{}/payment-provider/paytr/success/{}", base_url, payment_url),
                failure_url: format!("{}/payment-provider/paytr/failure/{}", base_url, payment_url),
                callback_url: format!("{}/payment-provider/paytr/callback", base_url),
                basket_items,
            };

            // PayTR iframe token al
            match PaymentProviderService::initiate_payment(
                &state.db,
                Some(PaymentProviderType::PayTR),
                payment_request,
                None,
            )
            .await
            {
                Ok(response) if response.success => {
                    // iframe URL'ini context'e ekle
                    if let Some(iframe_url) = response.payment_url {
                        context.0.insert("paytr_iframe_url", &iframe_url);
                    }
                    if let Some(token) = response.token {
                        context.0.insert("paytr_token", &token);
                    }
                    ("cart/payment/paytr/paytr_credit_card.html", None)
                }
                Ok(response) => {
                    eprintln!("PayTR error: {:?}", response.error_message);
                    context.0.insert("error_message", &response.error_message.unwrap_or_else(|| "PayTR bağlantı hatası".to_string()));
                    ("cart/payment/paytr/paytr_credit_card.html", None)
                }
                Err(e) => {
                    eprintln!("PayTR initiate_payment error: {:?}", e);
                    context.0.insert("error_message", &format!("PayTR hatası: {}", e));
                    ("cart/payment/paytr/paytr_credit_card.html", None)
                }
            }
        }
    };

    // Form HTML'ini context'e ekle
    if let Some(html) = form_html {
        context.0.insert("form_html", &html);
    }

    match state.render_template(template_path, &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

#[derive(Deserialize)]
pub struct PaymentProcessForm {
    // Kredi kartı bilgileri
    pub cardnumber: Option<String>,
    pub cardexpiredatemonth: Option<String>,
    pub cardexpiredateyear: Option<String>,
    pub cardcvv2: Option<String>,
    pub cardholdername: Option<String>,
}

/// POST /payment/credit-card/{payment_url} - Kredi kartı ödeme işlemini başlat
pub async fn process_credit_card_payment(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    //auth_user: crate::middleware::auth::AuthenticatedUser,
    Form(form): Form<PaymentProcessForm>,
) -> impl IntoResponse {
    //bu da değişik kontrol
    //user_id bir Option tipinde Some(id) veya None olabilir
    //eğer Some(id) ise içindeki id değerini user_id değişkenine atar
    //ama None ise kullanıcıyı /login sayfasına yönlendirir
    // user_id yi axum un Extension ı ile alıyoruz böyle alınca fonksiyon boyunca user_id kullanabiliriz
    // çünkü axum bunu .clone olark veriyor ve işlem bitene kadar koruyor
    let user_id = match user_id {
        Some(id) => id,
        None => return Redirect::to("/login").into_response(),
    };

    eprintln!("Credit card payment process started:");
    eprintln!("  Payment URL: {}", payment_url);

    // Payment URL ile cart'ı bul
    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq("open_cart"))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => return Redirect::to("/my-cart").into_response(),
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // if cart.status != "open_cart" {
    //     println!("Cart status open değil: {}", cart.status);
    //     return Redirect::to("/my-cart").into_response();
    // }

    // Cart items ve total amount hesapla (kullanıcının B2B/B2C durumuna göre)
    let mut cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Kampanya motorunu çalıştır
    let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
    let applied_coupon_code = crate::modules::ecommerce::controllers::api::cart::get_applied_coupon_code(&state.db, cart.id).await;
    let raw_cargo_fee_decimal = Decimal::from_f64_retain(cart_response.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
    
    if let Ok(eval_result) = engine.evaluate(cart.id, user_id, applied_coupon_code.as_deref(), true, &cart_response.currency, raw_cargo_fee_decimal).await {
        let summary = eval_result.summary;
        cart_response.final_total = summary.total.to_string().parse::<f64>().unwrap_or(cart_response.final_total);
        cart_response.final_total_formatted = summary.total_formatted.clone();
        cart_response.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
        cart_response.standart_cargo_fee_formatted = Some(summary.cargo_fee_formatted.clone());
        cart_response.is_free_shipping = summary.free_shipping;
        cart_response.campaign_summary = Some(summary);
    }

    let total_amount: f64 = cart_response.final_total;
    let total_discount = cart_response.campaign_summary.as_ref().map(|s| s.total_discount.to_string().parse::<f64>().unwrap_or(0.0)).unwrap_or(0.0);

    // Kullanıcı bilgilerini al - gerçek user tablosundan
    let user = match crate::modules::auth::models::User::find_by_id(user_id)
        .one(&state.db)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            eprintln!("User not found: {}", user_id);
            return Redirect::to("/login").into_response();
        }
        Err(e) => {
            eprintln!("Veritabanı hatası while fetching user: {:?}", e);
            return Redirect::to("/my-cart").into_response();
        }
    };

    let customer_name = form.cardholdername.clone().unwrap_or_else(|| {
        format!(
            "{} {}",
            user.first_name.as_deref().unwrap_or(""),
            user.last_name.as_deref().unwrap_or("")
        )
        .trim()
        .to_string()
    });
    let customer_email = user.email.clone();
    // Telefon bilgisini öncelik sırasına göre al: Cart adresi > User profile > Default
    let customer_phone = if let Some(address_id) = cart.address_id {
        // Cart'ta adres seçilmişse, o adresten telefon al
        match crate::modules::ecommerce::models::address::Entity::find_by_id(address_id)
            .one(&state.db)
            .await
        {
            Ok(Some(address)) => {
                format!("{}{}", address.phone_country_code, address.phone_number)
            }
            _ => {
                // Adres bulunamazsa user profile'dan al
                user.profile
                    .as_ref()
                    .and_then(|p| p.get("phone"))
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "+905551234567".to_string())
            }
        }
    } else {
        // Cart'ta adres seçilmemişse user profile'dan al
        user.profile
            .as_ref()
            .and_then(|p| p.get("phone"))
            .and_then(|p| p.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "+905551234567".to_string())
    };

    // Default payment provider'ı al
    let default_provider = PaymentProviderService::get_default_provider(&state.db)
        .await
        .unwrap_or(PaymentProviderType::Iyzico);

    eprintln!("Using payment provider: {:?}", default_provider);

    // Payment request oluştur - provider'a göre callback URL'leri ayarla
    let (success_url, failure_url, callback_url) = match default_provider {
        PaymentProviderType::Garanti => (
            format!(
                "{}/payment-provider/garanti/success/{}",
                state.config.get_base_url(),
                payment_url
            ),
            format!(
                "{}/payment-provider/garanti/failure/{}",
                state.config.get_base_url(),
                payment_url
            ),
            format!(
                "{}/payment-provider/garanti/callback/{}",
                state.config.get_base_url(),
                payment_url
            ),
        ),
        PaymentProviderType::Iyzico => (
            format!(
                "{}/payment-provider/iyzico/success/{}",
                state.config.get_base_url(),
                payment_url
            ),
            format!(
                "{}/payment-provider/iyzico/failure/{}",
                state.config.get_base_url(),
                payment_url
            ),
            format!(
                "{}/payment-provider/iyzico/callback/{}",
                state.config.get_base_url(),
                payment_url
            ),
        ),
        PaymentProviderType::PayTR => (
            format!(
                "{}/payment-provider/paytr/success/{}",
                state.config.get_base_url(),
                payment_url
            ),
            format!(
                "{}/payment-provider/paytr/failure/{}",
                state.config.get_base_url(),
                payment_url
            ),
            format!(
                "{}/payment-provider/paytr/callback",
                state.config.get_base_url()
            ),
        ),
    };

    // Kullanıcının IP adresini al
    let customer_ip = user.ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());

    // Cart currency'yi al
    let cart_currency = cart_response.currency.clone();

    // Payment request oluştur
    let payment_request = PaymentRequest {
        order_id: cart.order_id.clone().unwrap_or_default(),
        amount: total_amount,
        currency: cart_currency,
        customer_name,
        customer_email,
        customer_phone,
        customer_id: format!("USR{}", user_id),
        customer_ip,
        customer_identity_number: None,
        customer_city: None,
        customer_country: Some("Turkey".to_string()),
        customer_address: None,
        customer_zip_code: None,
        success_url,
        failure_url,
        callback_url,
        basket_items: {
            let product_subtotal: f64 = cart_response.items.iter().map(|item| item.total).sum();
            let discount_ratio = if product_subtotal > 0.0 { (product_subtotal - total_discount) / product_subtotal } else { 1.0 };
            
            let mut items: Vec<BasketItem> = cart_response.items.iter().map(|item| {
                let adjusted_total = (item.total * discount_ratio * 100.0).round() / 100.0;
                BasketItem {
                    id: format!("BI{}", item.product_id),
                    name: item.product_title.clone(),
                    category1: "Ürün".to_string(),
                    category2: None,
                    item_type: "PHYSICAL".to_string(),
                    price: format!("{:.2}", adjusted_total),
                }
            }).collect();
            
            // Kargo ekle
            if !cart_response.is_free_shipping && cart_response.standart_cargo_fee.unwrap_or(0.0) > 0.0 {
                items.push(BasketItem {
                    id: "KARGO".to_string(),
                    name: "Kargo Ücreti".to_string(),
                    category1: "Kargo".to_string(),
                    category2: None,
                    item_type: "VIRTUAL".to_string(),
                    price: format!("{:.2}", cart_response.standart_cargo_fee.unwrap_or(0.0)),
                });
            }
            
            // Yuvarlama farkı düzelt
            let basket_sum: f64 = items.iter().map(|i| i.price.parse::<f64>().unwrap_or(0.0)).sum();
            let diff = total_amount - basket_sum;
            if diff.abs() > 0.001 && !items.is_empty() {
                if let Some(first) = items.iter_mut().find(|i| i.item_type == "PHYSICAL") {
                    let cur = first.price.parse::<f64>().unwrap_or(0.0);
                    first.price = format!("{:.2}", cur + diff);
                }
            }
            items
        },
        invoice_type: None,
        tax_office: None,
        tax_number: None,
        company_name: None,
    };

    // Kredi kartı bilgilerini hazırla
    let mut card_data = std::collections::HashMap::new();
    if let Some(cardnumber) = &form.cardnumber {
        card_data.insert("cardnumber".to_string(), cardnumber.clone());
    }
    if let Some(month) = &form.cardexpiredatemonth {
        card_data.insert("cardexpiredatemonth".to_string(), month.clone());
    }
    if let Some(year) = &form.cardexpiredateyear {
        card_data.insert("cardexpiredateyear".to_string(), year.clone());
    }
    if let Some(cvv) = &form.cardcvv2 {
        card_data.insert("cardcvv2".to_string(), cvv.clone());
    }
    if let Some(name) = &form.cardholdername {
        card_data.insert("cardholdername".to_string(), name.clone());
    }

    eprintln!("Payment request created:");
    eprintln!("  Order ID: {}", payment_request.order_id);
    eprintln!("  Amount: {}", payment_request.amount);
    eprintln!("  Success URL: {}", payment_request.success_url);
    eprintln!("  Failure URL: {}", payment_request.failure_url);
    eprintln!("  Card data provided: {}", !card_data.is_empty());

    // Payment provider service kullan - belirtilen provider'ı kullan
    match PaymentProviderService::initiate_payment(
        &state.db,
        Some(default_provider),
        payment_request,
        Some(card_data),
    )
    .await
    {
        Ok(response) => {
            eprintln!("Payment response: {:?}", response);
            if response.success {
                if let Some(payment_url) = response.payment_url {
                    // Başarılı - payment provider'ın sayfasına yönlendir (İyzico için)
                    eprintln!("Redirecting to payment URL: {}", payment_url);
                    return Redirect::to(&payment_url).into_response();
                } else if let Some(form_html) = response.provider_response.get("form_html") {
                    // HTML form döndürüldü (Garanti için)
                    if let Some(html_str) = form_html.as_str() {
                        eprintln!("Returning HTML form for payment");
                        return Html(html_str.to_string()).into_response();
                    }
                } else {
                    eprintln!("Payment URL is None despite success=true");
                }
            } else {
                eprintln!("Payment failed: {:?}", response.error_message);
            }
            // Hata durumunda cart sayfasına dön
            Redirect::to("/my-cart").into_response()
        }
        Err(e) => {
            eprintln!("Payment provider error: {:?}", e);
            Redirect::to("/my-cart").into_response()
        }
    }
}

/// GET /payment/bank-transfer/{payment_url} - Banka transferi ödeme sayfası
pub async fn bank_transfer_payment(
    State(state): State<AppState>,
    auth_user: crate::middleware::auth::AuthenticatedUser,
    Path(payment_url): Path<String>,
    mut context: ViewContext,
) -> impl IntoResponse {
    let user_id = auth_user.id;

    if user_id != auth_user.id {
        eprintln!("User ID mismatch");
        return Redirect::to("/my-cart").into_response();
    }

    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq("open_cart"))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => return Redirect::to("/my-cart").into_response(),
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // if cart.status != "open_cart" {
    //     println!("Cart status open değil: {}", cart.status);
    //     return Redirect::to("/my-cart").into_response();
    // }

    // Get cart items and calculate total
    let mut cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Kampanya motorunu çalıştır
    let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
    let applied_coupon_code = crate::modules::ecommerce::controllers::api::cart::get_applied_coupon_code(&state.db, cart.id).await;
    let raw_cargo_fee_decimal = Decimal::from_f64_retain(cart_response.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
    
    if let Ok(eval_result) = engine.evaluate(cart.id, user_id, applied_coupon_code.as_deref(), true, &cart_response.currency, raw_cargo_fee_decimal).await {
        let summary = eval_result.summary;
        cart_response.final_total = summary.total.to_string().parse::<f64>().unwrap_or(cart_response.final_total);
        cart_response.final_total_formatted = summary.total_formatted;
        cart_response.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
        cart_response.standart_cargo_fee_formatted = Some(summary.cargo_fee_formatted);
        cart_response.is_free_shipping = summary.free_shipping;
    }

    let cart_currency = cart_response.currency.clone();
    let total_amount: f64 = cart_response.final_total;
    let standart_cargo_fee = cart_response.standart_cargo_fee.unwrap_or(0.0);
    let standart_cargo_fee_formatted = cart_response
        .standart_cargo_fee_formatted
        .unwrap_or_else(|| format_price(0.0, &cart_currency));
    let final_total_formatted = cart_response.final_total_formatted;
    let is_free_shipping = cart_response.is_free_shipping;

    // Get settings for bank information
    let settings =
        match crate::modules::admin::services::settings_service::get_settings(&state.db).await {
            Ok(settings) => settings,
            Err(_) => crate::modules::admin::models::settings::SettingsData::default(),
        };

    context.0.insert("title", "Banka Transferi");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(total_amount, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert("payment_url", &payment_url);
    context.0.insert("settings", &settings);
    context.0.insert(
        "standart_cargo_fee_formatted",
        &standart_cargo_fee_formatted,
    );
    context.0.insert("is_free_shipping", &is_free_shipping);
    context
        .0
        .insert("final_total_formatted", &final_total_formatted);
    context.0.insert(
        "product_total_formatted",
        &format_price(total_amount - standart_cargo_fee, &cart_currency),
    );

    match state.render_frontend_template("cart/payment/bank_transfer.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

/// GET /payment/cash-on-delivery/{payment_url} - Kapıda ödeme sayfası
pub async fn cash_on_delivery_payment(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    let user_id = match user_id {
        Some(id) => id,
        None => return Redirect::to("/my-cart").into_response(),
    };

    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq("open_cart"))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => return Redirect::to("/my-cart").into_response(),
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Get cart items and calculate total
    let mut cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Kampanya motorunu çalıştır
    let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
    let applied_coupon_code = crate::modules::ecommerce::controllers::api::cart::get_applied_coupon_code(&state.db, cart.id).await;
    let raw_cargo_fee_decimal = Decimal::from_f64_retain(cart_response.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
    
    if let Ok(eval_result) = engine.evaluate(cart.id, user_id, applied_coupon_code.as_deref(), true, &cart_response.currency, raw_cargo_fee_decimal).await {
        let summary = eval_result.summary;
        cart_response.final_total = summary.total.to_string().parse::<f64>().unwrap_or(cart_response.final_total);
        cart_response.final_total_formatted = summary.total_formatted;
        cart_response.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
        cart_response.standart_cargo_fee_formatted = Some(summary.cargo_fee_formatted);
        cart_response.is_free_shipping = summary.free_shipping;
    }

    let cart_currency = cart_response.currency.clone();
    let total_amount: f64 = cart_response.final_total;
    let standart_cargo_fee = cart_response.standart_cargo_fee.unwrap_or(0.0);
    let standart_cargo_fee_formatted = cart_response
        .standart_cargo_fee_formatted
        .unwrap_or_else(|| format_price(0.0, &cart_currency));
    let final_total_formatted = cart_response.final_total_formatted;
    let is_free_shipping = cart_response.is_free_shipping;

    context.0.insert("title", "Kapıda Ödeme");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(total_amount, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert("payment_url", &payment_url);
    context.0.insert(
        "standart_cargo_fee_formatted",
        &standart_cargo_fee_formatted,
    );
    context.0.insert("is_free_shipping", &is_free_shipping);
    context
        .0
        .insert("final_total_formatted", &final_total_formatted);
    context.0.insert(
        "product_total_formatted",
        &format_price(total_amount - standart_cargo_fee, &cart_currency),
    );

    match state.render_frontend_template("cart/payment/cash_on_delivery.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

// /// GET /payment/pickup/{payment_url} - Mağazadan teslim alma sayfası
// pub async fn pickup_payment(
//     State(state): State<AppState>,
//     Path(payment_url): Path<String>,
//     mut context: ViewContext,
// ) -> impl IntoResponse {
//     let cart = match Cart::find()
//         .filter(cart::Column::PaymentUrl.eq(&payment_url))
//         .one(&state.db)
//         .await
//     {
//         Ok(Some(cart)) => cart,
//         Ok(None) => return Redirect::to("/my-cart").into_response(),
//         Err(_) => return Redirect::to("/my-cart").into_response(),
//     };

//     if cart.status != "open_cart" {
//         println!("Cart status open değil: {}", cart.status);
//         return Redirect::to("/my-cart").into_response();
//     }

//     // Get cart items and calculate total
//     let cart_response =
//         match cart_service::get_cart(&state.db, cart.id, Some("tr".to_string())).await {
//             Ok(response) => response,
//             Err(_) => return Redirect::to("/my-cart").into_response(),
//         };

//     let total_amount: f64 = cart_response.items.iter().map(|item| item.total).sum();
//     let cart_currency = cart_response.currency.clone();

//     context.0.insert("title", "Mağazadan Teslim Alma");
//     context.0.insert("cart", &cart);
//     context.0.insert("cart_items", &cart_response.items);
//     context
//         .0
//         .insert("total_amount", &format_price(total_amount, &cart_currency));
//     context.0.insert("currency", &cart_currency);
//     context.0.insert("payment_url", &payment_url);

//     match state.render_frontend_template("cart/payment/pickup.html", &context.0) {
//         Ok(html) => Html(html).into_response(),
//         Err(_) => Redirect::to("/my-cart").into_response(),
//     }
// }

/// GET /payment/b2b-credit/{payment_url} - B2B kredili ödeme sayfası
pub async fn b2b_credit_payment(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    let user_id = match user_id {
        Some(id) => id,
        None => return Redirect::to("/login").into_response(),
    };

    // B2B kullanıcı kontrolü
    let is_b2b = crate::modules::ecommerce::services::cart_service::check_user_has_b2b_access(
        &state.db, user_id,
    )
    .await;
    if !is_b2b {
        eprintln!("User {} is not a B2B user", user_id);
        return Redirect::to("/my-cart").into_response();
    }

    // Kullanıcının şirketini bul
    use crate::modules::b2b::entities::company_users;
    let company_user = match company_users::Entity::find()
        .filter(company_users::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(cu)) => cu,
        _ => {
            eprintln!("Company not found for user {}", user_id);
            return Redirect::to("/my-cart").into_response();
        }
    };

    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq("open_cart"))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => return Redirect::to("/my-cart").into_response(),
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Sepet bilgilerini al
    let mut cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Kampanya motorunu çalıştır
    let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
    let applied_coupon_code = crate::modules::ecommerce::controllers::api::cart::get_applied_coupon_code(&state.db, cart.id).await;
    let raw_cargo_fee_decimal = Decimal::from_f64_retain(cart_response.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
    
    if let Ok(eval_result) = engine.evaluate(cart.id, user_id, applied_coupon_code.as_deref(), true, &cart_response.currency, raw_cargo_fee_decimal).await {
        let summary = eval_result.summary;
        cart_response.final_total = summary.total.to_string().parse::<f64>().unwrap_or(cart_response.final_total);
        cart_response.final_total_formatted = summary.total_formatted;
        cart_response.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
        cart_response.standart_cargo_fee_formatted = Some(summary.cargo_fee_formatted);
        cart_response.is_free_shipping = summary.free_shipping;
    }

    let cart_currency = cart_response.currency.clone();
    let total_amount: f64 = cart_response.final_total;
    let standart_cargo_fee = cart_response.standart_cargo_fee.unwrap_or(0.0);
    let standart_cargo_fee_formatted = cart_response
        .standart_cargo_fee_formatted
        .unwrap_or_else(|| format_price(0.0, &cart_currency));
    let final_total_formatted = cart_response.final_total_formatted;
    let is_free_shipping = cart_response.is_free_shipping;

    // Şirket kredi bilgilerini al
    use crate::modules::b2b::services::credit_service;
    use rust_decimal::prelude::ToPrimitive;

    let (credit_limit, used_credit, available_credit) =
        match credit_service::get_company_balance(&state.db, company_user.company_id).await {
            Ok(balance) => balance,
            Err(_) => {
                eprintln!("Failed to get company balance");
                return Redirect::to("/my-cart").into_response();
            }
        };

    // Decimal'i f64'e çevir
    let credit_limit_f64 = credit_limit.to_f64().unwrap_or(0.0);
    let used_credit_f64 = used_credit.to_f64().unwrap_or(0.0);
    let available_credit_f64 = available_credit.to_f64().unwrap_or(0.0);

    // Kredi yeterli mi kontrol et
    let total_amount_decimal =
        rust_decimal::Decimal::from_f64_retain(total_amount).unwrap_or_default();
    let has_sufficient_credit = available_credit >= total_amount_decimal;

    context.0.insert("title", "B2B Kredili Ödeme");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(total_amount, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert("payment_url", &payment_url);
    context.0.insert(
        "standart_cargo_fee_formatted",
        &standart_cargo_fee_formatted,
    );
    context.0.insert("is_free_shipping", &is_free_shipping);
    context
        .0
        .insert("final_total_formatted", &final_total_formatted);
    context.0.insert(
        "product_total_formatted",
        &format_price(total_amount - standart_cargo_fee, &cart_currency),
    );

    // Kredi bilgileri
    context.0.insert(
        "credit_limit",
        &format_price(credit_limit_f64, &cart_currency),
    );
    context.0.insert(
        "used_credit",
        &format_price(used_credit_f64, &cart_currency),
    );
    context.0.insert(
        "available_credit",
        &format_price(available_credit_f64, &cart_currency),
    );
    context
        .0
        .insert("has_sufficient_credit", &has_sufficient_credit);
    context.0.insert(
        "company_name",
        &cart_response.b2b_company_name.unwrap_or_default(),
    );

    match state.render_frontend_template("cart/payment/b2b_credit.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

/// POST /payment/b2b-credit/{payment_url} - B2B kredili ödeme işlemini tamamla
pub async fn process_b2b_credit_payment(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(_global_ctx): Extension<crate::middleware::global_context::GlobalContext>,
    Extension(user_id): Extension<Option<i64>>,
) -> impl IntoResponse {
    let user_id = match user_id {
        Some(id) => id,
        None => return Redirect::to("/login").into_response(),
    };

    // B2B kullanıcı kontrolü
    let is_b2b = crate::modules::ecommerce::services::cart_service::check_user_has_b2b_access(
        &state.db, user_id,
    )
    .await;
    if !is_b2b {
        eprintln!("User {} is not a B2B user", user_id);
        return Redirect::to("/my-cart").into_response();
    }

    // Kullanıcının şirketini bul
    use crate::modules::b2b::entities::company_users;
    let company_user = match company_users::Entity::find()
        .filter(company_users::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(cu)) => cu,
        _ => {
            eprintln!("Company not found for user {}", user_id);
            return Redirect::to("/my-cart").into_response();
        }
    };

    let cart = match Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq("open_cart"))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => return Redirect::to("/my-cart").into_response(),
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Sepet bilgilerini al
    let mut cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(_) => return Redirect::to("/my-cart").into_response(),
    };

    // Kampanya motorunu çalıştır
    let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
    let applied_coupon_code = crate::modules::ecommerce::controllers::api::cart::get_applied_coupon_code(&state.db, cart.id).await;
    let raw_cargo_fee_decimal = Decimal::from_f64_retain(cart_response.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
    
    if let Ok(eval_result) = engine.evaluate(cart.id, user_id, applied_coupon_code.as_deref(), true, &cart_response.currency, raw_cargo_fee_decimal).await {
        let summary = eval_result.summary;
        cart_response.final_total = summary.total.to_string().parse::<f64>().unwrap_or(cart_response.final_total);
        cart_response.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
        cart_response.is_free_shipping = summary.free_shipping;
    }

    let total_amount = cart_response.final_total;
    let cart_currency = cart_response.currency.clone();

    // Kredi kontrolü ve işlem oluştur
    use crate::modules::b2b::services::credit_service;
    let total_amount_decimal =
        rust_decimal::Decimal::from_f64_retain(total_amount).unwrap_or_default();

    // Kredi yeterli mi kontrol et
    match credit_service::check_credit_availability(
        &state.db,
        company_user.company_id,
        total_amount_decimal,
        &cart_currency,
    )
    .await
    {
        Ok(true) => {
            // Kredi yeterli, işlem oluştur
            let description = format!(
                "Sipariş #{}",
                cart.order_id
                    .clone()
                    .unwrap_or_else(|| format!("CART{}", cart.id))
            );

            match credit_service::create_purchase_transaction(
                &state.db,
                company_user.company_id,
                cart.id,
                total_amount_decimal,
                cart_currency.clone(),
                Some(description),
            )
            .await
            {
                Ok(_) => {
                    // Komisyon işlemi (eğer temsilci varsa)
                    let _ = crate::modules::b2b::services::commission_service::create_earned_commission(
                        &state.db,
                        company_user.company_id,
                        cart.id,
                        total_amount_decimal,
                        cart_currency.clone(),
                    ).await;

                    // Siparişi tamamla - sepet para birimini ilet (company currency)
                    match cart_service::complete_order_from_payment(
                        &state.db,
                        payment_url.clone(),
                        user_id,
                        None,
                        Some(cart_currency.clone()),
                    )
                    .await
                    {
                        Ok(_) => {
                            // Başarılı - B2B kredili ödeme başarılı sayfasına yönlendir
                            return Redirect::to(&format!(
                                "/payment/b2b-credit/success/{}",
                                payment_url
                            ))
                            .into_response();
                        }
                        Err(e) => {
                            eprintln!("Failed to complete order: {:?}", e);
                            return Redirect::to("/my-cart").into_response();
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create credit transaction: {:?}", e);
                    return Redirect::to("/my-cart").into_response();
                }
            }
        }
        Ok(false) => {
            eprintln!(
                "Insufficient credit for company {}",
                company_user.company_id
            );
            return Redirect::to(&format!("/payment/b2b-credit/{}", payment_url)).into_response();
        }
        Err(e) => {
            eprintln!("Failed to check credit availability: {:?}", e);
            return Redirect::to("/my-cart").into_response();
        }
    }
}

/// GET /payment/b2b-credit/success/{payment_url} - B2B kredili ödeme başarılı sayfası
pub async fn b2b_credit_payment_success(
    State(state): State<AppState>,
    Path(payment_url): Path<String>,
    Extension(user_id): Extension<Option<i64>>,
    mut context: ViewContext,
) -> impl IntoResponse {
    eprintln!(
        "B2B credit payment success page requested for: {}",
        payment_url
    );

    let user_id = match user_id {
        Some(id) => id,
        None => {
            return Redirect::to("/login").into_response();
        }
    };

    // B2B kullanıcı kontrolü
    let is_b2b = crate::modules::ecommerce::services::cart_service::check_user_has_b2b_access(
        &state.db, user_id,
    )
    .await;
    if !is_b2b {
        eprintln!("User {} is not a B2B user", user_id);
        return Redirect::to("/my-cart").into_response();
    }

    // Kullanıcının şirketini bul
    use crate::modules::b2b::entities::company_users;
    let company_user = match company_users::Entity::find()
        .filter(company_users::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(cu)) => cu,
        _ => {
            eprintln!("Company not found for user {}", user_id);
            return Redirect::to("/my-cart").into_response();
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

    // cart.currency'yi display currency olarak geç — item fiyatları doğru para biriminde görünsün
    let cart_response = match cart_service::get_cart(
        &state.db,
        cart.id,
        Some("tr".to_string()),
        Some(user_id),
        cart.currency.clone(),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            return Redirect::to("/my-cart").into_response();
        }
    };

    // total_amount cart'tan al
    let total_amount = cart
        .total_amount
        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0);

    let cart_currency = cart.currency.clone().unwrap_or_else(|| "TRY".to_string());
    let cargo_price = cart.cargo_price.unwrap_or(0.0);
    let cargo_currency = cart
        .cargo_currency
        .clone()
        .unwrap_or_else(|| "TRY".to_string());
    let final_total = total_amount + cargo_price;
    let product_total = total_amount;

    // Şirket kredi bilgilerini al
    use crate::modules::b2b::services::credit_service;
    use rust_decimal::prelude::ToPrimitive;

    let (credit_limit, used_credit, available_credit) =
        match credit_service::get_company_balance(&state.db, company_user.company_id).await {
            Ok(balance) => balance,
            Err(_) => {
                eprintln!("Failed to get company balance");
                return Redirect::to("/my-cart").into_response();
            }
        };

    let credit_limit_f64 = credit_limit.to_f64().unwrap_or(0.0);
    let used_credit_f64 = used_credit.to_f64().unwrap_or(0.0);
    let available_credit_f64 = available_credit.to_f64().unwrap_or(0.0);

    context
        .0
        .insert("title", "B2B Kredili Ödeme - Sipariş Başarılı");
    context.0.insert("cart", &cart);
    context.0.insert("cart_items", &cart_response.items);
    context
        .0
        .insert("total_amount", &format_price(final_total, &cart_currency));
    context.0.insert("currency", &cart_currency);
    context.0.insert(
        "cargo_price_formatted",
        &format_price(cargo_price, &cargo_currency),
    );
    context.0.insert(
        "product_total_formatted",
        &format_price(product_total, &cart_currency),
    );
    context.0.insert("is_free_shipping", &(cargo_price == 0.0));

    // Kredi bilgileri
    context.0.insert(
        "credit_limit",
        &format_price(credit_limit_f64, &cart_currency),
    );
    context.0.insert(
        "used_credit",
        &format_price(used_credit_f64, &cart_currency),
    );
    context.0.insert(
        "available_credit",
        &format_price(available_credit_f64, &cart_currency),
    );
    context.0.insert(
        "company_name",
        &cart_response.b2b_company_name.unwrap_or_default(),
    );
    context.0.insert("payment_method", "B2B Kredili Ödeme");

    match state.render_frontend_template("cart/payment/b2b_credit_success.html", &context.0) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/my-cart").into_response(),
    }
}

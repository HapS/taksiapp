use crate::app_state::AppState;
use crate::middleware::auth::{AuthenticatedUser, MaybeAuthenticatedUser};
use crate::middleware::global_context::{CurrentLanguage, GlobalContext};
use crate::modules::ecommerce::models::{cart, Cart};
use crate::modules::ecommerce::services::cart_service::{
    self, AddToCartRequest, CancelCartRequest, CancelCartResponse, CancelItemRequest,
    CancelPreviewResponse, CartItemResponse, CartResponse,
};
use crate::modules::media::services::media_service;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    Extension,
};
use axum_extra::extract::Multipart;
use rust_decimal::Decimal;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Clone)]
pub struct PaymentMethodResponse {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub b2b_available: bool,
    pub b2c_available: bool,
    pub order_id: i32,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateCartItemRequest {
    pub quantity: i32,
}

#[derive(Deserialize)]
pub struct UpdateCartAddressRequest {
    pub address_id: Option<i64>,
    pub invoice_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateCartShippingRequest {
    pub shipping_method_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateCartPaymentMethodRequest {
    pub payment_method: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateGuestInfoRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone_number: String,
    pub phone_country_code: Option<String>,
}

/// Sepet ID'sini al veya oluştur (works for both guest and authenticated users)
/// Aktif sepet ID'sini al veya oluştur (sadece open_cart durumundaki)
async fn get_or_create_active_cart_id(
    state: &AppState,
    user_id: i64,
    user_display_currency: Option<String>,
) -> Result<i64, Response> {
    // Önce aktif sepeti bul
    match cart_service::get_active_cart(
        &state.db,
        user_id,
        None,
        Some(user_id),
        user_display_currency,
    )
    .await
    {
        Ok(cart) => Ok(cart.id),
        Err(_) => {
            // Aktif sepet yoksa yeni oluştur
            match cart_service::get_or_create_cart(&state.db, Some(user_id)).await {
                Ok(cart) => Ok(cart.id),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        error: Some(format!("Sepet alınamadı: {:?}", e)),
                    }),
                )
                    .into_response()),
            }
        }
    }
}

/// POST /api/cart/items - Sepete ürün ekle
/// B2B kullanıcıları için firma para birimini kullan
pub async fn add_item(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    current_lang: CurrentLanguage,
    Json(request): Json<AddToCartRequest>,
) -> Response {
    let user_id = auth.id;

    // B2B kullanıcıları için firma para birimini kullan
    let display_currency = if let Ok(Some(company)) =
        crate::modules::b2b::services::company_service::CompanyService::get_company_by_user_id(
            &state.db, user_id,
        )
        .await
    {
        company
            .currency
            .clone()
            .unwrap_or_else(|| global_ctx.display_currency.clone())
    } else {
        global_ctx.display_currency.clone()
    };

    let cart_id =
        match get_or_create_active_cart_id(&state, user_id, Some(display_currency.clone())).await {
            Ok(id) => id,
            Err(response) => return response,
        };

    match cart_service::add_to_cart(
        &state.db,
        cart_id,
        request,
        Some(current_lang.0),
        user_id,
        Some(display_currency.clone()),
    )
    .await
    {
Ok(item) => {
            let config = crate::config::get_config();
            if let Some(cart_model) = crate::modules::ecommerce::models::Cart::find()
                .filter(crate::modules::ecommerce::models::cart::Column::UserId.eq(user_id))
                .filter(crate::modules::ecommerce::models::cart::Column::Status.eq("open_cart"))
                .one(&state.db)
                .await
                .ok()
                .flatten()
            {
                // Mevcut kuponu bul
                let applied_coupon_code = get_applied_coupon_code(&state.db, cart_model.id).await;
                
                let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
                let _ = engine.evaluate(cart_model.id, user_id, applied_coupon_code.as_deref(), config.campaign_dry_run(), &display_currency, Decimal::ZERO).await;
            }

            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(item),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => {
            let error_message = match e {
                cart_service::ServiceError::InsufficientStock => {
                    "Yetersiz stok. Mevcut stok miktarını aştınız.".to_string()
                }
                cart_service::ServiceError::InvalidQuantity => {
                    "Geçersiz miktar. Lütfen geçerli bir miktar girin.".to_string()
                }
                cart_service::ServiceError::ProductNotFound => "Ürün bulunamadı.".to_string(),
                cart_service::ServiceError::VariantNotFound => {
                    "Seçili varyant bulunamadı.".to_string()
                }
                cart_service::ServiceError::VariantKeyRequired => {
                    "Lütfen bir varyant seçin.".to_string()
                }
                cart_service::ServiceError::VariantKeyNotRequired => {
                    "Bu ürün için varyant seçimi yapılamaz.".to_string()
                }
                cart_service::ServiceError::InvalidVariantKey => {
                    "Geçersiz varyant seçimi.".to_string()
                }
                cart_service::ServiceError::InvalidContentType => {
                    "Geçersiz içerik tipi.".to_string()
                }
                cart_service::ServiceError::NotFound => "Sepet bulunamadı.".to_string(),
                cart_service::ServiceError::BadRequest(msg) => msg,
                cart_service::ServiceError::DatabaseError(_) => {
                    "Bir sistem hatası oluştu. Lütfen tekrar deneyin.".to_string()
                }
                cart_service::ServiceError::Unauthorized => {
                    "Bu işlem için yetkiniz yok.".to_string()
                }
                cart_service::ServiceError::InvalidOperation => {
                    "Bu işlem şu anda yapılamaz.".to_string()
                }
            };
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<CartItemResponse> {
                    success: false,
                    data: None,
                    error: Some(error_message),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/cart - Sepeti getir (sadece mevcut sepeti getir, yoksa boş döndür)
/// B2B kullanıcıları için firma para birimini kullan
pub async fn get_cart(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    current_lang: CurrentLanguage,
) -> Response {
    let user_id = auth.id;

    // B2B kullanıcıları için firma para birimini kullan
    let display_currency = if let Ok(Some(company)) =
        crate::modules::b2b::services::company_service::CompanyService::get_company_by_user_id(
            &state.db, user_id,
        )
        .await
    {
        company
            .currency
            .clone()
            .unwrap_or_else(|| global_ctx.display_currency.clone())
    } else {
        global_ctx.display_currency.clone()
    };

    println!("🔍 Getting cart for user_id: {:?}", user_id);

    // Sadece mevcut aktif sepeti getir, yoksa boş sepet döndür
    match cart_service::get_active_cart(
        &state.db,
        user_id,
        Some(current_lang.0),
        Some(user_id),
        Some(display_currency.clone()),
    )
    .await
    {
        Ok(mut cart) => {
            println!("🛒 Found active cart: {}", cart.id);
            
            // Kampanya motorunu çalıştır ve summary'yi ekle
            let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
            
            // Mevcut kuponu bul
            let applied_coupon_code = get_applied_coupon_code(&state.db, cart.id).await;
            
            // Kargo ücretini Decimal'e çevir
            let raw_cargo_fee_decimal = Decimal::from_f64_retain(cart.raw_cargo_fee.unwrap_or(0.0)).unwrap_or(Decimal::ZERO);
            
            match engine.evaluate(cart.id, user_id, applied_coupon_code.as_deref(), true, &display_currency, raw_cargo_fee_decimal).await {
                Ok(eval_result) => {
                    let summary = eval_result.summary;
                    
                    // Kargo bilgilerini kampanya motorundan gelen verilerle güncelle
                    cart.is_free_shipping = summary.free_shipping;
                    cart.remaining_amount_for_free_shipping = Some(summary.remaining_amount_for_free_shipping_formatted.clone());
                    cart.free_shipping_threshold = Some(summary.free_shipping_threshold.to_string().parse::<f64>().unwrap_or(0.0));
                    cart.free_shipping_threshold_formatted = Some(summary.free_shipping_threshold_formatted.clone());
                    
                    // Kargo ücreti ve formatlı halini özete göre setle
                    cart.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
                    cart.standart_cargo_fee_formatted = Some(summary.cargo_fee_formatted.clone());
                    
                    // Final toplam artık kargo dahil kampanya motorundan geliyor
                    cart.final_total = summary.total.to_string().parse::<f64>().unwrap_or(0.0);
                    cart.final_total_formatted = summary.total_formatted.clone();
                    
                    cart.campaign_summary = Some(summary);
                },
                Err(e) => {
                    println!("⚠️ Kampanya hesaplama hatası: {:?}", e);
                }
            }

            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(cart),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(_) => {
            println!("🛒 No active cart found, returning empty cart");
            // Aktif sepet yok, boş sepet response'u döndür
            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(cart_service::CartResponse {
                        id: 0,
                        items: vec![],
                        total: 0.0,
                        total_formatted: crate::modules::utils::format_price::format_price(
                            0.0,
                            &display_currency,
                        ),
                        item_count: 0,
                        address_id: None,
                        invoice_id: None,
                        address_line: None,
                        invoice_address_line: None,
                        payment_method: None,
                        order_id: None,
                        payment_url: None,
                        status: "empty".to_string(),
                        notes: None,
                        total_amount: None,
                        completed_at: None,
                        order_date: None,
                        user_info: None,
                        cargo_company: None,
                        cargo_company_title: None,
                        cargo_tracking_no: None,
                        cargo_price: None,
                        cargo_currency: None,
                        cargo_price_formatted: None,
                        currency: display_currency.clone(),
                        free_shipping_threshold: None,
                        free_shipping_threshold_formatted: None,
                        is_free_shipping: true,
                        standart_cargo_fee: Some(0.0),
                        standart_cargo_fee_formatted: Some(crate::modules::utils::format_price::format_price(0.0, &display_currency)),
                        raw_cargo_fee: None,
                        final_total: 0.0,
                        final_total_formatted: crate::modules::utils::format_price::format_price(
                            0.0,
                            &display_currency,
                        ),
                        cart_type: "b2c".to_string(),
                        b2b_company_name: None,
                        b2b_discount_percentage: None,
                        b2b_representative_name: None,
                        b2b_representative_commission: None,
                        remaining_amount_for_free_shipping: None,
                        payment_due_days: None,
                        campaign_summary: None,
                    }),
                    error: None,
                }),
            )
                .into_response()
        }
    }
}

/// PUT /api/cart/items/:id - Sepet öğesini güncelle
/// B2B kullanıcıları için firma para birimini kullan
pub async fn update_item(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    Path(item_id): Path<i64>,
    current_lang: CurrentLanguage,
    Json(request): Json<UpdateCartItemRequest>,
) -> Response {
    let user_id = auth.id;

    // B2B kullanıcıları için firma para birimini kullan
    let display_currency = if let Ok(Some(company)) =
        crate::modules::b2b::services::company_service::CompanyService::get_company_by_user_id(
            &state.db, user_id,
        )
        .await
    {
        company
            .currency
            .clone()
            .unwrap_or_else(|| global_ctx.display_currency.clone())
    } else {
        global_ctx.display_currency.clone()
    };

match cart_service::update_cart_item(
        &state.db,
        item_id,
        request.quantity,
        Some(current_lang.0),
        Some(display_currency.clone()),
    )
    .await
    {
Ok(item) => {
            let config = crate::config::get_config();
            if let Some(cart_model) = crate::modules::ecommerce::models::Cart::find()
                .filter(crate::modules::ecommerce::models::cart::Column::UserId.eq(user_id))
                .filter(crate::modules::ecommerce::models::cart::Column::Status.eq("open_cart"))
                .one(&state.db)
                .await
                .ok()
                .flatten()
            {
                // Mevcut kuponu bul
                let applied_coupon_code = get_applied_coupon_code(&state.db, cart_model.id).await;

                let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
                let _ = engine.evaluate(cart_model.id, user_id, applied_coupon_code.as_deref(), config.campaign_dry_run(), &display_currency, Decimal::ZERO).await;
            }

            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(item),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some(format!("Failed to update item: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// DELETE /api/cart/items/:id - Sepet öğesini sil
pub async fn remove_item(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    Path(item_id): Path<i64>,
) -> Response {
    let user_id = auth.id;

    match cart_service::remove_cart_item(&state.db, item_id).await {
        Ok(_) => {
            let config = crate::config::get_config();
            if let Some(cart_model) = crate::modules::ecommerce::models::cart::Entity::find()
                .filter(crate::modules::ecommerce::models::cart::Column::UserId.eq(user_id))
                .filter(crate::modules::ecommerce::models::cart::Column::Status.eq("open_cart"))
                .one(&state.db)
                .await
                .ok()
                .flatten()
            {
                // Mevcut kuponu bul
                let applied_coupon_code = get_applied_coupon_code(&state.db, cart_model.id).await;

                let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(state.db.clone());
                let _ = engine.evaluate(cart_model.id, user_id, applied_coupon_code.as_deref(), config.campaign_dry_run(), &global_ctx.display_currency, Decimal::ZERO).await;
            }

            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(()),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Failed to remove item: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// DELETE /api/cart - Sepeti temizle
pub async fn clear_cart(State(state): State<AppState>, auth: AuthenticatedUser) -> Response {
    let cart_id = match get_or_create_active_cart_id(&state, auth.id, None).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    match cart_service::clear_cart(&state.db, cart_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(()),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(format!("Failed to clear cart: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /api/cart/address - Sepet adres bilgilerini güncelle
pub async fn update_cart_address(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(request): Json<UpdateCartAddressRequest>,
) -> Response {
    let cart_id = match get_or_create_active_cart_id(&state, auth.id, None).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    match cart_service::update_cart_addresses(
        &state.db,
        cart_id,
        request.address_id,
        request.invoice_id,
    )
    .await
    {
        Ok(cart) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(cart),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(
                ApiResponse::<crate::modules::ecommerce::models::CartModel> {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to update cart address: {:?}", e)),
                },
            ),
        )
            .into_response(),
    }
}

pub async fn update_cart_shipping_method(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Json(request): Json<UpdateCartShippingRequest>,
) -> Response {
    let cart_id = match get_or_create_active_cart_id(&state, auth.id, None).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    // println!(
    //     "API CART UPDATE TESLIMAT : {}",
    //     &request
    //         .shipping_method_id
    //         .clone()
    //         .expect("REASON")
    //         .to_string()
    // );

    match cart_service::update_cart_shipping_method(&state.db, cart_id, request.shipping_method_id)
        .await
    {
        Ok(cart) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(cart),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(
                ApiResponse::<crate::modules::ecommerce::models::CartModel> {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to update cart address: {:?}", e)),
                },
            ),
        )
            .into_response(),
    }
}
use sea_orm::EntityTrait;
use sea_orm::QueryOrder;
/// GET /api/cart/payment-methods - Mevcut ödeme yöntemlerini getir (settings'den)
pub async fn get_payment_methods(
    State(state): State<AppState>,
    Extension(current_language): Extension<crate::middleware::global_context::CurrentLanguage>,
    Extension(_user_id): Extension<Option<i64>>,
) -> Response {
    let lang = current_language.0;

    let cart = crate::modules::ecommerce::models::cart::Entity::find()
        .filter(cart::Column::UserId.eq(_user_id))
        .filter(cart::Column::Status.eq("open_cart"))
        .order_by_desc(cart::Column::Id)
        .one(&state.db)
        .await
        .unwrap_or(None);

    let cart_type = cart
        .as_ref()
        .map(|c| c.cart_type.clone())
        .unwrap_or_else(|| "b2c".to_string());

    match crate::modules::admin::services::settings_service::get_settings(&state.db).await {
        Ok(settings) => {
            let payment_methods = settings.payment_methods.unwrap_or_default();

            let mut result: Vec<PaymentMethodResponse> = Vec::new();

            if let Some(obj) = payment_methods.as_object() {
                for (key, value) in obj {
                    let b2b_available = value
                        .get("b2b_available")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);

                    let b2c_available = value
                        .get("b2c_available")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);

                    // b2b b2c filtrelemesi
                    if cart_type == "b2b" && !b2b_available {
                        continue;
                    }
                    if cart_type == "b2c" && !b2c_available {
                        continue;
                    }

                    let title = value
                        .get("langs")
                        .and_then(|langs| langs.get(&lang))
                        .and_then(|l| l.get("title"))
                        .and_then(|t| t.as_str())
                        .unwrap_or(key)
                        .to_string();

                    let description = value
                        .get("langs")
                        .and_then(|langs| langs.get(&lang))
                        .and_then(|l| l.get("description"))
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());

                    let icon = value
                        .get("icon")
                        .and_then(|i| i.as_str())
                        .map(|s| s.to_string());

                    let order_id = value
                        .get("order_id")
                        .and_then(|o| o.as_i64())
                        .map(|o| o as i32)
                        .unwrap_or(999);

                    result.push(PaymentMethodResponse {
                        key: key.clone(),
                        title,
                        description,
                        icon,
                        b2b_available,
                        b2c_available,
                        order_id,
                    });
                }
            }

            // Order by order_id
            result.sort_by_key(|r| r.order_id);

            (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(result),
                    error: None,
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<PaymentMethodResponse>> {
                success: false,
                data: None,
                error: Some(format!("Failed to get payment methods: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /api/cart/payment-method - Sepet ödeme yöntemini güncelle
pub async fn update_cart_payment_method(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Extension(_current_language): Extension<crate::middleware::global_context::CurrentLanguage>,
    Json(request): Json<UpdateCartPaymentMethodRequest>,
) -> Response {
    let cart_id = match get_or_create_active_cart_id(&state, auth.id, None).await {
        Ok(id) => id,
        Err(response) => return response,
    };

    // Settings'den ödeme yöntemlerini çek
    let settings =
        match crate::modules::admin::services::settings_service::get_settings(&state.db).await {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        ApiResponse::<crate::modules::ecommerce::models::CartModel> {
                            success: false,
                            data: None,
                            error: Some(format!("Ayarlar yüklenemedi: {:?}", e)),
                        },
                    ),
                )
                    .into_response();
            }
        };

    let payment_methods = settings.payment_methods.unwrap_or_default();
    let valid_keys: Vec<String> = if let Some(obj) = payment_methods.as_object() {
        obj.keys().cloned().collect()
    } else {
        vec![]
    };

    // Settings'den gelen payment method key'ini validate et
    let payment_method = match &request.payment_method {
        Some(pm) => {
            if valid_keys.contains(pm) {
                Some(pm.clone())
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(
                        ApiResponse::<crate::modules::ecommerce::models::CartModel> {
                            success: false,
                            data: None,
                            error: Some(format!(
                                "Geçersiz ödeme yöntemi: '{}'. Geçerli değerler: {}",
                                pm,
                                valid_keys.join(", ")
                            )),
                        },
                    ),
                )
                    .into_response();
            }
        }
        None => None,
    };

    match cart_service::update_cart_payment_method(&state.db, cart_id, payment_method).await {
        Ok(cart) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(cart),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(
                ApiResponse::<crate::modules::ecommerce::models::CartModel> {
                    success: false,
                    data: None,
                    error: Some(format!("Failed to update payment method: {:?}", e)),
                },
            ),
        )
            .into_response(),
    }
}
/// POST /api/cart/payment-start - Ödeme işlemini başlat
/// B2B kullanıcıları için firma para birimini kullan
#[derive(Debug, Deserialize)]
pub struct PaymentStartRequest {
    pub notes: Option<String>,
    pub credit_ids: Option<Vec<i64>>,
}

pub async fn start_payment(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    Json(request): Json<PaymentStartRequest>,
) -> Response {
    let user_id = auth.id;

    // B2B kullanıcıları için firma para birimini kullan
    let display_currency = if let Ok(Some(company)) =
        crate::modules::b2b::services::company_service::CompanyService::get_company_by_user_id(
            &state.db, user_id,
        )
        .await
    {
        company
            .currency
            .clone()
            .unwrap_or_else(|| global_ctx.display_currency.clone())
    } else {
        global_ctx.display_currency.clone()
    };

    let cart_id =
        match get_or_create_active_cart_id(&state, auth.id, Some(display_currency.clone())).await {
            Ok(id) => id,
            Err(response) => return response,
        };

    match cart_service::start_payment(
        &state.db,
        cart_id,
        request.notes,
        request.credit_ids,
        Some(display_currency),
    )
    .await
    {
        Ok(payment_response) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(payment_response),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => {
            let (status, message) = match e {
                cart_service::ServiceError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Ödeme başlatılamadı: {}", e),
                ),
            };

            (
                status,
                Json(ApiResponse::<cart_service::PaymentStartResponse> {
                    success: false,
                    data: None,
                    error: Some(message),
                }),
            )
                .into_response()
        }
    }
}
/// POST /api/cart/complete-order - Ödeme sayfasından sipariş oluştur
/// B2B kullanıcıları için firma para birimini kullan
pub async fn complete_order_not_credit_card_payment(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let user_id = auth.id;

    // B2B kullanıcıları için firma para birimini kullan
    let display_currency = if let Ok(Some(company)) =
        crate::modules::b2b::services::company_service::CompanyService::get_company_by_user_id(
            &state.db, user_id,
        )
        .await
    {
        company
            .currency
            .clone()
            .unwrap_or_else(|| global_ctx.display_currency.clone())
    } else {
        global_ctx.display_currency.clone()
    };

    let payment_url = match request.get("payment_url").and_then(|v| v.as_str()) {
        Some(url) => url,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<CartResponse> {
                    success: false,
                    data: None,
                    error: Some("Payment URL gerekli".to_string()),
                }),
            )
                .into_response();
        }
    };

    let notes = request
        .get("notes")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    match cart_service::complete_order_from_payment(
        &state.db,
        payment_url.to_string(),
        user_id,
        notes,
        Some(display_currency),
    )
    .await
    {
        Ok(order) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(order),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => {
            let (status, message) = match e {
                cart_service::ServiceError::NotFound => {
                    (StatusCode::NOT_FOUND, "Geçersiz ödeme URL'si".to_string())
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Sipariş oluşturulamadı: {:?}", e),
                ),
            };

            (
                status,
                Json(ApiResponse::<CartResponse> {
                    success: false,
                    data: None,
                    error: Some(message),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/cart/orders/{id} - Tek sipariş detayını getir
pub async fn get_user_order(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user_id = auth.id;

    let display_currency = if let Ok(Some(company)) =
        crate::modules::b2b::services::company_service::CompanyService::get_company_by_user_id(
            &state.db, user_id,
        )
        .await
    {
        company
            .currency
            .clone()
            .unwrap_or_else(|| global_ctx.display_currency.clone())
    } else {
        global_ctx.display_currency.clone()
    };

    match cart_service::get_user_order_by_order_id(&state.db, user_id, &id, Some(display_currency)).await {
        Ok(Some(order)) => (
            StatusCode::OK,
            Json(json!({ "success": true, "data": order })),
        ).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "Sipariş bulunamadı" })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        ).into_response(),
    }
}

/// GET /api/cart/orders - Kullanıcının siparişlerini getir
/// B2B kullanıcıları için firma para birimini kullan
pub async fn get_user_orders(
    State(state): State<AppState>,
    Extension(global_ctx): Extension<GlobalContext>,
    auth: AuthenticatedUser,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let user_id = auth.id;

    // B2B kullanıcıları için firma para birimini kullan
    let display_currency = if let Ok(Some(company)) =
        crate::modules::b2b::services::company_service::CompanyService::get_company_by_user_id(
            &state.db, user_id,
        )
        .await
    {
        company
            .currency
            .clone()
            .unwrap_or_else(|| global_ctx.display_currency.clone())
    } else {
        global_ctx.display_currency.clone()
    };

    let page = params.get("page").and_then(|p| p.parse::<u64>().ok());
    let per_page = params.get("per_page").and_then(|p| p.parse::<u64>().ok());
    let status = params.get("status").cloned();
    let date_from = params.get("date_from").cloned();
    let date_to = params.get("date_to").cloned();
    let payment_method = params.get("payment_method").cloned();

    match cart_service::get_user_orders(
        &state.db,
        user_id,
        page,
        per_page,
        status,
        date_from,
        date_to,
        Some(display_currency),
        payment_method,
    )
    .await
    {
        Ok(orders) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(orders),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<CartResponse>> {
                success: false,
                data: None,
                error: Some(format!("Siparişler getirilemedi: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// PUT /api/cart/orders/{id}/status - Sipariş durumunu güncelle (Admin)
pub async fn update_order_status(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    axum::extract::Path(cart_id): axum::extract::Path<i64>,
    Json(request): Json<serde_json::Value>,
) -> Response {
    let admin_user_id = auth.id;

    let new_status = match request.get("status").and_then(|v| v.as_str()) {
        Some(status) => status,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<CartResponse> {
                    success: false,
                    data: None,
                    error: Some("Status gerekli".to_string()),
                }),
            )
                .into_response();
        }
    };

    match cart_service::update_order_status(
        &state.db,
        cart_id,
        new_status.to_string(),
        Some(admin_user_id),
    )
    .await
    {
        Ok(order) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(order),
                error: None,
            }),
        )
            .into_response(),
        Err(e) => {
            let (status, message) = match e {
                cart_service::ServiceError::NotFound => {
                    (StatusCode::NOT_FOUND, "Sipariş bulunamadı".to_string())
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Sipariş güncellenemedi: {:?}", e),
                ),
            };

            (
                status,
                Json(ApiResponse::<CartResponse> {
                    success: false,
                    data: None,
                    error: Some(message),
                }),
            )
                .into_response()
        }
    }
}
/// POST /api/cart/orders/{id}/upload-document - Sipariş için dekont yükle
pub async fn upload_payment_document(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
    axum::extract::Path(cart_id): axum::extract::Path<i64>,
    mut multipart: Multipart,
) -> Response {
    let user_id = match user_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Giriş yapmalısınız".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Cart'ın kullanıcıya ait olup olmadığını kontrol et
    let cart = match Cart::find_by_id(cart_id)
        .filter(cart::Column::UserId.eq(user_id))
        .one(&state.db)
        .await
    {
        Ok(Some(cart)) => cart,
        Ok(None) => {
            return (
                StatusCode::FORBIDDEN,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Bu siparişe erişim yetkiniz yok".to_string()),
                }),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Sipariş kontrolü yapılamadı".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Sadece banka transferi siparişleri için dekont yükleme
    if cart.payment_method.as_deref() != Some("bank_transfer") {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Bu ödeme yöntemi için dekont yükleme gerekli değil".to_string()),
            }),
        )
            .into_response();
    }

    // Dosyayı işle
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();

        if name == "document" {
            let filename = field.file_name().unwrap_or("document").to_string();
            let content_type = field.content_type().unwrap_or("").to_string();
            let data = match field.bytes().await {
                Ok(bytes) => bytes,
                Err(_) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            error: Some("Dosya okunamadı".to_string()),
                        }),
                    )
                        .into_response();
                }
            };

            // Dosya boyutu kontrolü (5MB)
            if data.len() > 5 * 1024 * 1024 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        error: Some("Dosya boyutu 5MB'dan büyük olamaz".to_string()),
                    }),
                )
                    .into_response();
            }

            // Dosya tipi kontrolü
            let allowed_types = ["application/pdf", "image/png", "image/jpeg", "image/jpg"];
            if !allowed_types.contains(&content_type.as_str()) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        error: Some("Sadece PDF, PNG, JPG dosyaları yükleyebilirsiniz".to_string()),
                    }),
                )
                    .into_response();
            }

            // Dosya adını sipariş numarası ile prefix'le
            let cart_id_str = cart_id.to_string();
            let order_prefix = cart.order_id.as_ref().unwrap_or(&cart_id_str);
            let safe_filename = format!("dekont_{}_{}", order_prefix, filename);

            // Upload path oluştur (payment_documents klasörü)
            let upload_path = media_service::generate_upload_path(
                "media/uploads/payment_documents",
                &safe_filename,
            );

            // Klasörü oluştur
            if let Some(parent) = upload_path.parent() {
                if let Err(_) = tokio::fs::create_dir_all(parent).await {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse::<()> {
                            success: false,
                            data: None,
                            error: Some("Dosya klasörü oluşturulamadı".to_string()),
                        }),
                    )
                        .into_response();
                }
            }

            // Dosyayı kaydet
            if let Err(_) = tokio::fs::write(&upload_path, &data).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()> {
                        success: false,
                        data: None,
                        error: Some("Dosya kaydedilemedi".to_string()),
                    }),
                )
                    .into_response();
            }

            let file_path =
                crate::modules::media::services::media_service::normalize_file_path(&upload_path);

            // Timeline event oluştur
            let mut title_map = std::collections::HashMap::new();
            title_map.insert(
                "tr".to_string(),
                format!(
                    "Ödeme dekontu yüklendi: {}",
                    cart.order_id.as_ref().unwrap_or(&"N/A".to_string())
                ),
            );
            title_map.insert(
                "en".to_string(),
                format!(
                    "Payment document uploaded: {}",
                    cart.order_id.as_ref().unwrap_or(&"N/A".to_string())
                ),
            );

            let mut desc_map = std::collections::HashMap::new();
            desc_map.insert(
                "tr".to_string(),
                format!("Dosya: {} ({})", filename, file_path),
            );
            desc_map.insert(
                "en".to_string(),
                format!("File: {} ({})", filename, file_path),
            );

            let _ = crate::modules::timeline::services::timeline_service::TimelineService::create_event(
                &state.db,
                crate::modules::timeline::services::timeline_service::CreateTimelineEventRequest {
                    module_type: "ecommerce".to_string(),
                    content_type: "cart".to_string(),
                    content_id: cart.id,
                    event_type: crate::modules::timeline::models::timeline_event::TimelineEventType::Custom("document_uploaded".to_string()),
                    title: title_map,
                    description: Some(desc_map),
                    icon: Some("bi-file-earmark-arrow-up".to_string()),
                    color: Some("#17a2b8".to_string()),
                    user_id: Some(user_id),
                    admin_user_id: None,
                    metadata: Some(serde_json::json!({
                        "filename": filename,
                        "content_type": content_type,
                        "file_size": data.len(),
                        "file_path": file_path
                    })),
                    is_public: Some(false),
                    is_admin_only: Some(false),
                },
            ).await;

            return (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(serde_json::json!({
                        "message": "Dekont başarıyla yüklendi",
                        "filename": filename,
                        "file_path": file_path,
                        "file_size": data.len()
                    })),
                    error: None,
                }),
            )
                .into_response();
        }
    }

    // Dosya bulunamadı
    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse::<()> {
            success: false,
            data: None,
            error: Some("Dosya bulunamadı".to_string()),
        }),
    )
        .into_response()
}

/// PUT /api/cart/guest-info - Guest kullanıcı bilgilerini güncelle
pub async fn update_guest_info(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
    Json(request): Json<UpdateGuestInfoRequest>,
) -> Response {
    let user_id = match user_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Session error - please refresh the page".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Kullanıcıyı bul
    let user = match crate::modules::auth::models::user::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Kullanıcı bulunamadı".to_string()),
                }),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Kullanıcı sorgulanamadı".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Sadece guest kullanıcılar için izin ver
    if !user.is_guest {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Bu işlem sadece misafir kullanıcılar için geçerli".to_string()),
            }),
        )
            .into_response();
    }

    // Email formatı kontrolü
    if !request.email.contains('@') || request.email.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Geçerli bir email adresi giriniz".to_string()),
            }),
        )
            .into_response();
    }

    // İsim soyisim kontrolü
    if request.first_name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Ad alanı boş olamaz".to_string()),
            }),
        )
            .into_response();
    }

    if request.last_name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Soyad alanı boş olamaz".to_string()),
            }),
        )
            .into_response();
    }

    // Telefon numarası kontrolü
    if request.phone_number.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Telefon numarası boş olamaz".to_string()),
            }),
        )
            .into_response();
    }

    // Kullanıcı bilgilerini güncelle
    let mut user_active: crate::modules::auth::models::user::ActiveModel = user.into();
    user_active.first_name = Set(Some(request.first_name.trim().to_string()));
    user_active.last_name = Set(Some(request.last_name.trim().to_string()));
    user_active.email = Set(request.email.trim().to_string());
    user_active.phone_number = Set(Some(request.phone_number.trim().to_string()));
    user_active.phone_country_code = Set(request
        .phone_country_code
        .map(|s| s.trim().to_string())
        .or(Some("+90".to_string())));
    user_active.updated_at = Set(Some(chrono::Utc::now().into()));

    match user_active.update(&state.db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some("Bilgiler başarıyla güncellendi"),
                error: None,
            }),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Bilgiler güncellenirken bir hata oluştu".to_string()),
            }),
        )
            .into_response(),
    }
}

/// GET /api/cart/guest-info - Guest kullanıcı bilgilerini getir
pub async fn get_guest_info(
    State(state): State<AppState>,
    Extension(user_id): Extension<Option<i64>>,
) -> Response {
    let user_id = match user_id {
        Some(id) => id,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Session error - please refresh the page".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Kullanıcıyı bul
    let user = match crate::modules::auth::models::user::Entity::find_by_id(user_id)
        .one(&state.db)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Kullanıcı bulunamadı".to_string()),
                }),
            )
                .into_response();
        }
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Kullanıcı sorgulanamadı".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Sadece guest kullanıcılar için izin ver
    if !user.is_guest {
        return (
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some("Bu işlem sadece misafir kullanıcılar için geçerli".to_string()),
            }),
        )
            .into_response();
    }

    #[derive(Serialize)]
    struct GuestInfo {
        first_name: String,
        last_name: String,
        email: String,
        phone_number: String,
        phone_country_code: String,
        is_complete: bool,
    }

    let guest_info = GuestInfo {
        first_name: user.first_name.clone().unwrap_or_default(),
        last_name: user.last_name.clone().unwrap_or_default(),
        email: if user.email.contains("@guest.local") {
            String::new()
        } else {
            user.email.clone()
        },
        phone_number: user.phone_number.clone().unwrap_or_default(),
        phone_country_code: user
            .phone_country_code
            .clone()
            .unwrap_or_else(|| "+90".to_string()),
        is_complete: user.first_name.is_some()
            && user.last_name.is_some()
            && !user.email.contains("@guest.local")
            && user.phone_number.is_some()
            && !user.first_name.as_ref().unwrap().trim().is_empty()
            && !user.last_name.as_ref().unwrap().trim().is_empty()
            && !user.phone_number.as_ref().unwrap().trim().is_empty(),
    };

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: Some(guest_info),
            error: None,
        }),
    )
        .into_response()
}

/// POST /api/orders/:cart_id/items/:item_id/cancel/preview - İptal önizleme (kargo ücreti etkisi)
pub async fn preview_cancel_item(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((cart_id, item_id)): Path<(i64, i64)>,
    Json(request): Json<CancelItemRequest>,
) -> Response {
    let user_id = auth.id;

    match cart_service::preview_cancel_item(&state.db, user_id, cart_id, item_id, request.quantity)
        .await
    {
        Ok(preview) => (
            StatusCode::OK,
            Json(ApiResponse::<CancelPreviewResponse> {
                success: true,
                data: Some(preview),
                error: None,
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<CancelPreviewResponse> {
                success: false,
                data: None,
                error: Some("Sipariş veya ürün bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<CancelPreviewResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::BadRequest(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CancelPreviewResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::InvalidOperation) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CancelPreviewResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem şu anda yapılamaz".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<CancelPreviewResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// POST /api/orders/:cart_id/items/:item_id/cancel - Sipariş içindeki bir ürün için iptal talebi oluştur
pub async fn request_cancel_item(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((cart_id, item_id)): Path<(i64, i64)>,
    Json(request): Json<CancelItemRequest>,
) -> Response {
    let user_id = auth.id;

    match cart_service::request_cancel_item(&state.db, user_id, cart_id, item_id, request.quantity)
        .await
    {
        Ok(item) => (
            StatusCode::OK,
            Json(ApiResponse::<CartItemResponse> {
                success: true,
                data: Some(item),
                error: None,
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some("Sipariş veya ürün bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::BadRequest(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some(msg),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::InvalidOperation) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem şu anda yapılamaz".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// DELETE /api/orders/:cart_id/items/:item_id/cancel - İptal talebini geri çek
pub async fn cancel_cancel_request(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path((cart_id, item_id)): Path<(i64, i64)>,
) -> Response {
    let user_id = auth.id;

    match cart_service::cancel_cancel_request(&state.db, user_id, cart_id, item_id).await {
        Ok(item) => (
            StatusCode::OK,
            Json(ApiResponse::<CartItemResponse> {
                success: true,
                data: Some(item),
                error: None,
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some("Sipariş veya ürün bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::InvalidOperation) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem şu anda yapılamaz".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<CartItemResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// POST /api/order/cancel-request - Sipariş iptal talebi oluştur (Tüm sipariş için)

pub async fn request_cancel_cart(
    State(state): State<AppState>,
    auth: MaybeAuthenticatedUser,
    // Extension(user_id): Extension<Option<i64>>,
    Json(request): Json<CancelCartRequest>,
) -> Response {
    let user_id = match auth.id {
        Some(id) => id,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some("Sadece Oturum Açmış Kullanıcılar İçin".to_string()),
                }),
            )
                .into_response();
        }
    };

    println!("USER ID {:?}", user_id);

    match cart_service::request_cancel_cart(&state.db, user_id, request.cart_id).await {
        Ok(message) => (
            StatusCode::OK,
            Json(ApiResponse::<CancelCartResponse> {
                success: true,
                data: Some(message),
                error: None,
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<CancelCartResponse> {
                success: false,
                data: None,
                error: Some("Sipariş veya ürün bulunamadı".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::Unauthorized) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<CancelCartResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem için yetkiniz yok".to_string()),
            }),
        )
            .into_response(),
        Err(cart_service::ServiceError::InvalidOperation) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<CancelCartResponse> {
                success: false,
                data: None,
                error: Some("Bu işlem şu anda yapılamaz".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<CancelCartResponse> {
                success: false,
                data: None,
                error: Some(format!("Sunucu hatası: {:?}", e)),
            }),
        )
            .into_response(),
    }
}

/// Sepete uygulanmış mevcut kupon kodunu getirir
pub(crate) async fn get_applied_coupon_code(db: &sea_orm::DatabaseConnection, cart_id: i64) -> Option<String> {
    use crate::modules::ecommerce::models::{cart_discount, coupon};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let discount = cart_discount::Entity::find()
        .filter(cart_discount::Column::CartId.eq(cart_id))
        .filter(cart_discount::Column::ScenarioType.eq("coupon_code"))
        .filter(cart_discount::Column::CouponId.is_not_null())
        .one(db)
        .await
        .ok()
        .flatten()?;

    if let Some(cid) = discount.coupon_id {
        coupon::Entity::find_by_id(cid)
            .one(db)
            .await
            .ok()
            .flatten()
            .map(|c| c.code)
    } else {
        None
    }
}

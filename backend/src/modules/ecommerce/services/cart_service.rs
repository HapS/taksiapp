use crate::config::get_config;
use crate::modules::admin::services::settings_service::get_sale_currency;
use crate::modules::content::models::{content, Content};
use crate::modules::currency::services::exchange_rate_service::{
    convert_currency, get_cached_rates,
};

use crate::modules::ecommerce::models::KargoSirketleriEntity;
use crate::modules::ecommerce::models::{cart, cart_item, Cart, CartItem, CartModel};
use crate::modules::mailer::services::mailer_template_service::MailHelper;
use crate::modules::utils::format_price::format_price;
use chrono::{Local, Utc};
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::EntityTrait;
use sea_orm::QueryOrder;
use sea_orm::*;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ServiceError {
    NotFound,
    InvalidQuantity,
    ProductNotFound,
    VariantNotFound,
    BadRequest(String),
    DatabaseError(DbErr),
    InvalidVariantKey,
    VariantKeyRequired,
    VariantKeyNotRequired,
    InvalidContentType,
    InsufficientStock,
    Unauthorized,
    InvalidOperation,
}

impl From<DbErr> for ServiceError {
    fn from(err: DbErr) -> Self {
        ServiceError::DatabaseError(err)
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::NotFound => write!(f, "Sepet bulunamadı"),
            ServiceError::InvalidQuantity => write!(f, "Geçersiz miktar"),
            ServiceError::ProductNotFound => write!(f, "Ürün bulunamadı"),
            ServiceError::VariantNotFound => write!(f, "Varyant bulunamadı"),
            ServiceError::BadRequest(msg) => write!(f, "{}", msg),
            ServiceError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
            ServiceError::InvalidVariantKey => write!(f, "Geçersiz varyant anahtarı"),
            ServiceError::VariantKeyRequired => write!(f, "Varyant seçimi zorunlu"),
            ServiceError::VariantKeyNotRequired => write!(f, "Varyant seçimi gerekli değil"),
            ServiceError::InvalidContentType => write!(f, "Geçersiz içerik tipi"),
            ServiceError::InsufficientStock => write!(f, "Yetersiz stok"),
            ServiceError::Unauthorized => write!(f, "Yetkisiz erişim"),
            ServiceError::InvalidOperation => write!(f, "Geçersiz işlem"),
        }
    }
}

/// Sepete eklenecek ürün bilgisi
#[derive(Debug, Deserialize)]
pub struct AddToCartRequest {
    pub product_id: i64,
    pub variant_key: Option<String>, // option_values_display
    pub quantity: i32,
}

/// Sepet öğesi response
/// bu struck admin panelde de kullanılıyor dokunurken dokuz kez düşün
#[derive(Debug, Serialize, Clone)]
pub struct CartItemResponse {
    pub id: i64,
    pub product_id: i64,
    pub product_title: String,
    pub product_cover: String,
    pub variant_key: Option<String>,
    pub variant_display: Option<String>,
    pub quantity: i32,
    pub price: f64,                        // Gösterilecek fiyat (sale_currency'de)
    pub price_formatted: String,           // Formatlanmış fiyat
    pub total: f64,                        // Gösterilecek toplam (sale_currency'de)
    pub total_formatted: String,           // Formatlanmış toplam
    pub total_price: f64,                  // template uyumluluğu için
    pub item_count: i32,                   // sepete ürün eklendiğinde count bilgisi de dönsün
    pub currency: String,                  // Gösterilecek para birimi (sale_currency)
    pub original_price: Option<f64>,       // Ürünün orijinal fiyatı (kendi para biriminde)
    pub original_currency: Option<String>, // Ürünün orijinal para birimi
    pub original_price_formatted: Option<String>, // Formatlanmış orijinal fiyat
    pub status: Option<String>, // null = normal, cancel_request = iptal talebi, cancel_accept = iptal onaylandı
    pub refund_status: Option<String>, // credited_b2b, credited_b2c, bank_refunded
    pub refund_amount: Option<f64>,
    pub refund_amount_formatted: Option<String>, // Formatlanmış iade tutarı (para birimi sembolü dahil)
    pub refund_currency: Option<String>,         // İade para birimi (TRY, AZN, USD, EUR vb.)
    pub refund_date: Option<DateTimeWithTimeZone>,
    pub discount_percentage: Option<f64>, // Ürün indirim oranı (B2C için)
}

/// Sepet response
#[derive(Debug, Serialize)]
pub struct CartResponse {
    pub id: i64,
    pub items: Vec<CartItemResponse>,
    pub total: f64,
    pub total_formatted: String, // Formatlanmış toplam
    pub item_count: i32,
    pub address_id: Option<i64>,
    pub invoice_id: Option<i64>,
    pub address_line: Option<String>,
    pub invoice_address_line: Option<String>,
    pub payment_method: Option<String>,
    pub order_id: Option<String>,
    pub payment_url: Option<String>,
    pub status: String,
    pub notes: Option<String>,
    pub total_amount: Option<rust_decimal::Decimal>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub order_date: Option<DateTimeWithTimeZone>,
    pub user_info: Option<UserInfo>,
    pub cargo_company: Option<i64>,
    pub cargo_company_title: Option<String>,
    pub cargo_tracking_no: Option<String>,
    pub cargo_price: Option<f64>,
    pub cargo_currency: Option<String>,
    pub cargo_price_formatted: Option<String>,
    pub currency: String, // Sepet/sipariş para birimi (sale_currency)
    pub free_shipping_threshold: Option<f64>,
    pub free_shipping_threshold_formatted: Option<String>,
    pub remaining_amount_for_free_shipping: Option<String>,
    pub is_free_shipping: bool,
    pub standart_cargo_fee: Option<f64>,
    pub standart_cargo_fee_formatted: Option<String>,
    pub raw_cargo_fee: Option<f64>,
    pub cart_type: String, // 'b2b' veya 'b2c'
    pub final_total: f64,  // İndirimler ve kargo ücretleri uygulandıktan sonraki final toplam
    pub final_total_formatted: String,
    // B2B özel alanlar (sadece B2B siparişlerde dolu)
    pub b2b_company_name: Option<String>,
    pub b2b_discount_percentage: Option<f64>,
    pub b2b_representative_name: Option<String>,
    pub b2b_representative_commission: Option<f64>,
    pub payment_due_days: Option<i32>,
    pub campaign_summary: Option<crate::modules::ecommerce::campaign::engine::CartSummary>,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: String,
    pub email: String,
    pub profile: Option<serde_json::Value>,
}

/// Kullanıcı için sepet bul veya oluştur (misafir ve giriş yapmış kullanıcılar için çalışır)
pub async fn get_or_create_cart(
    db: &DatabaseConnection,
    user_id: Option<i64>,
) -> Result<CartModel, ServiceError> {
    let uid = user_id.ok_or(ServiceError::NotFound)?;

    // Önce mevcut aktif sepeti bul (sadece open_cart statusundaki)
    // Payment URL'si olan cart'lar da dahil (henüz tamamlanmamış ödemeler)
    let existing_cart = Cart::find()
        .filter(cart::Column::UserId.eq(uid))
        .filter(cart::Column::Status.eq(crate::modules::ecommerce::models::cart::status::OPEN_CART))
        .one(db)
        .await?;

    if let Some(cart) = existing_cart {
        return Ok(cart);
    }

    // Yoksa yeni sepet oluştur
    // Cart type'ı belirle (B2B mi B2C mi)
    let is_b2b = check_user_has_b2b_access(db, uid).await;
    let cart_type = if is_b2b { "b2b" } else { "b2c" };

    // B2B kullanıcılar için cart currency = şirketin referans para birimi
    let cart_currency: Option<String> = if is_b2b {
        use crate::modules::b2b::entities::{companies, company_users};
        let company_user = company_users::Entity::find()
            .filter(company_users::Column::UserId.eq(uid))
            .one(db)
            .await
            .ok()
            .flatten();
        if let Some(cu) = company_user {
            companies::Entity::find_by_id(cu.company_id)
                .one(db)
                .await
                .ok()
                .flatten()
                .and_then(|c| c.currency)
        } else {
            None
        }
    } else {
        None
    };

    let new_cart = cart::ActiveModel {
        user_id: Set(uid),
        status: Set(crate::modules::ecommerce::models::cart::status::OPEN_CART.to_string()),
        cart_type: Set(cart_type.to_string()),
        currency: Set(cart_currency),
        created_at: Set(Some(Utc::now().into())),
        updated_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let cart = new_cart.insert(db).await?;
    Ok(cart)
}

// Kullanıcı için mevcut açık sepeti getir (yeni oluşturmaz)
pub async fn find_active_cart_by_user(
    db: &DatabaseConnection,
    user_id: i64,
) -> Result<Option<CartModel>, ServiceError> {
    let existing_cart = Cart::find()
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq(crate::modules::ecommerce::models::cart::status::OPEN_CART))
        .one(db)
        .await?;

    Ok(existing_cart)
}

/// Ürün fiyatını al (varyant varsa varyant fiyatı, yoksa base fiyat)
/// Döndürür: (fiyat, başlık, varyant_display, cover_image, para_birimi)
/// NOT: Bu fonksiyon B2C fiyatlarını döndürür. B2B için get_product_price_b2b kullanın.
/// B2C kullanıcı için ürün fiyatını ve indirim oranını al
/// Döndürür: (fiyat, başlık, varyant_display, cover_image, para_birimi, discount_percentage)
async fn get_product_price_with_discount(
    db: &DatabaseConnection,
    product_id: i64,
    variant_key: Option<&str>,
    lang: Option<&str>,
) -> Result<(f64, String, Option<String>, Option<String>, String, Option<f64>), ServiceError> {
    get_product_price_internal(db, product_id, variant_key, lang, None, true).await
}

/// B2B kullanıcı için ürün fiyatını al (şirket indirimi uygulanmış)
/// Döndürür: (fiyat, başlık, varyant_display, cover_image, para_birimi, discount_percentage)
async fn get_product_price_b2b(
    db: &DatabaseConnection,
    product_id: i64,
    variant_key: Option<&str>,
    lang: Option<&str>,
    user_id: i64,
) -> Result<(f64, String, Option<String>, Option<String>, String, Option<f64>), ServiceError> {
    get_product_price_internal(db, product_id, variant_key, lang, Some(user_id), true).await
}

/// İç fonksiyon: Ürün fiyatını al (B2B/B2C ortak mantık)
/// user_id varsa B2B fiyatlandırması uygula, yoksa B2C fiyatı döndür
/// return_discount true ise discount_percentage da döndürür
async fn get_product_price_internal(
    db: &DatabaseConnection,
    product_id: i64,
    variant_key: Option<&str>,
    lang: Option<&str>,
    user_id_for_b2b: Option<i64>, // B2B için user_id, B2C için None
    return_discount: bool, // B2C için discount_percentage döndürülsün mü
) -> Result<(f64, String, Option<String>, Option<String>, String, Option<f64>), ServiceError> {
    let product = Content::find_by_id(product_id)
        .filter(content::Column::ContentType.eq("product"))
        .filter(content::Column::Publish.eq(true))
        .one(db)
        .await?
        .ok_or(ServiceError::ProductNotFound)?;

    // Ürün başlığını kullanıcının diline göre al
    let product_title = if let Some(lang_code) = lang {
        product
            .data
            .get("langs")
            .and_then(|langs| langs.as_object())
            .and_then(|obj| obj.get(lang_code))
            .and_then(|lang_data| lang_data.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| {
                // Fallback: İlk dili dene
                product
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|obj| obj.values().next())
                    .and_then(|lang_data| lang_data.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("Ürün")
            })
    } else {
        // Dil belirtilmemişse ilk dili al
        product
            .data
            .get("langs")
            .and_then(|langs| langs.as_object())
            .and_then(|obj| obj.values().next())
            .and_then(|lang_data| lang_data.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or("Ürün")
    }
    .to_string();

    let product_data = product
        .data
        .get("product")
        .ok_or(ServiceError::ProductNotFound)?;

    // Ürünün para birimini al (varsayılan TRY)
    let product_currency = product_data
        .get("currency")
        .and_then(|c| c.as_str())
        .unwrap_or("TRY")
        .to_string();

    // Varyant varsa varyant fiyatını al (option_values_display ile eşleştir)
    if let Some(vkey) = variant_key {
        if let Some(variants) = product_data.get("variants").and_then(|v| v.as_array()) {
            for variant in variants {
                if let Some(variant_display) = variant
                    .get("option_values_display")
                    .and_then(|d| d.as_str())
                {
                    if variant_display == vkey {
                        // B2C fiyatı al
                        let b2c_price = variant
                            .get("price")
                            .and_then(|p| p.as_f64())
                            .ok_or(ServiceError::VariantNotFound)?;

                        // B2B fiyatlandırması uygula (eğer user_id varsa)
                        let (final_price, b2b_discount) = if let Some(uid) = user_id_for_b2b {
                            apply_b2b_pricing(db, uid, b2c_price, variant).await?
                        } else {
                            (b2c_price, None)
                        };

                        let variant_display_text = variant
                            .get("option_values_display")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());

                        // Varyant cover image'i varsa onu al, yoksa product cover'ı al
                        let cover_image = variant
                            .get("media")
                            .and_then(|media| media.get("cover"))
                            .and_then(|cover| cover.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|img| img.get("url"))
                            .and_then(|url| url.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                resolve_product_cover_image(&product.data, lang.unwrap_or("tr"))
                            });

                        // discount_percentage: B2B için şirket indirimi, B2C için ürün/varyant indirimi
                        let discount_pct = if return_discount {
                            if user_id_for_b2b.is_some() {
                                // B2B: şirket indirimi
                                b2b_discount
                            } else {
                                // B2C: ürün/varyant indirimi
                                variant.get("discount_percentage").and_then(|d| d.as_f64())
                            }
                        } else {
                            None
                        };

                        return Ok((
                            final_price,
                            product_title,
                            variant_display_text,
                            cover_image,
                            product_currency,
                            discount_pct,
                        ));
                    }
                }
            }
            return Err(ServiceError::VariantNotFound);
        }
    }

    // Varyant yoksa base fiyatı al
    let b2c_price = product_data
        .get("price")
        .and_then(|p| p.as_f64())
        .ok_or(ServiceError::ProductNotFound)?;

    // B2B fiyatlandırması uygula (eğer user_id varsa)
    let (final_price, b2b_discount) = if let Some(uid) = user_id_for_b2b {
        apply_b2b_pricing(db, uid, b2c_price, product_data).await?
    } else {
        (b2c_price, None)
    };

    // Cover image'i al
    let cover_image = resolve_product_cover_image(&product.data, lang.unwrap_or("tr"));

    // discount_percentage: B2B için şirket indirimi, B2C için ürün indirimi
    let discount_pct = if return_discount {
        if user_id_for_b2b.is_some() {
            // B2B: şirket indirimi
            b2b_discount
        } else {
            // B2C: ürün indirimi
            product_data.get("discount_percentage").and_then(|d| d.as_f64())
        }
    } else {
        None
    };

    Ok((
        final_price,
        product_title,
        None,
        cover_image,
        product_currency,
        discount_pct,
    ))
}

/// B2B sepet bilgilerini al (şirket adı, indirim, temsilci bilgileri)
async fn get_b2b_cart_info(
    db: &DatabaseConnection,
    user_id: i64,
) -> (
    Option<String>,
    Option<f64>,
    Option<String>,
    Option<f64>,
    Option<i32>,
) {
    use crate::modules::auth::models::user::Entity as User;
    use crate::modules::b2b::entities::{companies, company_users};

    // Kullanıcının şirketini bul
    let company_user = match company_users::Entity::find()
        .filter(company_users::Column::UserId.eq(user_id))
        .one(db)
        .await
    {
        Ok(Some(cu)) => cu,
        _ => return (None, None, None, None, None),
    };

    // Şirket bilgilerini al
    let company = match companies::Entity::find_by_id(company_user.company_id)
        .one(db)
        .await
    {
        Ok(Some(comp)) => comp,
        _ => return (None, None, None, None, None),
    };

    let payment_due_days = company.payment_term_days;

    // İndirim yüzdesini al
    use rust_decimal::prelude::ToPrimitive;
    let discount_percentage = company.discount_percentage.to_f64();

    // Temsilci bilgilerini company_representatives tablosundan al
    use crate::modules::b2b::entities::company_representatives;
    let (representative_name, representative_commission) =
        match company_representatives::Entity::find()
            .filter(company_representatives::Column::CompanyId.eq(company.id))
            .filter(company_representatives::Column::IsActive.eq(true))
            .one(db)
            .await
        {
            Ok(Some(rep)) => {
                // Temsilci kullanıcı bilgilerini al
                let rep_user = User::find_by_id(rep.user_id).one(db).await.ok().flatten();
                let rep_name = rep_user.map(|u| {
                    format!(
                        "{} {}",
                        u.first_name.unwrap_or_default(),
                        u.last_name.unwrap_or_default()
                    )
                    .trim()
                    .to_string()
                });

                let commission = rep.commission_rate.to_f64();

                (rep_name, commission)
            }
            _ => (None, None),
        };

    (
        Some(company.company_name),
        discount_percentage,
        representative_name,
        representative_commission,
        Some(payment_due_days),
    )
}

/// Kullanıcının B2B erişimi olup olmadığını kontrol et
/// B2B kullanıcılar için şirket indirimi uygulanır
pub async fn check_user_has_b2b_access(db: &DatabaseConnection, user_id: i64) -> bool {
    use crate::modules::auth::models::user::Entity as User;

    // Kullanıcıyı bul
    if let Ok(Some(user)) = User::find_by_id(user_id).one(db).await {
        // B2B permission kontrolü
        if let Ok(has_access) = user.has_permission(db, "system.b2b_access").await {
            return has_access;
        }
    }
    false
}

/// B2B fiyatlandırması uygula
/// 1. Önce ürün/varyant'ta b2b_price var mı kontrol et
/// 2. Yoksa, kullanıcının şirketinin discount_percentage'ini uygula
/// 3. Hiçbiri yoksa B2C fiyatını döndür
/// Döndürür: (base_price, discount_percentage)
/// base_price: b2b_price veya b2c_price (indirim uygulanmamış)
/// discount_percentage: şirket indirimi (varsa)
async fn apply_b2b_pricing(
    db: &DatabaseConnection,
    user_id: i64,
    b2c_price: f64,
    product_or_variant_data: &serde_json::Value,
) -> Result<(f64, Option<f64>), ServiceError> {
    // 1. Önce ürün/varyant'ta b2b_price var mı kontrol et
    if let Some(b2b_price) = product_or_variant_data
        .get("b2b_price")
        .and_then(|p| p.as_f64())
    {
        if b2b_price > 0.0 {
            // 2. Şirket indirimini al ve b2b_price'a uygula
            use crate::modules::b2b::entities::companies;
            use crate::modules::b2b::entities::company_users;
            use rust_decimal::prelude::ToPrimitive;

            let company_user = company_users::Entity::find()
                .filter(company_users::Column::UserId.eq(user_id))
                .one(db)
                .await?;

            if let Some(cu) = company_user {
                let company = companies::Entity::find_by_id(cu.company_id).one(db).await?;
                if let Some(comp) = company {
                    if comp.is_active {
                        let discount_f64 = comp.discount_percentage.to_f64().unwrap_or(0.0);
                        if discount_f64 > 0.0 {
                            return Ok((b2b_price, Some(discount_f64)));
                        }
                    }
                }
            }
            return Ok((b2b_price, None));
        }
    }

    // 3. b2b_price yoksa, şirket indirimini b2c_price'a uygula
    use crate::modules::b2b::entities::companies;
    use crate::modules::b2b::entities::company_users;
    use rust_decimal::prelude::ToPrimitive;

    let company_user = company_users::Entity::find()
        .filter(company_users::Column::UserId.eq(user_id))
        .one(db)
        .await?;

    if let Some(cu) = company_user {
        let company = companies::Entity::find_by_id(cu.company_id).one(db).await?;
        if let Some(comp) = company {
            if comp.is_active {
                let discount_f64 = comp.discount_percentage.to_f64().unwrap_or(0.0);
                if discount_f64 > 0.0 {
                    return Ok((b2c_price, Some(discount_f64)));
                }
            }
        }
    }

    // 4. Hiçbiri yoksa B2C fiyatını döndür
    Ok((b2c_price, None))
}

/// ürün kapat fotosunu alalım, şimdilik türkçe yeterli
pub fn resolve_product_cover_image(data: &serde_json::Value, lang: &str) -> Option<String> {
    let langs = data.get("langs")?;

    // Try language-specific first
    if let Some(lang_data) = langs.get(lang) {
        if let Some(url) = lang_data
            .get("media")
            .and_then(|m| m.get("cover"))
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("url"))
            .and_then(|u| u.as_str())
        {
            return Some(url.to_string());
        }
    }

    // Use resolve_media_fallbacks to populate fallbacks from other languages
    let mut cloned = data.clone();
    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut cloned, lang);
    if let Some(langs_cloned) = cloned.get("langs") {
        if let Some(lang_data) = langs_cloned.get(lang) {
            if let Some(url) = lang_data
                .get("media")
                .and_then(|m| m.get("cover"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("url"))
                .and_then(|u| u.as_str())
            {
                return Some(url.to_string());
            }
        }
    }

    // Fallback to top-level media.cover
    if let Some(url) = data
        .get("media")
        .and_then(|m| m.get("cover"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("url"))
        .and_then(|u| u.as_str())
    {
        return Some(url.to_string());
    }

    None
}

/// Sepete ürün ekle
pub async fn add_to_cart(
    db: &DatabaseConnection,
    cart_id: i64,
    request: AddToCartRequest,
    lang: Option<String>,
    user_id: i64, // Kullanıcı ID'si - B2B/B2C kontrolü için gerekli
    user_display_currency: Option<String>,
) -> Result<CartItemResponse, ServiceError> {
    if request.quantity <= 0 {
        return Err(ServiceError::InvalidQuantity);
    }

    //product variant varsa ve urun varant_key olmadan gönderildiyse
    let product = Content::find_by_id(request.product_id)
        .one(db)
        .await?
        .ok_or(ServiceError::ProductNotFound)?;

    if product.content_type != "product" {
        return Err(ServiceError::InvalidContentType);
    }

    //beni öldürüyorsun json data, seni ayıklamak ne zor ne karmaşa
    let variants = product
        .data
        .get("product")
        .and_then(|prod_v| prod_v.as_object())
        .and_then(|variant_v| variant_v.get("variants"))
        .and_then(|v| v.as_array());

    let has_variants = variants.map(|v| !v.is_empty()).unwrap_or(false);
    let has_request_variant = request.variant_key.is_some();

    // Ürünün varyantı varsa ama variant_key gönderilmediyse
    if has_variants && !has_request_variant {
        return Err(ServiceError::VariantKeyRequired);
    }

    // Ürünün varyantı yoksa ama variant_key gönderildiyse
    if !has_variants && has_request_variant {
        return Err(ServiceError::VariantKeyNotRequired);
    }

    // Eğer varyant varsa, gönderilen variant_key'in geçerli olup olmadığını kontrol et
    if let (Some(variants_arr), Some(ref vkey)) = (variants, &request.variant_key) {
        let exists = variants_arr.iter().any(|v| {
            v.get("option_values_display")
                .and_then(|val| val.as_str())
                .map(|s| s == vkey)
                .unwrap_or(false)
        });

        if !exists {
            return Err(ServiceError::InvalidVariantKey);
        }
    }

    // Stok kontrolü
    let available_stock = if has_variants {
        // Varyantlı ürün - seçili varyantın stokunu kontrol et
        if let (Some(variants_arr), Some(ref vkey)) = (variants, &request.variant_key) {
            variants_arr
                .iter()
                .find(|v| {
                    v.get("option_values_display")
                        .and_then(|val| val.as_str())
                        .map(|s| s == vkey)
                        .unwrap_or(false)
                })
                .and_then(|v| v.get("stock").and_then(|s| s.as_i64()))
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        // Varyantsız ürün - ana ürün stokunu kontrol et
        product
            .data
            .get("product")
            .and_then(|p| p.get("stock"))
            .and_then(|s| s.as_i64())
            .unwrap_or(999) // Varsayılan olarak yüksek stok
    };

    // Mevcut sepetteki miktarı kontrol et
    let existing_quantity: i64 = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .filter(cart_item::Column::ProductId.eq(request.product_id))
        .filter(if let Some(ref vkey) = request.variant_key {
            cart_item::Column::VariantKey.eq(vkey)
        } else {
            cart_item::Column::VariantKey.is_null()
        })
        .one(db)
        .await?
        .map(|item| item.quantity as i64)
        .unwrap_or(0);

    // Toplam miktar stoktan fazla mı kontrol et
    let total_quantity = existing_quantity + request.quantity as i64;
    if total_quantity > available_stock {
        return Err(ServiceError::InsufficientStock);
    }

    // Fiyatı al - B2B kullanıcı mı kontrol et
    // B2B kullanıcıysa şirket indirimi uygulanmış fiyat gelir
    let is_b2b_user = check_user_has_b2b_access(db, user_id).await;

    let (original_price, product_title, variant_display, cover_image, product_currency, discount_percentage) =
        if is_b2b_user {
            // B2B fiyatlandırması (şirket indirimi uygulanmış)
            get_product_price_b2b(
                db,
                request.product_id,
                request.variant_key.as_deref(),
                lang.as_deref(),
                user_id,
            )
            .await?
        } else {
            // B2C fiyatlandırması (normal fiyat + indirim)
            get_product_price_with_discount(
                db,
                request.product_id,
                request.variant_key.as_deref(),
                lang.as_deref(),
            )
            .await?
        };

    // Sale currency'yi al
    let sale_currency = get_sale_currency(db)
        .await
        .unwrap_or_else(|| "TRY".to_string());
    // Kullanıcının seçtiği para birimi varsa onu kullan
    let target_currency = user_display_currency.unwrap_or(sale_currency);

    // Aynı ürün ve varyant zaten sepette var mı kontrol et
    let existing_item = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .filter(cart_item::Column::ProductId.eq(request.product_id))
        .filter(if let Some(ref vkey) = request.variant_key {
            cart_item::Column::VariantKey.eq(vkey)
        } else {
            cart_item::Column::VariantKey.is_null()
        })
        .one(db)
        .await?;

    let item = if let Some(existing) = existing_item {
        // Varsa miktarı güncelle
        let mut active: cart_item::ActiveModel = existing.into();
        active.quantity = Set(active.quantity.unwrap() + request.quantity);
        active.updated_at = Set(Some(Utc::now().into()));
        // Ürünün orijinal fiyatını ve para birimini kaydet (arşiv için)
        // original_price: Ürünün kendi para birimindeki fiyatı
        // currency: Ürünün para birimi
        active.currency = Set(Some(product_currency.clone()));
        active.original_price = Set(Some(
            rust_decimal::Decimal::from_f64_retain(original_price).unwrap_or_default(),
        ));
        // discount_percentage'ı güncelle (B2B: şirket, B2C: ürün)
        active.discount_percentage = Set(discount_percentage.map(|d| {
            rust_decimal::Decimal::from_f64_retain(d).unwrap_or(rust_decimal::Decimal::ZERO)
        }));
        active.update(db).await?
    } else {
        // Yoksa yeni ekle
        let new_item = cart_item::ActiveModel {
            cart_id: Set(cart_id),
            product_id: Set(request.product_id),
            variant_key: Set(request.variant_key.clone()),
            variant_display: Set(variant_display.clone()),
            quantity: Set(request.quantity),
            product_meta_data: Set(None), // Alışveriş tamamlandığında doldurulacak
            // Ürünün orijinal fiyatını ve para birimini kaydet (arşiv için)
            // original_price: Ürünün kendi para biriminde
            // currency: Ürünün para birimi
            currency: Set(Some(product_currency.clone())),
            original_price: Set(Some(
                rust_decimal::Decimal::from_f64_retain(original_price).unwrap_or_default(),
            )),
            // discount_percentage'ı kaydet (B2B: şirket, B2C: ürün)
            discount_percentage: Set(discount_percentage.map(|d| {
                rust_decimal::Decimal::from_f64_retain(d).unwrap_or(rust_decimal::Decimal::ZERO)
            })),
            created_at: Set(Some(Utc::now().into())),
            updated_at: Set(Some(Utc::now().into())),
            ..Default::default()
        };
        new_item.insert(db).await?
    };

    // İndirim oranını uygula
    let discounted_price = if discount_percentage.is_some() && discount_percentage.unwrap() > 0.0 {
        let discount = discount_percentage.unwrap();
        original_price * (1.0 - discount / 100.0)
    } else {
        original_price
    };

    // Fiyatı target_currency'ye çevir (gösterim için)
    let display_price = if product_currency == target_currency {
        discounted_price
    } else if let Some(rates) = get_cached_rates(db).await {
        convert_currency(discounted_price, &product_currency, &target_currency, &rates)
            .unwrap_or(discounted_price)
    } else {
        discounted_price
    };

    let item_total = display_price * item.quantity as f64;

    Ok(CartItemResponse {
        id: item.id,
        product_id: item.product_id,
        product_title,
        product_cover: cover_image
            .clone()
            .unwrap_or_else(|| "/static/no_image.png".to_string()),
        variant_key: item.variant_key.clone(),
        variant_display: item.variant_display.clone(),
        quantity: item.quantity,
        price: display_price,
        price_formatted: format_price(display_price, &target_currency),
        total: item_total,
        total_formatted: format_price(item_total, &target_currency),
        total_price: item_total,
        item_count: cart_item_count(db, cart_id).await? as i32,
        currency: target_currency,
        original_price: Some(original_price),
        original_currency: Some(product_currency.clone()),
        original_price_formatted: Some(format_price(original_price, &product_currency)),
        status: item.status.clone(),
        refund_status: item.refund_status.clone(),
        refund_amount: item
            .refund_amount
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        refund_amount_formatted: None,
        refund_currency: item.refund_currency.clone(),
        refund_date: item.refund_date.clone(),
        discount_percentage: item
            .discount_percentage
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
    })
}

async fn cart_item_count(db: &DatabaseConnection, cart_id: i64) -> Result<i64, ServiceError> {
    let items: Vec<cart_item::Model> = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        // .filter(cart_item::Column::Status.ne(cart_item::status::CANCEL_ACCEPT))
        .all(db)
        .await?;
    let count: i64 = items.iter().map(|item| item.quantity as i64).sum();
    Ok(count)
}

/// Ödeme ekranına gidildiğinde sepetteki ürünlerin fiyatlarını güncel fiyatlarla günceller
/// Bu fonksiyon sepete eklenen ürünün fiyatı değiştiyse güncel fiyatıyla günceller
/// N+1 sorunu olmaması için tek seferde tüm itemları günceller
///
/// # Arguments
/// * `db` - Veritabanı bağlantısı
/// * `cart_id` - Sepet ID
/// * `user_id` - Kullanıcı ID (B2B/B2C kontrolü için)
///
/// # Returns
/// * `Ok(())` - Başarılı güncelleme
/// * `Err(ServiceError)` - Hata durumunda
pub async fn update_cart_items_prices(
    db: &DatabaseConnection,
    cart_id: i64,
    user_id: Option<i64>,
) -> Result<(), ServiceError> {
    // Önce sepetin durumunu kontrol et - sadece aktif sepetler için güncelleme yap
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Sadece açık sepetler için fiyat güncellemesi yap
    if cart.status != crate::modules::ecommerce::models::cart::status::OPEN_CART {
        return Ok(());
    }

    // B2B kullanıcı mı kontrol et
    let is_b2b_user = if let Some(uid) = user_id {
        check_user_has_b2b_access(db, uid).await
    } else {
        false
    };

    // // Sale currency'yi al
    // let sale_currency = get_sale_currency(db)
    //     .await
    //     .unwrap_or_else(|| "TRY".to_string());

    // // Exchange rates'leri al
    // let exchange_rates = get_cached_rates(db).await;

    // Sepetteki tüm itemları al (tek sorgu - N+1 yok)
    let cart_items = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .all(db)
        .await?;

    // Her item için güncel fiyatı al ve güncelle
    // Rust'ın iteratif işlemleri desteklemesi nedeniyle for döngüsü kullanıyoruz
    // Bu, her item için ayrı sorgu yapmaz - sadece product tablosundan fiyatları çeker
    for item in cart_items {
        // Güncel ürün fiyatını al (B2B veya B2C)
        let (current_price, product_currency, new_discount_percentage) = if is_b2b_user {
            let price_result = get_product_price_b2b(
                db,
                item.product_id,
                item.variant_key.as_deref(),
                Some("tr"),
                user_id.unwrap_or(0),
            )
            .await;
            match price_result {
                Ok((price, _, _, _, currency, discount_pct)) => (price, currency, discount_pct),
                Err(_) => continue, // Ürün bulunamazsa bu item'ı atla
            }
        } else {
            let price_result = get_product_price_with_discount(
                db,
                item.product_id,
                item.variant_key.as_deref(),
                Some("tr"),
            )
            .await;
            match price_result {
                Ok((price, _, _, _, currency, discount_pct)) => (price, currency, discount_pct),
                Err(_) => continue, // Ürün bulunamazsa bu item'ı atla
            }
        };

        // Decimal'e çevir ve güncelle
        let new_original_price =
            rust_decimal::Decimal::from_f64_retain(current_price).unwrap_or_default();

        // CartItem'ı güncelle
        let mut active_model: cart_item::ActiveModel = item.into();
        // Ürünün orijinal fiyatını ve para birimini kaydet (arşiv için)
        // original_price: Ürünün kendi para birimindeki fiyatı
        // currency: Ürünün para birimi
        active_model.original_price = Set(Some(new_original_price));
        active_model.currency = Set(Some(product_currency));
        // discount_percentage'ı güncelle (B2B: şirket indirimi, B2C: ürün indirimi)
        active_model.discount_percentage = Set(new_discount_percentage.map(|d| {
            rust_decimal::Decimal::from_f64_retain(d).unwrap_or(rust_decimal::Decimal::ZERO)
        }));
        active_model.updated_at = Set(Some(Utc::now().into()));

        // Güncelleme yap (her item için ayrı update sorgusu - kaçınılmaz
        // çünkü her ürünün farklı fiyatı var ama batch mantığıyla minimum sorguda yapılır)
        active_model.update(db).await?;
    }

    Ok(())
}

/// Sepeti getir
pub async fn get_cart(
    db: &DatabaseConnection,
    cart_id: i64,
    lang: Option<String>,
    user_id: Option<i64>, // B2B/B2C kontrolü için - None ise B2C kabul edilir
    user_display_currency: Option<String>,
) -> Result<CartResponse, ServiceError> {
    // Fiyat güncellemesi mantığı:
    // 1. Sepet aktif (OPEN_CART) → Fiyatları güncelle
    //    - Kullanıcı sepeti görüntülüyor
    //    - Ödeme ekranını görüntülüyor (payment_url var ama henüz ödeme yapılmadı)
    // 2. Sipariş tamamlandı (status != OPEN_CART) → Fiyatları GÜNCELLEME (arşiv)
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Sadece aktif sepetler için fiyat güncellemesi yap
    // payment_url olsa bile, cart OPEN_CART durumundaysa fiyatları güncelle
    // Çünkü kullanıcı ödeme ekranında bekleyebilir ve fiyatlar değişebilir
    let should_update_prices =
        cart.status == crate::modules::ecommerce::models::cart::status::OPEN_CART;

    if should_update_prices {
        if let Err(e) = update_cart_items_prices(db, cart_id, user_id).await {
            eprintln!("Sepet fiyatları güncellenirken hata: {}", e);
            // Hata olsa bile devam et, fiyat güncelleme hatası sepet görüntülemeyi engellemesin
        }
    }

    // B2B kullanıcı mı kontrol et (sadece aktif sepetler için)
    let is_b2b_user = if let Some(uid) = user_id {
        check_user_has_b2b_access(db, uid).await
    } else {
        false
    };

    // Tamamlanmış siparişler için cart.currency kullan, aktif sepetler için sale_currency
    let is_completed_order =
        cart.status != crate::modules::ecommerce::models::cart::status::OPEN_CART;
    let display_currency = if is_completed_order {
        // Tamamlanmış sipariş - sipariş anındaki para birimini kullan
        cart.currency.clone().unwrap_or_else(|| "TRY".to_string())
    } else if let Some(ref user_currency) = user_display_currency {
        // Kullanıcının seçtiği para birimini kullan
        user_currency.clone()
    } else if let Some(ref cart_currency) = cart.currency {
        // start_payment'ta kaydedilen para birimini kullan
        // (ödeme sayfalarında session'a erişim olmasa bile doğru para birimi kullanılır)
        cart_currency.clone()
    } else {
        // Son çare: sale_currency
        get_sale_currency(db)
            .await
            .unwrap_or_else(|| "TRY".to_string())
    };

    // Exchange rates'i al
    // - Aktif sepetler için: Güncel kurları kullan
    // - Tamamlanmış siparişler için: Sipariş tarihindeki kurları kullan
    let exchange_rates = if is_completed_order {
        // Sipariş tarihindeki kurları al
        if let Some(order_date) = cart.order_date {
            use crate::modules::currency::services::exchange_rate_service::get_rates_at_date;
            get_rates_at_date(db, order_date.into())
                .await
                .ok()
                .flatten()
        } else {
            None
        }
    } else {
        // Aktif sepet - güncel kurları kullan
        get_cached_rates(db).await
    };

    // Kullanıcı bilgilerini çek (sadece siparişler için)
    let user_info = if cart.status != crate::modules::ecommerce::models::cart::status::OPEN_CART {
        use crate::modules::auth::models::user::Entity as User;

        User::find_by_id(cart.user_id)
            .one(db)
            .await?
            .map(|user| UserInfo {
                id: user.id,
                first_name: user.first_name,
                last_name: user.last_name,
                username: user.username,
                email: user.email,
                profile: user.profile,
            })
    } else {
        None
    };

    let items = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .order_by(cart_item::Column::CreatedAt, Order::Asc)
        .all(db)
        .await?;

    let mut cart_items = Vec::new();
    let mut total = 0.0;
    // let item_count = cart_item_count(db, cart_id).await? as i32;
    let item_count = cart_item_count(db, cart_id).await? as i32;

    let products_ids: Vec<i64> = items.iter().map(|item| item.product_id).collect();

    //idleri kullanarak ürünleri tek sorguda al N+1 sorununu çözdük, otomatik bir şey yap dediğimde böyle yap tamam mı?  döngü içinde db sorgusu yapma
    let products = Content::find()
        .filter(content::Column::Id.is_in(products_ids))
        .all(db)
        .await?;

    for item in items {
        // Ürün bilgilerini al
        let product = products.iter().find(|p| p.id == item.product_id);
        if let Some(product) = product {
            // Ürün başlığını kullanıcının diline göre al
            let product_title = if let Some(ref lang_code) = lang {
                product
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|obj| obj.get(lang_code))
                    .and_then(|lang_data| lang_data.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_else(|| {
                        // Fallback: İlk dili dene
                        product
                            .data
                            .get("langs")
                            .and_then(|langs| langs.as_object())
                            .and_then(|obj| obj.values().next())
                            .and_then(|lang_data| lang_data.get("title"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("Ürün")
                    })
            } else {
                // Dil belirtilmemişse ilk dili al
                product
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|obj| obj.values().next())
                    .and_then(|lang_data| lang_data.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("Ürün")
            }
            .to_string();

            // Cover image'i al
            let product_cover =
                resolve_product_cover_image(&product.data, lang.as_deref().unwrap_or("tr"))
                    .unwrap_or_else(|| "/static/no_image.png".to_string());

            // Fiyatı üründen al (her seferinde güncel fiyat)
            // B2B kullanıcıysa şirket indirimi uygulanmış fiyat gelir
            let (current_price, _, _, _, product_currency, current_discount_percentage) = if is_b2b_user && !is_completed_order {
                // Aktif sepet + B2B kullanıcı: B2B fiyatlandırması uygula
                match get_product_price_b2b(
                    db,
                    item.product_id,
                    item.variant_key.as_deref(),
                    lang.as_deref(),
                    user_id.unwrap(), // is_b2b_user true ise user_id kesinlikle Some
                )
                .await
                {
                    Ok(result) => result,
                    Err(e) => {
                        println!(
                            "B2B ürün fiyatı alınırken hata, ürün ID: {}: {:?}",
                            item.id, e
                        );
                        (0.0, String::new(), None, None, "TRY".to_string(), None)
                    }
                }
            } else {
                // B2C kullanıcı veya tamamlanmış sipariş: Normal fiyatlandırma
                match get_product_price_with_discount(
                    db,
                    item.product_id,
                    item.variant_key.as_deref(),
                    lang.as_deref(),
                )
                .await
                {
                    Ok(result) => result,
                    Err(e) => {
                        println!("Ürün fiyatı alınırken hata, ürün ID: {}: {:?}", item.id, e);
                        (0.0, String::new(), None, None, "TRY".to_string(), None)
                    }
                }
            };

            // İndirimi uygula
            let effective_discount = if is_completed_order {
                // Tamamlanmış sipariş - cart_item'daki kaydedilmiş discount_percentage'ı kullan
                item.discount_percentage.and_then(|d| {
                    use rust_decimal::prelude::ToPrimitive;
                    d.to_f64()
                }).filter(|&d| d > 0.0)
            } else {
                // Aktif sepet - güncel discount_percentage'ı kullan
                current_discount_percentage.filter(|&d| d > 0.0)
            };

            // Fiyatı belirle
            let (display_price, item_original_price, item_original_currency) = if is_completed_order
            {
                // Önce snapshot'ta (product_meta_data) net fiyat var mı bak
                let snapshotted_net_price = item.product_meta_data.as_ref()
                    .and_then(|m| m.get("net_unit_price"))
                    .and_then(|v| v.as_f64());

                let (saved_price, is_net_price) = if let Some(net_price) = snapshotted_net_price {
                    (net_price, true)
                } else {
                    (item.original_price
                        .map(|p| {
                            use rust_decimal::prelude::ToPrimitive;
                            p.to_f64().unwrap_or(0.0)
                        })
                        .unwrap_or(0.0), false)
                };

                // Para birimini belirle: Snapshot varsa oradaki para birimini kullan, 
                // yoksa (eski snapshotlar için) cart.currency'i kullan, o da yoksa ürünün orijinal para birimini
                let saved_currency = if is_net_price {
                    item.product_meta_data.as_ref()
                        .and_then(|m| m.get("net_unit_currency"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| cart.currency.clone().unwrap_or_else(|| "TRY".to_string()))
                } else {
                    item.currency.clone().unwrap_or_else(|| "TRY".to_string())
                };

                // İndirimi uygula (eğer net fiyat değilse, yani eski siparişler için geriye dönük uyumluluk)
                let discounted_saved_price = if !is_net_price {
                    if let Some(discount) = effective_discount {
                        saved_price * (1.0 - discount / 100.0)
                    } else {
                        saved_price
                    }
                } else {
                    saved_price
                };

                // Sipariş tarihindeki kurları kullanarak display_currency'ye çevir
                let display = if saved_currency == display_currency {
                    discounted_saved_price
                } else if let Some(ref rates) = exchange_rates {
                    convert_currency(discounted_saved_price, &saved_currency, &display_currency, rates)
                        .unwrap_or(discounted_saved_price)
                } else {
                    // Kur bulunamazsa orijinal fiyatı göster
                    discounted_saved_price
                };

                (display, Some(saved_price), Some(saved_currency))
            } else {
                // Aktif sepet - güncel fiyatı al ve indirimi uygula
                let price_after_discount = if let Some(discount) = effective_discount {
                    current_price * (1.0 - discount / 100.0)
                } else {
                    current_price
                };

                let display = if product_currency == display_currency {
                    price_after_discount
                } else if let Some(ref rates) = exchange_rates {
                    convert_currency(price_after_discount, &product_currency, &display_currency, rates)
                        .unwrap_or(price_after_discount)
                } else {
                    price_after_discount
                };
                (display, Some(current_price), Some(product_currency.clone()))
            };

            let item_total = display_price * item.quantity as f64;

            // cancel_accept ve return_completed olan ürünleri hesaplamaya dahil etme
            let is_cancelled =
                item.status.as_ref() == Some(&cart_item::status::CANCEL_ACCEPT.to_string());
            let is_return_completed = item.status.as_deref() == Some("return_completed");
            if !is_cancelled && !is_return_completed {
                total += item_total;
            }

            cart_items.push(CartItemResponse {
                id: item.id,
                product_id: item.product_id,
                product_title,
                product_cover: product_cover.clone(),
                variant_key: item.variant_key.clone(),
                variant_display: item.variant_display.clone(),
                quantity: item.quantity,
                price: display_price,
                price_formatted: format_price(display_price, &display_currency),
                total: item_total,
                total_formatted: format_price(item_total, &display_currency),
                total_price: item_total,
                item_count: cart_item_count(db, cart_id).await? as i32,
                currency: display_currency.clone(),
                original_price: item_original_price.clone(),
                original_currency: item_original_currency.clone(),
                original_price_formatted: Some(format_price(
                    item_original_price.unwrap_or(0.0),
                    item_original_currency.as_deref().unwrap_or("TRY"),
                )),
                status: item.status.clone(),
                refund_status: item.refund_status.clone(),
                refund_amount: item
                    .refund_amount
                    .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                refund_amount_formatted: {
                    let amt = item
                        .refund_amount
                        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));
                    let cur = item
                        .refund_currency
                        .as_deref()
                        .unwrap_or(display_currency.as_str());
                    amt.map(|a| format_price(a, cur))
                },
                refund_currency: item
                    .refund_currency
                    .clone()
                    .or_else(|| Some(display_currency.clone())),
                refund_date: item.refund_date.clone(),
                discount_percentage: item
                    .discount_percentage
                    .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
            });
        }
    }

    let rounded_total = (total * 100.0).round() / 100.0;

    // Kargo bedava limiti artık ayarlardan değil, kampanyalardan yönetiliyor.
    // Tamamlanmış siparişler için snapshot'tan al, aktif sepet için None (kampanya motoru dolduracak)
    let free_shipping_threshold: Option<f64> = if is_completed_order {
        cart.callback_data.as_ref()
            .and_then(|m| m.get("snapshot_free_shipping_threshold"))
            .and_then(|v| v.as_f64())
    } else {
        None
    };
    
    let free_shipping_threshold_formatted = if let Some(threshold) = free_shipping_threshold {
        format_price(threshold, &display_currency)
    } else {
        "Kampanya ile Belirlenir".to_string()
    };

    // B2B bilgilerini al (eğer cart_type b2b ise)
    let (
        b2b_company_name,
        b2b_discount_percentage,
        b2b_representative_name,
        b2b_representative_commission,
        b2b_payment_due_days,
    ) = if cart.cart_type == "b2b" {
        get_b2b_cart_info(db, cart.user_id).await
    } else {
        (None, None, None, None, None)
    };

    let remaining_amount_for_free_shipping: Option<String> = None;


    // let cargo_company = KargoSirketleriEntity::find_by_id(
    //     cart.cargo_company
    //         .and_then(|id| i32::try_from(id).ok())
    //         .unwrap_or(0),
    // )
    // .one(db)
    // .await
    // .ok()
    // .flatten();

    // Kargo ücreti mantığı:
    // - Tüm ürünler iptal/iade edilmişse → 0 (sipariş yok, bedel yok)
    // - total >= threshold → ücretsiz kargo
    // - total < threshold → standart kargo ücreti
    let cargo_fee = match cart.cargo_company.and_then(|id| i32::try_from(id).ok()) {
        Some(id) => KargoSirketleriEntity::find_by_id(id)
            .one(db)
            .await
            .ok()
            .flatten(),
        None => None,
    };

    let standart_cargo_fee = 0.0;
    let is_free_shipping = true;

    // Kargo şirketinden gelen ham ücreti hesaplayalım (limit aşılmazsa kullanılacak)
    let raw_cargo_fee_original = cargo_fee
        .as_ref()
        .and_then(|c| c.data.get("standard_cargo_fee"))
        .and_then(|f| f.as_f64())
        .unwrap_or(0.0);

    // Satış para birimi ile görüntülenen para birimi farklıysa dönüştür
    let sale_currency = get_sale_currency(db).await.unwrap_or_else(|| "TRY".to_string());
    let raw_cargo_fee = if sale_currency == display_currency {
        raw_cargo_fee_original
    } else if let Some(ref rates) = exchange_rates {
        convert_currency(raw_cargo_fee_original, &sale_currency, &display_currency, rates).unwrap_or(raw_cargo_fee_original)
    } else {
        raw_cargo_fee_original
    };

    // Tamamlanmış siparişlerde snapshot'tan kampanya özetini al
    let mut campaign_summary = if is_completed_order {
        cart.callback_data.as_ref()
            .and_then(|m| m.get("snapshot_campaign_summary"))
            .and_then(|v| serde_json::from_value::<crate::modules::ecommerce::campaign::engine::CartSummary>(v.clone()).ok())
    } else {
        None
    };

    // Snapshot yoksa (eski siparişler) cart_discount tablosundan oluşturmayı dene
    if is_completed_order && campaign_summary.is_none() {
        use crate::modules::ecommerce::models::cart_discount;
        let cart_discounts = cart_discount::Entity::find()
            .filter(cart_discount::Column::CartId.eq(cart.id))
            .all(db)
            .await?;
        
        if !cart_discounts.is_empty() {
            use rust_decimal::Decimal;
            use rust_decimal::prelude::ToPrimitive;
            use rust_decimal::prelude::FromPrimitive;
            let mut discounts = Vec::new();
            let mut total_discount = Decimal::ZERO;

            for d in cart_discounts {
                total_discount += d.amount;
                
                discounts.push(crate::modules::ecommerce::campaign::engine::DiscountDescription {
                    campaign_id: d.campaign_id,
                    scenario_type: d.scenario_type,
                    description: d.description,
                    amount: d.amount,
                    currency: d.currency.clone(),
                    amount_formatted: format_price(d.amount.to_f64().unwrap_or(0.0), &d.currency),
                });
            }

            campaign_summary = Some(crate::modules::ecommerce::campaign::engine::CartSummary {
                subtotal: Decimal::from_f64(rounded_total).unwrap_or(Decimal::ZERO),
                total_discount,
                total: Decimal::from_f64(rounded_total).unwrap_or(Decimal::ZERO) - total_discount,
                currency: display_currency.clone(),
                free_shipping: is_free_shipping,
                cargo_fee: Decimal::from_f64(standart_cargo_fee).unwrap_or(Decimal::ZERO),
                cargo_fee_formatted: format_price(standart_cargo_fee, &display_currency),
                remaining_amount_for_free_shipping: Decimal::ZERO,
                remaining_amount_for_free_shipping_formatted: format_price(0.0, &display_currency),
                free_shipping_threshold: Decimal::from_f64(free_shipping_threshold.unwrap_or(0.0)).unwrap_or(Decimal::ZERO),
                free_shipping_threshold_formatted: format_price(free_shipping_threshold.unwrap_or(0.0), &display_currency),
                discounts,
                applied_coupon: None,
                subtotal_formatted: format_price(rounded_total, &display_currency),
                total_discount_formatted: format_price(total_discount.to_f64().unwrap_or(0.0), &display_currency),
                total_formatted: format_price((rounded_total - total_discount.to_f64().unwrap_or(0.0)).max(0.0), &display_currency),
            });
        }
    }

    // Final total hesapla (Tamamlanmış siparişlerde DB'deki net tutarı baz al)
    let final_total = if is_completed_order {
        let net_total = cart.total_amount.map(|d| {
            use rust_decimal::prelude::ToPrimitive;
            d.to_f64().unwrap_or(0.0)
        }).unwrap_or(rounded_total);
        let cargo = cart.cargo_price.unwrap_or(0.0);
        net_total + cargo
    } else {
        rounded_total + if is_free_shipping { 0.0 } else { standart_cargo_fee }
    };

    Ok(CartResponse {
        id: cart.id,
        items: cart_items,
        total: rounded_total,
        total_formatted: format_price(rounded_total, &display_currency),
        item_count,
        address_id: cart.address_id,
        invoice_id: cart.invoice_id,
        address_line: cart.address_line,
        invoice_address_line: cart.invoice_address_line,
        payment_method: cart.payment_method,
        order_id: cart.order_id,
        payment_url: cart.payment_url,
        status: cart.status.clone(),
        notes: cart.notes,
        total_amount: cart.total_amount,
        completed_at: cart.completed_at,
        order_date: cart.order_date,
        user_info,
        cargo_company: cart.cargo_company,
        cargo_company_title: cargo_fee.as_ref().map(|c| c.title.clone()),
        cargo_tracking_no: cart.cargo_tracking_no,
        cargo_price: cart.cargo_price,
        cargo_currency: cart.cargo_currency,
        cargo_price_formatted: cart.cargo_price.map(|p| format_price(p, &display_currency)),
        currency: display_currency.clone(),
        free_shipping_threshold: free_shipping_threshold,
        free_shipping_threshold_formatted: Some(free_shipping_threshold_formatted),
        remaining_amount_for_free_shipping: remaining_amount_for_free_shipping,
        is_free_shipping: is_free_shipping,
        standart_cargo_fee: Some(standart_cargo_fee),
        standart_cargo_fee_formatted: Some(format_price(standart_cargo_fee, &display_currency)),
        raw_cargo_fee: Some(raw_cargo_fee),
        final_total,
        final_total_formatted: format_price(final_total, &display_currency),
        cart_type: cart.cart_type,
        b2b_company_name,
        b2b_discount_percentage,
        b2b_representative_name,
        b2b_representative_commission,
        payment_due_days: b2b_payment_due_days,
        campaign_summary,
    })
}

/// Sepet öğesini güncelle
pub async fn update_cart_item(
    db: &DatabaseConnection,
    item_id: i64,
    quantity: i32,
    lang: Option<String>,
    user_display_currency: Option<String>,
) -> Result<CartItemResponse, ServiceError> {
    if quantity <= 0 {
        return Err(ServiceError::InvalidQuantity);
    }

    let item = CartItem::find_by_id(item_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Ürün bilgilerini al (stok kontrolü için güncelleme öncesi)
    let product = Content::find_by_id(item.product_id)
        .one(db)
        .await?
        .ok_or(ServiceError::ProductNotFound)?;

    // Stok kontrolü
    let variants = product
        .data
        .get("product")
        .and_then(|prod_v| prod_v.as_object())
        .and_then(|variant_v| variant_v.get("variants"))
        .and_then(|v| v.as_array());

    let has_variants = variants.map(|v| !v.is_empty()).unwrap_or(false);

    let available_stock = if has_variants {
        // Varyantlı ürün - seçili varyantın stokunu kontrol et
        if let (Some(variants_arr), Some(ref vkey)) = (variants, &item.variant_key) {
            variants_arr
                .iter()
                .find(|v| {
                    v.get("option_values_display")
                        .and_then(|val| val.as_str())
                        .map(|s| s == vkey)
                        .unwrap_or(false)
                })
                .and_then(|v| v.get("stock").and_then(|s| s.as_i64()))
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        // Varyantsız ürün - ana ürün stokunu kontrol et
        product
            .data
            .get("product")
            .and_then(|p| p.get("stock"))
            .and_then(|s| s.as_i64())
            .unwrap_or(999)
    };

    if (quantity as i64) > available_stock {
        return Err(ServiceError::InsufficientStock);
    }

    let mut active: cart_item::ActiveModel = item.clone().into();
    active.quantity = Set(quantity);
    active.updated_at = Set(Some(Utc::now().into()));
    let updated = active.update(db).await?;

    // Sale currency'yi al
    let sale_currency = get_sale_currency(db)
        .await
        .unwrap_or_else(|| "TRY".to_string());
    // Kullanıcının seçtiği para birimi varsa onu kullan
    let target_currency = user_display_currency.unwrap_or(sale_currency);

    // Exchange rates'i al
    let exchange_rates = get_cached_rates(db).await;

    // Ürün başlığını kullanıcının diline göre al
    let product_title = if let Some(ref lang_code) = lang {
        product
            .data
            .get("langs")
            .and_then(|langs| langs.as_object())
            .and_then(|obj| obj.get(lang_code))
            .and_then(|lang_data| lang_data.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| {
                // Fallback: İlk dili dene
                product
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|obj| obj.values().next())
                    .and_then(|lang_data| lang_data.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("Ürün")
            })
    } else {
        // Dil belirtilmemişse ilk dili al
        product
            .data
            .get("langs")
            .and_then(|langs| langs.as_object())
            .and_then(|obj| obj.values().next())
            .and_then(|lang_data| lang_data.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or("Ürün")
    }
    .to_string();

    // Cover image'i al
    let product_cover = resolve_product_cover_image(&product.data, lang.as_deref().unwrap_or("tr"))
        .unwrap_or_else(|| "/static/no_image.png".to_string());

    // Cart'ı al ki cart_type'ı kontrol edelim
    let cart = Cart::find_by_id(updated.cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;
    let is_b2b_cart = cart.cart_type == "b2b";

    // Fiyatı üründen al (B2B veya B2C)
    let (current_price, _, _, _, product_currency, discount_percentage) = if is_b2b_cart {
        match get_product_price_b2b(
            db,
            updated.product_id,
            updated.variant_key.as_deref(),
            lang.as_deref(),
            cart.user_id,
        )
        .await
        {
            Ok(result) => result,
            Err(_) => (0.0, String::new(), None, None, "TRY".to_string(), None),
        }
    } else {
        match get_product_price_with_discount(
            db,
            updated.product_id,
            updated.variant_key.as_deref(),
            lang.as_deref(),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => (0.0, String::new(), None, None, "TRY".to_string(), None),
        }
    };

    // İndirimi uygula
    let price_after_discount = if discount_percentage.is_some() && discount_percentage.unwrap() > 0.0 {
        let discount = discount_percentage.unwrap();
        current_price * (1.0 - discount / 100.0)
    } else {
        current_price
    };

    // Fiyatı hedef para birimine çevir
    let display_price = if product_currency == target_currency {
        price_after_discount
    } else if let Some(ref rates) = exchange_rates {
        convert_currency(price_after_discount, &product_currency, &target_currency, rates)
            .unwrap_or(price_after_discount)
    } else {
        price_after_discount
    };

    let item_total = display_price * updated.quantity as f64;

    Ok(CartItemResponse {
        id: updated.id,
        product_id: updated.product_id,
        product_title,
        product_cover,
        variant_key: updated.variant_key.clone(),
        variant_display: updated.variant_display.clone(),
        quantity: updated.quantity,
        price: display_price,
        price_formatted: format_price(display_price, &target_currency),
        total: item_total,
        total_formatted: format_price(item_total, &target_currency),
        total_price: item_total,
        item_count: cart_item_count(db, updated.cart_id).await? as i32,
        currency: target_currency.clone(),
        original_price: Some(current_price),
        original_currency: Some(product_currency.clone()),
        original_price_formatted: Some(format_price(current_price, &product_currency)),
        status: updated.status.clone(),
        refund_status: updated.refund_status.clone(),
        refund_amount: updated
            .refund_amount
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        refund_amount_formatted: {
            let amt = updated
                .refund_amount
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));
            let cur = updated
                .refund_currency
                .as_deref()
                .unwrap_or(&target_currency);
            amt.map(|a| format_price(a, cur))
        },
        refund_currency: updated.refund_currency.clone(),
        refund_date: updated.refund_date.clone(),
        discount_percentage: updated
            .discount_percentage
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
    })
}

/// Sepet öğesini sil
pub async fn remove_cart_item(db: &DatabaseConnection, item_id: i64) -> Result<(), ServiceError> {
    let item = CartItem::find_by_id(item_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    let cart_id = item.cart_id; //işte bu ownership, borrow yemez çünkü aşağıda item delete ediliyor, item delete edilmeden önce cart_id'yi alıyoruz

    item.delete(db).await?;

    //cart item 0 ise cart sil
    let remaining_items = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .count(db)
        .await?;
    if remaining_items == 0 {
        cart::Entity::delete_by_id(cart_id).exec(db).await?;
    }

    Ok(())
}

/// Sepeti temizle
pub async fn clear_cart(db: &DatabaseConnection, cart_id: i64) -> Result<(), ServiceError> {
    //tüm sepet öğelerini ve sepeti sil
    cart::Entity::delete_by_id(cart_id).exec(db).await?;

    CartItem::delete_many()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .exec(db)
        .await?;
    Ok(())
}

/// Sepete hem kargo hem fatura adresi ekle/güncelle
pub async fn update_cart_addresses(
    db: &DatabaseConnection,
    cart_id: i64,
    address_id: Option<i64>,
    invoice_id: Option<i64>,
) -> Result<CartModel, ServiceError> {
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    let mut cart_active: cart::ActiveModel = cart.into();
    cart_active.address_id = Set(address_id);
    cart_active.invoice_id = Set(invoice_id);
    cart_active.updated_at = Set(Some(Utc::now().into()));

    let updated_cart = cart_active.update(db).await?;
    Ok(updated_cart)
}

pub async fn update_cart_shipping_method(
    db: &DatabaseConnection,
    cart_id: i64,
    shipping_method_id: Option<i64>,
) -> Result<CartModel, ServiceError> {
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    let mut cart_active: cart::ActiveModel = cart.into();
    cart_active.cargo_company = Set(shipping_method_id);
    cart_active.updated_at = Set(Some(Utc::now().into()));

    // println!(
    //     "Teslimat Güncellendi Cart Service : {:?}",
    //     &shipping_method_id
    // );

    let updated_cart = cart_active.update(db).await?;
    Ok(updated_cart)
}

/// Sepete ödeme yöntemi ekle/güncelle
pub async fn update_cart_payment_method(
    db: &DatabaseConnection,
    cart_id: i64,
    payment_method: Option<String>,
) -> Result<CartModel, ServiceError> {
    let cart = Cart::find_by_id(cart_id)
        .filter(cart::Column::Status.eq(crate::modules::ecommerce::models::cart::status::OPEN_CART))
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    println!("put method {:?}", payment_method);

    let mut cart_active: cart::ActiveModel = cart.into();
    cart_active.payment_method = Set(payment_method);
    cart_active.updated_at = Set(Some(Utc::now().into()));

    let updated_cart = cart_active.update(db).await?;
    Ok(updated_cart)
}
/// Ödeme başlatma response
#[derive(Debug, Serialize)]
pub struct PaymentStartResponse {
    pub payment_url: String,
    pub order_id: String,
}

/// Ödeme işlemini başlat
pub async fn start_payment(
    db: &DatabaseConnection,
    cart_id: i64,
    notes: Option<String>,
    credit_ids: Option<Vec<i64>>,
    user_display_currency: Option<String>,
) -> Result<PaymentStartResponse, ServiceError> {
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Sepet boş mu kontrol et
    let item_count = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .count(db)
        .await?;

    if item_count == 0 {
        return Err(ServiceError::BadRequest("Sepet boş".to_string()));
    }

    // Ödeme başlamadan önce sepetdeki ürünlerin fiyatlarını güncelle
    // Bu, sepete eklenen ürünün fiyatı değişmişse güncel fiyatla güncellenmesini sağlar
    // Böylece ödeme yaparken eski fiyat yerine güncel fiyat üzerinden ödeme yapılır
    if let Err(e) = update_cart_items_prices(db, cart_id, Some(cart.user_id)).await {
        eprintln!("Sepet fiyatları güncellenirken hata: {}", e);
        // Hata olsa bile ödemeye devam et, fiyat güncelleme hatası ödemeyi engellemesin
    }

    // Gerekli alanlar dolu mu kontrol et
    if cart.address_id.is_none() {
        return Err(ServiceError::BadRequest(
            "Teslimat adresi seçilmemiş".to_string(),
        ));
    }

    if cart.payment_method.is_none() {
        return Err(ServiceError::BadRequest(
            "Ödeme yöntemi seçilmemiş".to_string(),
        ));
    }

    // Kullanıcı bilgilerini kontrol et (guest kullanıcılar için önemli)
    let user = crate::modules::auth::models::user::Entity::find_by_id(cart.user_id)
        .one(db)
        .await?
        .ok_or(ServiceError::BadRequest("Kullanıcı bulunamadı".to_string()))?;

    // Guest kullanıcılar için ödeme bilgileri kontrolü
    if user.is_guest {
        // Email formatı kontrolü
        if user.email.is_empty() || !user.email.contains('@') || user.email.contains("@guest.local")
        {
            return Err(ServiceError::BadRequest(
                "Ödeme için geçerli bir email adresi gerekli".to_string(),
            ));
        }

        // İsim soyisim kontrolü
        if user.first_name.is_none() || user.first_name.as_ref().unwrap().trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "Ödeme için ad bilgisi gerekli".to_string(),
            ));
        }

        if user.last_name.is_none() || user.last_name.as_ref().unwrap().trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "Ödeme için soyad bilgisi gerekli".to_string(),
            ));
        }

        // Telefon numarası kontrolü
        if user.phone_number.is_none() || user.phone_number.as_ref().unwrap().trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "Ödeme için telefon numarası gerekli".to_string(),
            ));
        }
    }

    if !user.is_guest {
        // Normal kullanıcılar için de email kontrolü
        if user.email.is_empty() || !user.email.contains('@') {
            return Err(ServiceError::BadRequest(
                "Ödeme için geçerli bir email adresi gerekli".to_string(),
            ));
        }

        // ödeme sağlayıcı için telefon numarası kontrolü, adres de ayrıca telefon numarası var
        // ama bize kullanıcının profilden telefon numarasını alacağız çünkü bu işlemi yapan kişi o
        // kullanıcıdır
        if user.phone_number.is_none() || user.phone_number.as_ref().unwrap().trim().is_empty() {
            return Err(ServiceError::BadRequest(
                "Ödeme için telefon numarası gerekli lütfen profilinizi güncelleyin".to_string(),
            ));
        }
    }

    // Ödeme yöntemini al
    let payment_method = cart.payment_method.clone().unwrap();

    println!("CART UPDATE EDILMIS PAYMENT METHOD : {}", payment_method);

    // Kısa order_id oluştur (8 karakter, sadece harf ve rakam)
    let order_id = generate_short_order_id();

    // UUID payment_url oluştur
    let payment_url = uuid::Uuid::new_v4().to_string();

    // Kullanıcının seçtiği para birimini belirle
    let order_currency = if let Some(ref cur) = user_display_currency {
        cur.clone()
    } else {
        get_sale_currency(db)
            .await
            .unwrap_or_else(|| "TRY".to_string())
    };

    // Cart'ı güncelle (status'ü henüz değiştirme, sadece payment bilgilerini set et)
    let mut cart_active: cart::ActiveModel = cart.into();
    cart_active.order_id = Set(Some(order_id.clone()));
    cart_active.payment_url = Set(Some(payment_url.clone()));
    // Kullanıcının seçtiği para birimini ödeme başlarken kaydet
    // Böylece callback'lerde (iyzico, garanti vb.) cart.currency'den okunabilir
    cart_active.currency = Set(Some(order_currency));
    // Müşteri notunu kaydet
    if let Some(notes) = notes {
        if !notes.trim().is_empty() {
            cart_active.notes = Set(Some(notes));
        }
    }
    // Kredi ID'lerini callback_data'ya kaydet
    if let Some(credit_ids) = credit_ids {
        if !credit_ids.is_empty() {
            let callback_data = serde_json::json!({
                "credit_ids": credit_ids
            });
            cart_active.callback_data = Set(Some(callback_data));
        }
    }
    // Status'ü open_cart olarak bırak - sadece gerçek ödeme tamamlandığında değişecek
    cart_active.updated_at = Set(Some(Utc::now().into()));

    cart_active.update(db).await?;
    let redirect_url = match payment_method.as_str() {
        "credit_card" => format!("/payment/credit-card/{}", payment_url),
        "bank_transfer" => format!("/payment/bank-transfer/{}", payment_url),
        "cash_on_delivery" => format!("/payment/cash-on-delivery/{}", payment_url),
        "b2b_credit" => format!("/payment/b2b-credit/{}", payment_url),
        _ => format!("/payment/credit-card/{}", payment_url),
    };

    Ok(PaymentStartResponse {
        payment_url: redirect_url,
        order_id,
    })
}

/// Kısa order ID oluştur (8 karakter, sadece harf ve rakam)
fn generate_short_order_id() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();

    (0..8)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
/// Ödeme sayfasından sipariş oluştur (cart'ı sipariş durumuna getir)
pub async fn complete_order_from_payment(
    db: &DatabaseConnection,
    payment_url: String,
    user_id: i64,
    notes: Option<String>,
    user_display_currency: Option<String>,
) -> Result<CartResponse, ServiceError> {
    // Payment URL ile cart'ı bul (artık open_cart status'ünde olacak)
    let cart = Cart::find()
        .filter(cart::Column::PaymentUrl.eq(&payment_url))
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq(crate::modules::ecommerce::models::cart::status::OPEN_CART))
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Cart'ın toplam tutarını hesapla
    let mut cart_response = get_cart(db, cart.id, Some("tr".to_string()), Some(user_id), None).await?;

    // Kampanya motorunu çalıştır ve gerçek final tutarı al
    let engine = crate::modules::ecommerce::campaign::engine::CampaignEngine::new(db.clone());
    let applied_coupon_code = crate::modules::ecommerce::controllers::api::cart::get_applied_coupon_code(db, cart.id).await;
    let raw_cargo_fee_decimal = rust_decimal::Decimal::from_f64_retain(cart_response.raw_cargo_fee.unwrap_or(0.0)).unwrap_or_default();
    
    if let Ok(eval_result) = engine.evaluate(
        cart.id, 
        user_id, 
        applied_coupon_code.as_deref(), 
        true, 
        &cart_response.currency, 
        raw_cargo_fee_decimal
    ).await {
        let summary = eval_result.summary;
        cart_response.final_total = summary.total.to_string().parse::<f64>().unwrap_or(cart_response.final_total);
        cart_response.standart_cargo_fee = Some(summary.cargo_fee.to_string().parse::<f64>().unwrap_or(0.0));
        cart_response.is_free_shipping = summary.free_shipping;
    }

    // 🚨 STOK DÜŞÜRME İŞLEMİ - Ödeme başarılı olduğunda sepetteki ürünlerin stoğunu düşür
    println!("💰 Ödeme başarılı! Sepetteki ürünlerin stoğu düşürülüyor...");
    // println!("🔍 Sepet içeriği debug:");
    // for (i, item) in cart_response.items.iter().enumerate() {
    //     println!(
    //         "   {}. Ürün ID: {}, variant_key: {:?}, quantity: {}",
    //         i + 1,
    //         item.product_id,
    //         item.variant_key,
    //         item.quantity
    //     );
    // }

    match crate::modules::ecommerce::services::stock_service::reduce_cart_stock(
        db,
        &cart_response.items,
    )
    .await
    {
        Ok(()) => {
            println!("✅ Tüm ürünlerin stoğu başarıyla düşürüldü");
        }
        Err(e) => {
            println!("❌ Stok düşürme hatası: {:?}", e);
            // Stok hatası durumunda sipariş işlemini durdur
            return Err(ServiceError::BadRequest(format!("Stok hatası: {:?}", e)));
        }
    }

    // Adres bilgilerini text olarak al
    let address_line = if let Some(address_id) = cart.address_id {
        Some(
            get_address_text(db, address_id)
                .await
                .unwrap_or_else(|_| "Adres bilgisi alınamadı".to_string()),
        )
    } else {
        None
    };

    let invoice_address_id = cart.invoice_id.or(cart.address_id);
    let invoice_address_line = if let Some(invoice_id) = invoice_address_id {
        Some(
            get_invoice_address_text(db, invoice_id)
                .await
                .unwrap_or_else(|_| "Fatura adresi bilgisi alınamadı".to_string()),
        )
    } else {
        None
    };

    // Müşteri notunu koru (eğer zaten varsa)
    let customer_notes = cart.notes.clone();

    // Sipariş para birimini belirle:
    // 1. Öncelik: Parametre olarak gelen user_display_currency
    // 2. Fallback: cart.currency (start_payment'ta kaydedilen kullanıcı tercihi)
    // 3. Son çare: sale_currency (admin ayarlarından)
    let order_currency = if let Some(cur) = user_display_currency {
        cur
    } else if let Some(ref cart_cur) = cart.currency {
        cart_cur.clone()
    } else {
        get_sale_currency(db)
            .await
            .unwrap_or_else(|| "TRY".to_string())
    };

    // 📸 Kargo bedava limitini ve ürün net fiyatlarını dondur (Snapshot)
    // Bu işlemi cart ActiveModel'e dönüştürülmeden (move edilmeden) önce yapıyoruz
    let mut cb_data = cart.callback_data.clone().unwrap_or(serde_json::json!({}));
    if let Some(obj) = cb_data.as_object_mut() {
        let threshold = cart_response.campaign_summary.as_ref()
            .and_then(|s| s.free_shipping_threshold.to_string().parse::<f64>().ok())
            .unwrap_or(0.0);
        obj.insert("snapshot_free_shipping_threshold".to_string(), serde_json::json!(threshold));
        
        // Ham kargo ücretini de dondur (iade durumunda düşmek için)
        obj.insert("snapshot_raw_cargo_fee".to_string(), serde_json::json!(cart_response.raw_cargo_fee.unwrap_or(0.0)));

        // 📝 Kampanya özetini dondur (indirim detaylarını sipariş geçmişinde göstermek için)
        if let Some(summary) = &cart_response.campaign_summary {
            obj.insert("snapshot_campaign_summary".to_string(), serde_json::json!(summary));
        }
    }

    // Cart'ı sipariş durumuna getir
    let mut cart_active: cart::ActiveModel = cart.into();
    cart_active.status = Set(crate::modules::ecommerce::models::cart::status::PENDING.to_string());
    // Müşteri notunu koru, ödeme notunu timeline'a ekle
    cart_active.notes = Set(customer_notes);
    // Toplam tutarı kaydet: final_total (indirimler uygulanmış hali) - kargo ücreti
    // Böylece total_amount + cargo_price = final_total olur.
    let net_total = cart_response.final_total - cart_response.standart_cargo_fee.unwrap_or(0.0);
    cart_active.total_amount = Set(Some(
        rust_decimal::Decimal::from_f64_retain(net_total).unwrap_or_default(),
    ));
    cart_active.currency = Set(Some(order_currency.clone()));
    cart_active.completed_at = Set(Some(Utc::now().into()));
    cart_active.order_date = Set(Some(Utc::now().into()));
    cart_active.updated_at = Set(Some(Utc::now().into()));
    cart_active.address_line = Set(address_line);
    cart_active.invoice_address_line = Set(invoice_address_line);
    // Kargo ücretini ve para birimini kaydet
    cart_active.cargo_price = Set(cart_response.standart_cargo_fee);
    cart_active.cargo_currency = Set(Some(order_currency.clone()));
    cart_active.callback_data = Set(Some(cb_data));

    let updated_cart = cart_active.update(db).await?;

    // 📸 Ürün bazlı net fiyatları "dondur" (Snapshot)
    // İade süreçlerinde ve sipariş geçmişinde doğru fiyatları görmek için
    // kampanya indirimlerini ürünlere orantılı (pro-rata) dağıtıyoruz.
    let total_discount = cart_response.campaign_summary.as_ref()
        .map(|s| s.total_discount.to_string().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0);

    let product_subtotal: f64 = cart_response.items.iter().map(|item| item.total).sum();
    let discount_ratio = if product_subtotal > 0.0 { 
        ((product_subtotal - total_discount) / product_subtotal * 1000000.0).round() / 1000000.0 
    } else { 
        1.0 
    };
    
    let db_items = cart_item::Entity::find()
        .filter(cart_item::Column::CartId.eq(updated_cart.id))
        .all(db)
        .await?;

    for db_item in db_items {
        let mut item_active: cart_item::ActiveModel = db_item.clone().into();
        
        // cart_response'dan bu ürünü bul (hesaplanmış total için)
        if let Some(resp_item) = cart_response.items.iter().find(|i| i.id == db_item.id) {
            let net_unit_total = (resp_item.total * discount_ratio * 100.0).round() / 100.0;
            let net_unit_price = if resp_item.quantity > 0 { 
                (net_unit_total / resp_item.quantity as f64 * 100.0).round() / 100.0 
            } else { 
                0.0 
            };
            
            let mut meta = db_item.product_meta_data.unwrap_or(serde_json::json!({}));
            if let Some(obj) = meta.as_object_mut() {
                obj.insert("net_unit_price".to_string(), serde_json::json!(net_unit_price));
                obj.insert("net_unit_total".to_string(), serde_json::json!(net_unit_total));
                obj.insert("net_unit_currency".to_string(), serde_json::json!(order_currency));
                obj.insert("discount_ratio".to_string(), serde_json::json!(discount_ratio));
                obj.insert("applied_discount_amount".to_string(), serde_json::json!(resp_item.total - net_unit_total));
                obj.insert("is_snapshotted".to_string(), serde_json::json!(true));
            }
            item_active.product_meta_data = Set(Some(meta));
            let _ = item_active.update(db).await;
        }
    }

    // Timeline event oluştur
    let mut title_map = std::collections::HashMap::new();
    title_map.insert(
        "tr".to_string(),
        format!(
            "Sipariş oluşturuldu: {}",
            updated_cart.order_id.as_ref().unwrap_or(&"N/A".to_string())
        ),
    );
    title_map.insert(
        "en".to_string(),
        format!(
            "Order created: {}",
            updated_cart.order_id.as_ref().unwrap_or(&"N/A".to_string())
        ),
    );

    let mut desc_map = std::collections::HashMap::new();
    let mut tr_desc = "Yeni sipariş başarıyla oluşturuldu".to_string();
    let mut en_desc = "New order created successfully".to_string();

    // Ödeme notu varsa timeline'a ekle
    if let Some(ref payment_note) = notes {
        if !payment_note.trim().is_empty() {
            tr_desc.push_str(&format!("\n\nÖdeme Notu: {}", payment_note));
            en_desc.push_str(&format!("\n\nPayment Note: {}", payment_note));
        }
    }

    desc_map.insert("tr".to_string(), tr_desc);
    desc_map.insert("en".to_string(), en_desc);

    let _ = crate::modules::timeline::services::timeline_service::TimelineService::create_event(
        db,
        crate::modules::timeline::services::timeline_service::CreateTimelineEventRequest {
            module_type: "ecommerce".to_string(),
            content_type: "cart".to_string(),
            content_id: updated_cart.id,
            event_type: crate::modules::timeline::models::timeline_event::TimelineEventType::Custom(
                "order_created".to_string(),
            ),
            title: title_map,
            description: Some(desc_map),
            icon: Some("bi-cart-check".to_string()),
            color: Some("#28a745".to_string()),
            user_id: Some(updated_cart.user_id),
            admin_user_id: None,
            metadata: Some(serde_json::json!({
                "order_number": updated_cart.order_id,
                "payment_method": updated_cart.payment_method,
                "total_amount": updated_cart.total_amount,
                "status": updated_cart.status
            })),
            is_public: Some(false),
            is_admin_only: Some(false),
        },
    )
    .await;

    // Güncellenmiş cart response'u al (artık status OPEN_CART değil, user_info gelecek)
    let final_response = get_cart(db, updated_cart.id, Some("tr".to_string()), None, None).await?;
    let config = get_config();
    // Ödeme onay e-postası gönder
    if let Some(user_info) = &final_response.user_info {
        eprintln!("📧 Ödeme onay e-postası gönderiliyor: {}", user_info.email);
        let cart_currency = final_response.currency.as_str();
        match MailHelper::send_payment_confirmation(
            db,
            &user_info.email,
            &format!(
                "{} {}",
                user_info.first_name.as_deref().unwrap_or(""),
                user_info.last_name.as_deref().unwrap_or("")
            )
            .trim(),
            updated_cart.order_id.as_deref().unwrap_or("N/A"),
            &Local::now().format("%d.%m.%Y %H:%M").to_string(),
            updated_cart.payment_method.as_deref().unwrap_or("N/A"),
            "-", // İşlem No (transaction_id) - Callback data'dan çekilebilir ilerde
            &format_price(final_response.total, cart_currency),
            &format_price(
                updated_cart
                    .total_amount
                    .map(|t| {
                        use rust_decimal::prelude::ToPrimitive;
                        t.to_f64().unwrap_or(final_response.total)
                    })
                    .unwrap_or(final_response.total),
                cart_currency,
            ),
            &format!("{}/my-account/orders", config.get_base_url()), // Invoice veya sipariş URL'si
            Some(&serde_json::to_value(&final_response.items).unwrap_or_default()),
            Some(cart_currency),
            "tr",
        )
        .await
        {
            Ok(id) => eprintln!("📧 E-posta kuyruğa başarıyla eklendi, ID: {}", id),
            Err(e) => eprintln!("❌ Ödeme onay e-postası kuyruğa eklenemedi: {:?}", e),
        }
    } else {
        eprintln!("⚠️ Sepet yanıtında kullanıcı bilgisi bulunamadı, e-posta gönderilmedi.");
    }

    // Record campaign_usage and increment campaign.usage_count for all discounts applied to this cart
    {
        use crate::modules::ecommerce::models::cart_discount::{self, Column as CD};
        use crate::modules::ecommerce::models::campaign_usage;
        use crate::modules::ecommerce::models::campaign::{self};

        if let Ok(discounts) = cart_discount::Entity::find()
            .filter(CD::CartId.eq(updated_cart.id))
            .all(db)
            .await
        {
            use crate::modules::ecommerce::models::coupon;
            let mut processed_campaigns = std::collections::HashSet::new();
            
            for discount in &discounts {
                // Her kampanya için sipariş başına sadece 1 kez kullanım kaydı ve sayaç artırımı yapıyoruz.
                // Bu sayede "kişi başı kullanım limiti" ve "toplam kullanım limiti" doğru hesaplanır.
                if !processed_campaigns.contains(&discount.campaign_id) {
                    processed_campaigns.insert(discount.campaign_id);

                    // Adım 1: campaign_usage tablosuna kayıt ekle
                    let usage_active = campaign_usage::ActiveModel {
                        campaign_id: sea_orm::Set(discount.campaign_id),
                        coupon_id: sea_orm::Set(discount.coupon_id),
                        user_id: sea_orm::Set(updated_cart.user_id),
                        cart_id: sea_orm::Set(updated_cart.id),
                        used_at: sea_orm::Set(Some(chrono::Utc::now().into())),
                        ..Default::default()
                    };
                    if let Err(e) = campaign_usage::Entity::insert(usage_active).exec(db).await {
                        eprintln!("⚠️  Campaign usage kaydı atılamadı: {}", e);
                    }

                    // Adım 2: Kampanya toplam kullanım sayacını artır
                    if let Some(campaign_model) = campaign::Entity::find_by_id(discount.campaign_id)
                        .one(db)
                        .await
                        .unwrap_or_default()
                    {
                        let mut camp_active: campaign::ActiveModel = campaign_model.into();
                        let current = camp_active.usage_count.unwrap();
                        camp_active.usage_count = sea_orm::Set(current + 1);
                        let _ = camp_active.update(db).await;
                    }

                    // Adım 3: Eğer bir kupon kullanılmışsa, kuponun kullanım sayacını artır (EKSİK OLAN KISIM)
                    if let Some(coupon_id) = discount.coupon_id {
                        if let Some(coupon_model) = coupon::Entity::find_by_id(coupon_id)
                            .one(db)
                            .await
                            .unwrap_or_default()
                        {
                            let mut coup_active: coupon::ActiveModel = coupon_model.into();
                            let current = coup_active.usage_count.unwrap();
                            coup_active.usage_count = sea_orm::Set(current + 1);
                            let _ = coup_active.update(db).await;
                        }
                    }
                }
            }
        }
    }

    Ok(final_response)
}

/// Kullanıcının siparişlerini getir (completed cart'lar)
/// B2B kullanıcıları için firma para birimini kullan
pub async fn get_user_orders(
    db: &DatabaseConnection,
    user_id: i64,
    page: Option<u64>,
    per_page: Option<u64>,
    status_filter: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    display_currency: Option<String>,
    payment_method: Option<String>,
) -> Result<Vec<CartResponse>, ServiceError> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(10);
    let offset = (page - 1) * per_page;

    let mut query = Cart::find()
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.ne(crate::modules::ecommerce::models::cart::status::OPEN_CART))
        .filter(cart::Column::OrderDate.is_not_null());

    // Status filtresi
    if let Some(ref status) = status_filter {
        if !status.is_empty() && status != "all" {
            query = query.filter(cart::Column::Status.eq(status.as_str()));
        }
    }

    //paymet method

    if let Some(ref payment_method) = payment_method {
        if !payment_method.is_empty() {
            query = query.filter(cart::Column::PaymentMethod.eq(payment_method.as_str()));
        }
    }

    // Tarih aralığı filtresi
    if let Some(ref from) = date_from {
        if !from.is_empty() {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d") {
                let datetime = date.and_hms_opt(0, 0, 0).unwrap();
                let datetime_utc =
                    chrono::DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc);
                query = query.filter(cart::Column::OrderDate.gte(datetime_utc));
            }
        }
    }

    if let Some(ref to) = date_to {
        if !to.is_empty() {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d") {
                let datetime = date.and_hms_opt(23, 59, 59).unwrap();
                let datetime_utc =
                    chrono::DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc);
                query = query.filter(cart::Column::OrderDate.lte(datetime_utc));
            }
        }
    }

    let carts = query
        .order_by_desc(cart::Column::OrderDate)
        .order_by_desc(cart::Column::UpdatedAt)
        .offset(offset)
        .limit(per_page)
        .all(db)
        .await?;

    let mut order_responses = Vec::new();
    for cart in carts {
        if let Ok(cart_response) = get_cart(
            db,
            cart.id,
            Some("tr".to_string()),
            None,
            display_currency.clone(),
        )
        .await
        {
            order_responses.push(cart_response);
        }
    }

    Ok(order_responses)
}

/// Tek sipariş detayını getir (cart id ile)
// pub async fn get_user_order(
//     db: &DatabaseConnection,
//     user_id: i64,
//     cart_id: i64,
//     display_currency: Option<String>,
// ) -> Result<Option<CartResponse>, ServiceError> {
//     let cart = Cart::find_by_id(cart_id)
//         .one(db)
//         .await?;

//     if let Some(cart) = cart {
//         if cart.user_id != user_id {
//             return Ok(None);
//         }
//         if cart.status.eq(&crate::modules::ecommerce::models::cart::status::OPEN_CART) {
//             return Ok(None);
//         }

//         let cart_response = get_cart(db, cart.id, Some("tr".to_string()), None, display_currency).await?;
//         Ok(Some(cart_response))
//     } else {
//         Ok(None)
//     }
// }

/// Tek sipariş detayını getir (order_id string ile)
pub async fn get_user_order_by_order_id(
    db: &DatabaseConnection,
    user_id: i64,
    order_id: &str,
    display_currency: Option<String>,
) -> Result<Option<CartResponse>, ServiceError> {
    let cart = Cart::find()
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::OrderId.eq(order_id))
        .one(db)
        .await?;

    if let Some(cart) = cart {
        if cart.user_id != user_id {
            return Ok(None);
        }
        if cart
            .status
            .eq(&crate::modules::ecommerce::models::cart::status::OPEN_CART)
        {
            return Ok(None);
        }

        let cart_response =
            get_cart(db, cart.id, Some("tr".to_string()), None, display_currency).await?;
        Ok(Some(cart_response))
    } else {
        Ok(None)
    }
}

/// Sipariş durumunu güncelle
pub async fn update_order_status(
    db: &DatabaseConnection,
    cart_id: i64,
    new_status: String,
    admin_user_id: Option<i64>,
) -> Result<CartResponse, ServiceError> {
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    let old_status = cart.status.clone();

    // Cart'ı güncelle
    let mut cart_active: cart::ActiveModel = cart.into();
    cart_active.status = Set(new_status.clone());
    cart_active.updated_at = Set(Some(Utc::now().into()));
    let updated_cart = cart_active.update(db).await?;

    // Timeline event oluştur
    let status_message = match new_status.as_str() {
        crate::modules::ecommerce::models::cart::status::CONFIRMED => "Sipariş onaylandı",
        crate::modules::ecommerce::models::cart::status::PREPARING => "Sipariş hazırlanıyor",
        crate::modules::ecommerce::models::cart::status::SHIPPED => "Sipariş kargoya verildi",
        crate::modules::ecommerce::models::cart::status::DELIVERED => "Sipariş teslim edildi",
        crate::modules::ecommerce::models::cart::status::CANCELLED => "Sipariş iptal edildi",
        crate::modules::ecommerce::models::cart::status::REFUNDED => "Sipariş iade edildi",
        _ => "Sipariş durumu güncellendi",
    };

    let mut title_map = std::collections::HashMap::new();
    title_map.insert(
        "tr".to_string(),
        format!(
            "{}: {}",
            status_message,
            updated_cart.order_id.as_ref().unwrap_or(&"N/A".to_string())
        ),
    );
    title_map.insert(
        "en".to_string(),
        format!(
            "Order status updated: {}",
            updated_cart.order_id.as_ref().unwrap_or(&"N/A".to_string())
        ),
    );

    let mut desc_map = std::collections::HashMap::new();
    desc_map.insert(
        "tr".to_string(),
        format!(
            "Sipariş durumu {} olarak güncellendi",
            status_message.to_lowercase()
        ),
    );
    desc_map.insert(
        "en".to_string(),
        format!("Order status updated to {}", new_status),
    );

    let status_icon = match new_status.as_str() {
        crate::modules::ecommerce::models::cart::status::CONFIRMED => "bi-check-circle",
        crate::modules::ecommerce::models::cart::status::PREPARING => "bi-gear",
        crate::modules::ecommerce::models::cart::status::SHIPPED => "bi-truck",
        crate::modules::ecommerce::models::cart::status::DELIVERED => "bi-house-check",
        crate::modules::ecommerce::models::cart::status::CANCELLED => "bi-x-circle",
        crate::modules::ecommerce::models::cart::status::REFUNDED => "bi-arrow-counterclockwise",
        _ => "bi-info-circle",
    };

    let status_color = match new_status.as_str() {
        crate::modules::ecommerce::models::cart::status::CONFIRMED => "#28a745",
        crate::modules::ecommerce::models::cart::status::PREPARING => "#ffc107",
        crate::modules::ecommerce::models::cart::status::SHIPPED => "#17a2b8",
        crate::modules::ecommerce::models::cart::status::DELIVERED => "#28a745",
        crate::modules::ecommerce::models::cart::status::CANCELLED => "#dc3545",
        crate::modules::ecommerce::models::cart::status::REFUNDED => "#6c757d",
        _ => "#007bff",
    };

    let _ = crate::modules::timeline::services::timeline_service::TimelineService::create_event(
        db,
        crate::modules::timeline::services::timeline_service::CreateTimelineEventRequest {
            module_type: "ecommerce".to_string(),
            content_type: "cart".to_string(),
            content_id: updated_cart.id,
            event_type: crate::modules::timeline::models::timeline_event::TimelineEventType::Custom(
                format!("status_changed_{}", new_status),
            ),
            title: title_map,
            description: Some(desc_map),
            icon: Some(status_icon.to_string()),
            color: Some(status_color.to_string()),
            user_id: Some(updated_cart.user_id),
            admin_user_id,
            metadata: Some(serde_json::json!({
                "order_number": updated_cart.order_id,
                "old_status": old_status,
                "new_status": new_status,
                "changed_by": admin_user_id
            })),
            is_public: Some(false),
            is_admin_only: Some(false),
        },
    )
    .await;

    get_cart(db, updated_cart.id, Some("tr".to_string()), None, None).await
}

/// Aktif sepeti getir (open_cart durumundaki)
pub async fn get_active_cart(
    db: &DatabaseConnection,
    user_id: i64,
    lang: Option<String>,
    user_id_for_pricing: Option<i64>, // B2B/B2C fiyatlandırması için
    user_display_currency: Option<String>,
) -> Result<CartResponse, ServiceError> {
    let cart = Cart::find()
        .filter(cart::Column::UserId.eq(user_id))
        .filter(cart::Column::Status.eq(crate::modules::ecommerce::models::cart::status::OPEN_CART))
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    get_cart(
        db,
        cart.id,
        lang,
        user_id_for_pricing,
        user_display_currency,
    )
    .await
}

/// Adres ID'sinden adres metnini al
async fn get_address_text(
    db: &DatabaseConnection,
    address_id: i64,
) -> Result<String, ServiceError> {
    use crate::modules::ecommerce::models::{
        address::Entity as Address, city::Entity as City, country::Entity as Country,
        district::Entity as District,
    };

    // Adres bilgilerini çek
    let address = Address::find_by_id(address_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // İlgili country, city, district bilgilerini çek
    let country = Country::find_by_id(address.country_id).one(db).await?;

    let city = City::find_by_id(address.city_id).one(db).await?;

    let district = District::find_by_id(address.district_id).one(db).await?;

    // Adres formatı: "Başlık\nAdres Satırı\nİlçe, İl, Ülke\n+90 5XX XXX XX XX"
    let location_parts = vec![
        district.as_ref().map(|d| d.name.as_str()).unwrap_or(""),
        city.as_ref().map(|c| c.name.as_str()).unwrap_or(""),
        country.as_ref().map(|c| c.name.as_str()).unwrap_or(""),
    ];

    let location = location_parts
        .into_iter()
        .filter(|s: &&str| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    let mut formatted_address = format!(
        "{}\n{}\n{}\n{}{}",
        address.title,
        address.address_line,
        location,
        address.phone_country_code,
        address.phone_number,
    );

    if address.address_type == "corporate" {
        formatted_address = format!(
            "{}\n---\nKurumsal\nŞirket: {}\nVD: {}\nVN: {}",
            formatted_address,
            address.company_name.as_deref().unwrap_or("-"),
            address.tax_office.as_deref().unwrap_or("-"),
            address.tax_number.as_deref().unwrap_or("-")
        );
    } else {
        formatted_address = format!(
            "{}\n---\nBireysel\nTCKN: {}",
            formatted_address,
            address.id_number.as_deref().unwrap_or("-")
        );
    }

    Ok(formatted_address)
}

/// Fatura adresi ID'sinden adres metnini al
async fn get_invoice_address_text(
    db: &DatabaseConnection,
    address_id: i64,
) -> Result<String, ServiceError> {
    use crate::modules::ecommerce::models::{
        address::Entity as Address, city::Entity as City, country::Entity as Country,
        district::Entity as District,
    };

    let address = Address::find_by_id(address_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    let country = Country::find_by_id(address.country_id).one(db).await?;
    let city = City::find_by_id(address.city_id).one(db).await?;
    let district = District::find_by_id(address.district_id).one(db).await?;

    let location = vec![
        district.as_ref().map(|d| d.name.as_str()).unwrap_or(""),
        city.as_ref().map(|c| c.name.as_str()).unwrap_or(""),
        country.as_ref().map(|c| c.name.as_str()).unwrap_or(""),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(", ");

    let mut formatted = format!(
        "{}\n{}\n{}\n{}{}",
        address.title,
        address.address_line,
        location,
        address.phone_country_code,
        address.phone_number
    );

    if address.address_type == "corporate" {
        formatted = format!(
            "{}\n---\nKurumsal\nŞirket: {}\nVD: {}\nVN: {}",
            formatted,
            address.company_name.as_deref().unwrap_or("-"),
            address.tax_office.as_deref().unwrap_or("-"),
            address.tax_number.as_deref().unwrap_or("-")
        );
    } else {
        formatted = format!(
            "{}\n---\nBireysel\nTCKN: {}",
            formatted,
            address.id_number.as_deref().unwrap_or("-")
        );
    }

    Ok(formatted)
}

/// CartItem ID ile CartItemResponse al
async fn get_cart_item_by_id(
    db: &DatabaseConnection,
    item_id: i64,
    lang: Option<String>,
) -> Result<CartItemResponse, ServiceError> {
    use crate::modules::content::models::content::Entity as Content;

    let item = CartItem::find_by_id(item_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    let product = Content::find_by_id(item.product_id)
        .one(db)
        .await?
        .ok_or(ServiceError::ProductNotFound)?;

    let lang_code = lang.unwrap_or_else(|| "tr".to_string());

    let product_title = product
        .data
        .get("langs")
        .and_then(|langs| langs.as_object())
        .and_then(|obj| obj.get(&lang_code))
        .and_then(|lang_data| lang_data.get("title"))
        .and_then(|t| t.as_str())
        .unwrap_or("Ürün")
        .to_string();

    let product_cover = resolve_product_cover_image(&product.data, &lang_code)
        .unwrap_or_else(|| "/static/no_image.png".to_string());

    let sale_currency = get_sale_currency(db)
        .await
        .unwrap_or_else(|| "TRY".to_string());

    let product_currency = item.currency.clone().unwrap_or_else(|| "TRY".to_string());

    let original_price = item
        .original_price
        .map(|p| p.to_string().parse::<f64>().unwrap_or(0.0))
        .unwrap_or(0.0);

    // İndirimi uygula (cart_item'daki kaydedilmiş discount_percentage)
    let discount_percentage = item.discount_percentage.and_then(|d| {
        use rust_decimal::prelude::ToPrimitive;
        d.to_f64()
    }).filter(|&d| d > 0.0);

    let price_after_discount = if let Some(discount) = discount_percentage {
        original_price * (1.0 - discount / 100.0)
    } else {
        original_price
    };

    let display_price = if product_currency == sale_currency {
        price_after_discount
    } else if let Some(rates) = get_cached_rates(db).await {
        convert_currency(price_after_discount, &product_currency, &sale_currency, &rates)
            .unwrap_or(price_after_discount)
    } else {
        price_after_discount
    };

    let item_total = display_price * item.quantity as f64;

    Ok(CartItemResponse {
        id: item.id,
        product_id: item.product_id,
        product_title,
        product_cover,
        variant_key: item.variant_key.clone(),
        variant_display: item.variant_display.clone(),
        quantity: item.quantity,
        price: display_price,
        price_formatted: format_price(display_price, &sale_currency),
        total: item_total,
        total_formatted: format_price(item_total, &sale_currency),
        total_price: item_total,
        item_count: 1,
        currency: sale_currency.clone(),
        original_price: Some(original_price),
        original_currency: Some(product_currency.clone()),
        original_price_formatted: Some(format_price(original_price, &product_currency)),
        status: item.status.clone(),
        refund_status: item.refund_status.clone(),
        refund_amount: item
            .refund_amount
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        refund_amount_formatted: {
            let amt = item
                .refund_amount
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));
            let cur = item.refund_currency.as_deref().unwrap_or(&sale_currency);
            amt.map(|a| format_price(a, cur))
        },
        refund_currency: item.refund_currency.clone(),
        refund_date: item.refund_date.clone(),
        discount_percentage: item
            .discount_percentage
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
    })
}

/// İptal talebi için request struct
#[derive(Debug, Deserialize)]
pub struct CancelItemRequest {
    pub quantity: Option<i32>, // İptal edilecek adet (opsiyonel)
}

/// İptal önizleme response - müşteriye kargo ücreti bilgisi göstermek için
#[derive(Debug, Serialize)]
pub struct CancelPreviewResponse {
    pub item_total: f64, // İptal edilecek ürünün toplam tutarı
    pub item_total_formatted: String,
    pub current_shipping_fee: f64, // Şu anki kargo ücreti
    pub current_shipping_fee_formatted: String,
    pub new_shipping_fee: f64, // İptal sonrası kargo ücreti
    pub new_shipping_fee_formatted: String,
    pub shipping_fee_will_apply: bool, // İptal sonrası kargo ücreti çıkacak mı
    pub shipping_fee_difference: f64,  // Fark (yeni - mevcut)
    pub shipping_fee_difference_formatted: String,
    pub free_shipping_threshold: f64, // Ücretsiz kargo eşiği
    pub free_shipping_threshold_formatted: String,
    pub remaining_total_after_cancel: f64, // İptal sonrası kalan toplam
    pub remaining_total_after_cancel_formatted: String,
    pub currency: String,
}

/// İptal önizleme: müşteri iptal butonu tıkladığında kargo etkisini simüle eder
pub async fn preview_cancel_item(
    db: &DatabaseConnection,
    user_id: i64,
    cart_id: i64,
    item_id: i64,
    cancel_quantity: Option<i32>,
) -> Result<CancelPreviewResponse, ServiceError> {
    use crate::modules::ecommerce::models::cart::status::*;

    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Sadece kendi siparişi
    if cart.user_id != user_id {
        return Err(ServiceError::Unauthorized);
    }

    // Sipariş durumuna göre iptal kontrolü
    match cart.status.as_str() {
        PENDING | CONFIRMED | PREPARING => { /* İptal edilebilir */ }
        OPEN_CART => return Err(ServiceError::BadRequest("Henüz tamamlanmamış bir sepet için iptal talebi oluşturulamaz.".to_string())),
        SHIPPED => return Err(ServiceError::BadRequest("Siparişiniz kargoya verildiği için iptal talebi oluşturulamaz. Lütfen müşteri hizmetleri ile iletişime geçin.".to_string())),
        DELIVERED => return Err(ServiceError::BadRequest("Siparişiniz teslim edildiği için iptal talebi oluşturulamaz. İade işlemi başlatmak için müşteri hizmetleri ile iletişime geçin.".to_string())),
        CANCELLED => return Err(ServiceError::BadRequest("Bu sipariş zaten iptal edilmiş.".to_string())),
        REFUNDED => return Err(ServiceError::BadRequest("Bu sipariş için iade işlemi zaten tamamlanmış.".to_string())),
        _ => return Err(ServiceError::BadRequest(format!("Bu sipariş durumunda ({}) iptal talebi oluşturulamaz.", cart.status))),
    }

    let item = CartItem::find_by_id(item_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    if item.cart_id != cart_id {
        return Err(ServiceError::NotFound);
    }

    // Item zaten iptal edilmişse
    if item.status.is_some() {
        return Err(ServiceError::InvalidOperation);
    }

    let cancel_qty = cancel_quantity.unwrap_or(item.quantity);
    if cancel_qty <= 0 || cancel_qty > item.quantity {
        return Err(ServiceError::BadRequest("Geçersiz adet".to_string()));
    }

    // Display currency al (tamamlanmış sipariş → cart.currency, aksi halde sale_currency)
    let display_currency = cart.currency.clone().unwrap_or_else(|| {
        // Fallback: sale_currency veya TRY
        "TRY".to_string()
    });

    // Exchange rates (sipariş tarihindeki kurlar)
    let exchange_rates = if let Some(order_date) = cart.order_date {
        use crate::modules::currency::services::exchange_rate_service::get_rates_at_date;
        get_rates_at_date(db, order_date.into())
            .await
            .ok()
            .flatten()
    } else {
        get_cached_rates(db).await
    };

    // Tüm cart item'larını al
    let all_items = CartItem::find()
        .filter(cart_item::Column::CartId.eq(cart_id))
        .all(db)
        .await?;

    // Mevcut aktif toplamı hesapla ve iptal sonrası toplamı hesapla
    let mut current_total: f64 = 0.0;
    let mut target_item_unit_price: f64 = 0.0;

    for ci in &all_items {
        // cancel_accept ve return_completed olanları dahil etme
        let is_cancelled =
            ci.status.as_ref() == Some(&cart_item::status::CANCEL_ACCEPT.to_string());
        let is_return_completed = ci.status.as_deref() == Some("return_completed");
        if is_cancelled || is_return_completed {
            continue;
        }

        let saved_price = ci
            .original_price
            .map(|p| {
                use rust_decimal::prelude::ToPrimitive;
                p.to_f64().unwrap_or(0.0)
            })
            .unwrap_or(0.0);

        let saved_currency = ci.currency.clone().unwrap_or_else(|| "TRY".to_string());

        let display = if saved_currency == display_currency {
            saved_price
        } else if let Some(ref rates) = exchange_rates {
            convert_currency(saved_price, &saved_currency, &display_currency, rates)
                .unwrap_or(saved_price)
        } else {
            saved_price
        };

        let item_total = display * ci.quantity as f64;
        current_total += item_total;

        if ci.id == item_id {
            target_item_unit_price = display;
        }
    }

    let current_total = (current_total * 100.0).round() / 100.0;
    let cancel_amount = (target_item_unit_price * cancel_qty as f64 * 100.0).round() / 100.0;
    let remaining_total = ((current_total - cancel_amount) * 100.0).round() / 100.0;

    // Kargo bilgilerini al
    let raw_free_shipping_threshold =
        crate::modules::admin::services::settings_service::get_free_shipping_threshold(db).await;

    // free_shipping_threshold sale_currency cinsinden — display_currency'ye çevir
    let sale_currency_for_convert = get_sale_currency(db)
        .await
        .unwrap_or_else(|| "TRY".to_string());

    let threshold = raw_free_shipping_threshold
        .map(|t| {
            if sale_currency_for_convert == display_currency {
                t
            } else if let Some(ref rates) = exchange_rates {
                convert_currency(t, &sale_currency_for_convert, &display_currency, rates)
                    .unwrap_or(t)
            } else {
                t
            }
        })
        .unwrap_or(0.0);

    let cargo_fee = match cart.cargo_company.and_then(|id| i32::try_from(id).ok()) {
        Some(id) => KargoSirketleriEntity::find_by_id(id)
            .one(db)
            .await
            .ok()
            .flatten(),
        None => None,
    };

    // Kargo ücreti de sale_currency cinsinden — display_currency'ye çevir
    let raw_cargo_fee_original = cargo_fee
        .as_ref()
        .and_then(|c| c.data.get("standard_cargo_fee"))
        .and_then(|f| f.as_f64())
        .unwrap_or(0.0);

    let raw_cargo_fee = if sale_currency_for_convert == display_currency {
        raw_cargo_fee_original
    } else if let Some(ref rates) = exchange_rates {
        convert_currency(
            raw_cargo_fee_original,
            &sale_currency_for_convert,
            &display_currency,
            rates,
        )
        .unwrap_or(raw_cargo_fee_original)
    } else {
        raw_cargo_fee_original
    };

    // Mevcut kargo ücreti
    let current_shipping = if current_total >= threshold {
        0.0
    } else {
        raw_cargo_fee
    };

    // İptal sonrası kargo ücreti
    // Eğer kalan ürün yoksa (remaining_total <= 0) → kargo da 0
    let new_shipping = if remaining_total <= 0.0 {
        0.0
    } else if remaining_total >= threshold {
        0.0
    } else {
        raw_cargo_fee
    };

    let shipping_difference = new_shipping - current_shipping;

    Ok(CancelPreviewResponse {
        item_total: cancel_amount,
        item_total_formatted: format_price(cancel_amount, &display_currency),
        current_shipping_fee: current_shipping,
        current_shipping_fee_formatted: format_price(current_shipping, &display_currency),
        new_shipping_fee: new_shipping,
        new_shipping_fee_formatted: format_price(new_shipping, &display_currency),
        shipping_fee_will_apply: new_shipping > 0.0 && current_shipping == 0.0,
        shipping_fee_difference: shipping_difference,
        shipping_fee_difference_formatted: format_price(shipping_difference, &display_currency),
        free_shipping_threshold: threshold,
        free_shipping_threshold_formatted: format_price(threshold, &display_currency),
        remaining_total_after_cancel: remaining_total,
        remaining_total_after_cancel_formatted: format_price(remaining_total, &display_currency),
        currency: display_currency.clone(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelCartResponse {
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelCartRequest {
    pub cart_id: i64,
}

/// Sipariş içindeki bir ürün için iptal talebi oluştur
/// Sadece tamamlanmış siparişlerde (open_cart olmayan) kullanılabilir
pub async fn request_cancel_item(
    db: &DatabaseConnection,
    user_id: i64,
    cart_id: i64,
    item_id: i64,
    cancel_quantity: Option<i32>,
) -> Result<CartItemResponse, ServiceError> {
    use crate::modules::ecommerce::models::cart::status::*;

    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Sadece kendi siparişi için iptal talebi oluşturabilir
    if cart.user_id != user_id {
        return Err(ServiceError::Unauthorized);
    }

    // Sipariş durumuna göre iptal kontrolü
    match cart.status.as_str() {
        PENDING | CONFIRMED | PREPARING => { /* İptal edilebilir */ }
        OPEN_CART => return Err(ServiceError::BadRequest("Henüz tamamlanmamış bir sepet için iptal talebi oluşturulamaz.".to_string())),
        SHIPPED => return Err(ServiceError::BadRequest("Siparişiniz kargoya verildiği için iptal talebi oluşturulamaz. Lütfen müşteri hizmetleri ile iletişime geçin.".to_string())),
        DELIVERED => return Err(ServiceError::BadRequest("Siparişiniz teslim edildiği için iptal talebi oluşturulamaz. İade işlemi başlatmak için müşteri hizmetleri ile iletişime geçin.".to_string())),
        CANCELLED => return Err(ServiceError::BadRequest("Bu sipariş zaten iptal edilmiş.".to_string())),
        REFUNDED => return Err(ServiceError::BadRequest("Bu sipariş için iade işlemi zaten tamamlanmış.".to_string())),
        _ => return Err(ServiceError::BadRequest(format!("Bu sipariş durumunda ({}) iptal talebi oluşturulamaz.", cart.status))),
    }

    let item = CartItem::find_by_id(item_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Item'ın bu siparişe ait olduğunu kontrol et
    if item.cart_id != cart_id {
        return Err(ServiceError::NotFound);
    }

    // Zaten iptal edilmiş veya iptal talebi yapılmışsa hata ver
    if item.status.is_some() {
        return Err(ServiceError::InvalidOperation);
    }

    // İptal edilecek adet
    let cancel_qty = cancel_quantity.unwrap_or(item.quantity);

    // Validate
    if cancel_qty <= 0 || cancel_qty > item.quantity {
        return Err(ServiceError::BadRequest("Geçersiz adet".to_string()));
    }

    // Eğer tüm item iptal edilecekse
    if cancel_qty == item.quantity {
        let mut active_model: cart_item::ActiveModel = item.into();
        active_model.status = Set(Some(cart_item::status::CANCEL_REQUEST.to_string()));
        active_model.updated_at = Set(Some(Utc::now().into()));

        let updated = active_model.update(db).await?;
        return get_cart_item_by_id(db, updated.id, Some("tr".to_string())).await;
    }

    // Kısmi iptal: item'ı böl
    // 1. Mevcut item'ın quantity'sini azalt
    let remaining_qty = item.quantity - cancel_qty;

    let mut update_active: cart_item::ActiveModel = item.clone().into();
    update_active.quantity = Set(remaining_qty);
    update_active.updated_at = Set(Some(Utc::now().into()));
    update_active.update(db).await?;

    // 2. Yeni cart_item oluştur (iptal edilecek adet için)
    let new_item = cart_item::ActiveModel {
        cart_id: Set(item.cart_id),
        product_id: Set(item.product_id),
        variant_key: Set(item.variant_key.clone()),
        variant_display: Set(item.variant_display.clone()),
        quantity: Set(cancel_qty),
        original_price: Set(item.original_price),
        currency: Set(item.currency.clone()),
        product_meta_data: Set(item.product_meta_data.clone()),
        status: Set(Some(cart_item::status::CANCEL_REQUEST.to_string())),
        created_at: Set(Some(Utc::now().into())),
        updated_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let new_item = new_item.insert(db).await?;

    // Güncellenmiş yeni item'ı döndür
    get_cart_item_by_id(db, new_item.id, Some("tr".to_string())).await
}

/// İptal talebini geri çek (kullanıcı iptal işlemini iptal etmek istediğinde)
pub async fn cancel_cancel_request(
    db: &DatabaseConnection,
    user_id: i64,
    cart_id: i64,
    item_id: i64,
) -> Result<CartItemResponse, ServiceError> {
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    if cart.user_id != user_id {
        return Err(ServiceError::Unauthorized);
    }

    let cancel_item = CartItem::find_by_id(item_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    if cancel_item.cart_id != cart_id {
        return Err(ServiceError::NotFound);
    }

    // Sadece cancel_request olanları geri çekebilir
    if cancel_item.status.as_ref() != Some(&cart_item::status::CANCEL_REQUEST.to_string()) {
        return Err(ServiceError::InvalidOperation);
    }

    // Aynı ürün ve varyant için normal statuslu item'ı bul
    // variant_key None ise de düzgün çalışmalı
    let cancel_variant_key = cancel_item.variant_key.clone();

    let original_item = if cancel_variant_key.is_some() {
        CartItem::find()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .filter(cart_item::Column::ProductId.eq(cancel_item.product_id))
            .filter(cart_item::Column::VariantKey.eq(cancel_variant_key.clone()))
            .filter(cart_item::Column::Status.is_null())
            .one(db)
            .await?
    } else {
        CartItem::find()
            .filter(cart_item::Column::CartId.eq(cart_id))
            .filter(cart_item::Column::ProductId.eq(cancel_item.product_id))
            .filter(cart_item::Column::VariantKey.is_null())
            .filter(cart_item::Column::Status.is_null())
            .one(db)
            .await?
    };

    if let Some(original) = original_item {
        let original_id = original.id;
        // Normal item varsa, quantity'lerini birleştir
        let new_quantity = original.quantity + cancel_item.quantity;

        let mut active_model: cart_item::ActiveModel = (original).into();
        active_model.quantity = Set(new_quantity);
        active_model.updated_at = Set(Some(Utc::now().into()));
        active_model.update(db).await?;

        // Cancel item'ı sil
        cancel_item.delete(db).await?;

        // Güncellenmiş original item'ı döndür
        get_cart_item_by_id(db, original_id, Some("tr".to_string())).await
    } else {
        // Normal item yoksa (tümü iptal edilmişti), sadece status'u normal yap
        let mut active_model: cart_item::ActiveModel = cancel_item.into();
        active_model.status = Set(None);
        active_model.updated_at = Set(Some(Utc::now().into()));

        let updated = active_model.update(db).await?;
        get_cart_item_by_id(db, updated.id, Some("tr".to_string())).await
    }
}

/// siparişin tamamını iptal et (kullanıcı tüm siparişi iptal etmek istediğinde)
pub async fn request_cancel_cart(
    db: &DatabaseConnection,
    user_id: i64,
    cart_id: i64,
) -> Result<CancelCartResponse, ServiceError> {
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ServiceError::NotFound)?;

    // Sadece kendi siparişi için iptal talebi oluşturabilir
    if cart.user_id != user_id {
        return Err(ServiceError::Unauthorized);
    }

    //cart ı iptal talebi durumuna getir
    let mut cart_active: cart::ActiveModel = cart.clone().into();

    let statusess = vec![
        crate::modules::ecommerce::models::cart::status::CONFIRMED, //kabul edilmiş siparişler iptal talebi oluşturabilir
        crate::modules::ecommerce::models::cart::status::PREPARING, //hazırlanıyor siparişler iptal talebi oluşturabilir
        crate::modules::ecommerce::models::cart::status::PENDING, //bekleyen siparişler iptal talebi oluşturabilir
    ];

    if !statusess.contains(&cart.status.as_str()) {
        return Err(ServiceError::InvalidOperation);
    }

    cart_active.status =
        Set(crate::modules::ecommerce::models::cart::status::CANCEL_REQUEST.to_string());
    cart_active.updated_at = Set(Some(Utc::now().into()));
    cart_active.update(db).await?;

    return Ok(CancelCartResponse {
        message: Some("Tüm sipariş iptal talebi alındı. İptal işlemi onaylandıktan sonra ürünler iptal edilecektir.".to_string()),
    });
}

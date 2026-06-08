// Return Service - İade talebi işlemleri (müşteri tarafı)
use crate::modules::ecommerce::models::cart::status as cart_status;
use crate::modules::ecommerce::models::cart_item;
use crate::modules::ecommerce::models::return_request::{
    self, reason as return_reason, status as return_status,
};
use crate::modules::ecommerce::models::{Cart, CartItem, ReturnRequest};
use crate::modules::utils::format_price::format_price;
use chrono::Utc;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::*;
use serde::{Deserialize, Serialize};

// ─── Error Types ───

#[derive(Debug)]
pub enum ReturnServiceError {
    NotFound,
    Unauthorized,
    BadRequest(String),
    InvalidOperation(String),
    DatabaseError(DbErr),
}

impl From<DbErr> for ReturnServiceError {
    fn from(err: DbErr) -> Self {
        ReturnServiceError::DatabaseError(err)
    }
}

impl std::fmt::Display for ReturnServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReturnServiceError::NotFound => write!(f, "Bulunamadı"),
            ReturnServiceError::Unauthorized => write!(f, "Yetkisiz erişim"),
            ReturnServiceError::BadRequest(msg) => write!(f, "{}", msg),
            ReturnServiceError::InvalidOperation(msg) => write!(f, "{}", msg),
            ReturnServiceError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
        }
    }
}

// ─── Request / Response Types ───

#[derive(Debug, Deserialize)]
pub struct CreateReturnRequest {
    pub quantity: Option<i32>,
    pub reason: String,
    pub reason_text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCargoRequest {
    pub tracking_no: String,
    pub cargo_company: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ReturnRequestResponse {
    pub id: i64,
    pub cart_id: i64,
    pub cart_item_id: i64,
    pub user_id: i64,
    pub quantity: i32,
    pub status: String,
    pub status_text: String,
    pub reason: String,
    pub reason_text: Option<String>,
    pub photos: Option<serde_json::Value>,
    pub admin_notes: Option<String>,
    pub rejection_reason: Option<String>,
    pub return_cargo_tracking_no: Option<String>,
    pub return_cargo_company: Option<String>,
    pub refund_amount: Option<f64>,
    pub refund_currency: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub approved_at: Option<String>,
    pub shipped_at: Option<String>,
    pub received_at: Option<String>,
    pub completed_at: Option<String>,
    // Ürün bilgileri (join'den)
    pub product_title: Option<String>,
    pub product_cover: Option<String>,
    pub variant_display: Option<String>,
    pub item_price: Option<f64>,
    pub item_currency: Option<String>,
}

// ─── Constants ───

/// İade talebi oluşturulabilecek maksimum gün sayısı (teslimattan sonra)
const RETURN_WINDOW_DAYS: i64 = 14;

// ─── Service Functions ───

/// Müşteri: Yeni iade talebi oluştur
pub async fn create_return_request(
    db: &DatabaseConnection,
    user_id: i64,
    cart_id: i64,
    cart_item_id: i64,
    request: CreateReturnRequest,
) -> Result<ReturnRequestResponse, ReturnServiceError> {
    // 1. Cart'ı kontrol et
    let cart = Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(ReturnServiceError::NotFound)?;

    // Kendi siparişi mi?
    if cart.user_id != user_id {
        return Err(ReturnServiceError::Unauthorized);
    }

    // Sipariş teslim edilmiş olmalı
    if cart.status != cart_status::DELIVERED {
        return Err(ReturnServiceError::InvalidOperation(
            "İade talebi yalnızca teslim edilmiş siparişler için oluşturulabilir.".to_string(),
        ));
    }

    // İade süresi kontrolü (teslimattan sonra N gün)
    if let Some(updated_at) = cart.updated_at {
        let now = Utc::now();
        let delivered_date: chrono::DateTime<Utc> = updated_at.into();
        let days_since = (now - delivered_date).num_days();
        if days_since > RETURN_WINDOW_DAYS {
            return Err(ReturnServiceError::InvalidOperation(format!(
                "İade talebi süresi dolmuş. Teslim tarihinden itibaren {} gün içinde iade talebi oluşturabilirsiniz.",
                RETURN_WINDOW_DAYS
            )));
        }
    }

    // 2. Cart item'ı kontrol et
    let item = CartItem::find_by_id(cart_item_id)
        .one(db)
        .await?
        .ok_or(ReturnServiceError::NotFound)?;

    if item.cart_id != cart_id {
        return Err(ReturnServiceError::NotFound);
    }

    // Item zaten iptal edilmişse iade yapılamaz
    if item.status.is_some() {
        return Err(ReturnServiceError::InvalidOperation(
            "Bu ürün için zaten bir iptal veya iade işlemi mevcut.".to_string(),
        ));
    }

    // 3. Bu item için zaten aktif bir iade talebi var mı?
    let existing = ReturnRequest::find()
        .filter(return_request::Column::CartItemId.eq(cart_item_id))
        .filter(return_request::Column::Status.is_not_in(vec![
            return_status::REJECTED,
            return_status::CANCELLED,
            return_status::COMPLETED,
        ]))
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(ReturnServiceError::InvalidOperation(
            "Bu ürün için zaten aktif bir iade talebi bulunmaktadır.".to_string(),
        ));
    }

    // 4. İade sebebi doğrulama
    if !return_reason::ALL.contains(&request.reason.as_str()) {
        return Err(ReturnServiceError::BadRequest(
            "Geçersiz iade sebebi.".to_string(),
        ));
    }

    // 5. Adet kontrolü
    let return_qty = request.quantity.unwrap_or(item.quantity);
    if return_qty <= 0 || return_qty > item.quantity {
        return Err(ReturnServiceError::BadRequest(
            "Geçersiz iade adedi.".to_string(),
        ));
    }

    let now: DateTimeWithTimeZone = Utc::now().into();

    // 6. Kısmi iade mi tam iade mi?
    let target_item_id = if return_qty == item.quantity {
        // Tüm adet iade ediliyor → mevcut item'ın status'unu güncelle
        let mut item_active: crate::modules::ecommerce::models::cart_item::ActiveModel =
            item.into();
        item_active.status = Set(Some("return_request".to_string()));
        item_active.updated_at = Set(Some(now));
        item_active.update(db).await?;
        cart_item_id
    } else {
        // Kısmi iade: item'ı böl (iptal mantığıyla aynı)
        // 1. Mevcut item'ın quantity'sini azalt (kalan adet, normal durumda)
        let remaining_qty = item.quantity - return_qty;
        let mut update_active: crate::modules::ecommerce::models::cart_item::ActiveModel =
            item.clone().into();
        update_active.quantity = Set(remaining_qty);
        update_active.updated_at = Set(Some(now));
        update_active.update(db).await?;

        // 2. Yeni cart_item oluştur (iade edilecek adet için, status=return_request)
        let new_item = crate::modules::ecommerce::models::cart_item::ActiveModel {
            cart_id: Set(item.cart_id),
            product_id: Set(item.product_id),
            variant_key: Set(item.variant_key.clone()),
            variant_display: Set(item.variant_display.clone()),
            quantity: Set(return_qty),
            original_price: Set(item.original_price),
            currency: Set(item.currency.clone()),
            product_meta_data: Set(item.product_meta_data.clone()),
            status: Set(Some("return_request".to_string())),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        };
        let new_item = new_item.insert(db).await?;
        new_item.id
    };

    // 7. İade talebi oluştur (target_item_id → bölünmüş veya orijinal item)
    let new_return = return_request::ActiveModel {
        cart_id: Set(cart_id),
        cart_item_id: Set(target_item_id),
        user_id: Set(user_id),
        quantity: Set(return_qty),
        status: Set(return_status::REQUESTED.to_string()),
        reason: Set(request.reason),
        reason_text: Set(request.reason_text),
        photos: Set(None),
        admin_notes: Set(None),
        rejection_reason: Set(None),
        return_cargo_tracking_no: Set(None),
        return_cargo_company: Set(None),
        refund_amount: Set(None),
        refund_currency: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
        approved_at: Set(None),
        shipped_at: Set(None),
        received_at: Set(None),
        completed_at: Set(None),
        ..Default::default()
    };

    let inserted = new_return.insert(db).await?;

    // Timeline event: İade talebi oluşturuldu
    let _ = create_return_timeline_event(db, &inserted, cart_id, user_id, "requested").await;

    // Response döndür
    to_return_response(db, &inserted).await
}

/// Müşteri: İade talebini iptal et
pub async fn cancel_return_request(
    db: &DatabaseConnection,
    user_id: i64,
    return_id: i64,
) -> Result<ReturnRequestResponse, ReturnServiceError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(ReturnServiceError::NotFound)?;

    if return_req.user_id != user_id {
        return Err(ReturnServiceError::Unauthorized);
    }

    // Sadece requested veya approved durumunda iptal edilebilir
    match return_req.status.as_str() {
        return_status::REQUESTED | return_status::APPROVED => { /* iptal edilebilir */ }
        return_status::SHIPPED => {
            return Err(ReturnServiceError::InvalidOperation(
                "Ürün kargoya verildiği için iade talebi iptal edilemez.".to_string(),
            ));
        }
        _ => {
            return Err(ReturnServiceError::InvalidOperation(
                "Bu iade talebi artık iptal edilemez.".to_string(),
            ));
        }
    }

    let now: DateTimeWithTimeZone = Utc::now().into();

    // Return request'i cancelled yap
    let mut active: return_request::ActiveModel = return_req.clone().into();
    active.status = Set(return_status::CANCELLED.to_string());
    active.updated_at = Set(Some(now));
    let updated = active.update(db).await?;

    // Cart item'ı geri al — kısmi iade yapılmışsa birleştir (cancel_cancel_request pattern)
    let return_item = CartItem::find_by_id(return_req.cart_item_id)
        .one(db)
        .await?;

    if let Some(return_item) = return_item {
        let return_variant_key = return_item.variant_key.clone();

        // Aynı ürün ve varyant için normal statuslu (null) item'ı bul
        let original_item = if return_variant_key.is_some() {
            CartItem::find()
                .filter(cart_item::Column::CartId.eq(return_item.cart_id))
                .filter(cart_item::Column::ProductId.eq(return_item.product_id))
                .filter(cart_item::Column::VariantKey.eq(return_variant_key.clone()))
                .filter(cart_item::Column::Status.is_null())
                .filter(cart_item::Column::Id.ne(return_item.id))
                .one(db)
                .await?
        } else {
            CartItem::find()
                .filter(cart_item::Column::CartId.eq(return_item.cart_id))
                .filter(cart_item::Column::ProductId.eq(return_item.product_id))
                .filter(cart_item::Column::VariantKey.is_null())
                .filter(cart_item::Column::Status.is_null())
                .filter(cart_item::Column::Id.ne(return_item.id))
                .one(db)
                .await?
        };

        if let Some(original) = original_item {
            // Normal item varsa → quantity'leri birleştir ve iade item'ı sil
            let new_quantity = original.quantity + return_item.quantity;
            let mut active_model: crate::modules::ecommerce::models::cart_item::ActiveModel =
                original.into();
            active_model.quantity = Set(new_quantity);
            active_model.updated_at = Set(Some(now));
            active_model.update(db).await?;

            // İade item'ını sil (bölünmüş parça)
            return_item.delete(db).await?;
        } else {
            // Normal item yoksa (tümü iade edilmişti) → sadece status'u null yap
            let mut item_active: crate::modules::ecommerce::models::cart_item::ActiveModel =
                return_item.into();
            item_active.status = Set(None);
            item_active.updated_at = Set(Some(now));
            item_active.update(db).await?;
        }
    }

    // Timeline event: İade talebi iptal edildi
    let _ =
        create_return_timeline_event(db, &updated, return_req.cart_id, user_id, "cancelled").await;

    to_return_response(db, &updated).await
}

pub async fn update_return_cargo(
    db: &DatabaseConnection,
    user_id: i64,
    return_id: i64,
    request: UpdateCargoRequest,
) -> Result<ReturnRequestResponse, ReturnServiceError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(ReturnServiceError::NotFound)?;

    if return_req.user_id != user_id {
        return Err(ReturnServiceError::Unauthorized);
    }

    // Sadece approved durumunda kargo bilgisi girilebilir
    if return_req.status != return_status::APPROVED {
        return Err(ReturnServiceError::InvalidOperation(
            "Kargo bilgisi yalnızca onaylanmış iade talepleri için girilebilir.".to_string(),
        ));
    }

    if request.tracking_no.trim().is_empty() {
        return Err(ReturnServiceError::BadRequest(
            "Kargo takip numarası boş olamaz.".to_string(),
        ));
    }

    let now: DateTimeWithTimeZone = Utc::now().into();

    let mut active: return_request::ActiveModel = return_req.clone().into();
    active.return_cargo_tracking_no = Set(Some(request.tracking_no));
    active.return_cargo_company = Set(request.cargo_company);
    active.status = Set(return_status::SHIPPED.to_string());
    active.shipped_at = Set(Some(now));
    active.updated_at = Set(Some(now));

    let updated = active.update(db).await?;

    // Cart item status'unu return_shipped yap
    let item = CartItem::find_by_id(return_req.cart_item_id)
        .one(db)
        .await?;
    if let Some(item) = item {
        let mut item_active: crate::modules::ecommerce::models::cart_item::ActiveModel =
            item.into();
        item_active.status = Set(Some("return_shipped".to_string()));
        item_active.updated_at = Set(Some(now));
        item_active.update(db).await?;
    }

    // Timeline event: İade kargoya verildi
    let _ =
        create_return_timeline_event(db, &updated, return_req.cart_id, user_id, "shipped").await;

    to_return_response(db, &updated).await
}

/// Müşteri: Kendi iade taleplerini listele
pub async fn get_user_return_requests(
    db: &DatabaseConnection,
    user_id: i64,
    cart_id: Option<i64>,
) -> Result<Vec<ReturnRequestResponse>, ReturnServiceError> {
    let mut query = ReturnRequest::find()
        .filter(return_request::Column::UserId.eq(user_id))
        .order_by_desc(return_request::Column::CreatedAt);

    if let Some(cid) = cart_id {
        query = query.filter(return_request::Column::CartId.eq(cid));
    }

    let returns = query.all(db).await?;

    let mut responses = Vec::with_capacity(returns.len());
    for r in &returns {
        responses.push(to_return_response(db, r).await?);
    }

    Ok(responses)
}

/// Müşteri: Tek bir iade talebini getir
pub async fn get_return_request(
    db: &DatabaseConnection,
    user_id: i64,
    return_id: i64,
) -> Result<ReturnRequestResponse, ReturnServiceError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(ReturnServiceError::NotFound)?;

    if return_req.user_id != user_id {
        return Err(ReturnServiceError::Unauthorized);
    }

    to_return_response(db, &return_req).await
}

// Belirli bir cart_item için aktif iade talebi var mı?
// pub async fn has_active_return_request(
//     db: &DatabaseConnection,
//     cart_item_id: i64,
// ) -> Result<bool, ReturnServiceError> {
//     let count = ReturnRequest::find()
//         .filter(return_request::Column::CartItemId.eq(cart_item_id))
//         .filter(return_request::Column::Status.is_not_in(vec![
//             return_status::REJECTED,
//             return_status::CANCELLED,
//             return_status::COMPLETED,
//         ]))
//         .count(db)
//         .await?;

//     Ok(count > 0)
// }

// ─── Helpers ───

fn format_dt(dt: &Option<DateTimeWithTimeZone>) -> Option<String> {
    dt.map(|d| d.to_rfc3339())
}

fn format_dt_required(dt: &DateTimeWithTimeZone) -> Option<String> {
    Some(dt.to_rfc3339())
}

fn get_status_text(status: &str) -> String {
    match status {
        return_status::REQUESTED => "İade Talebi Alındı".to_string(),
        return_status::APPROVED => "İade Onaylandı".to_string(),
        return_status::REJECTED => "İade Reddedildi".to_string(),
        return_status::SHIPPED => "İade Kargoya Verildi".to_string(),
        return_status::RECEIVED => "İade Teslim Alındı".to_string(),
        return_status::COMPLETED => "İade Tamamlandı".to_string(),
        return_status::CANCELLED => "İade İptal Edildi".to_string(),
        _ => format!("Bilinmeyen ({})", status),
    }
}

/// Model → Response dönüşümü (ürün bilgileri ile birlikte)
/// İade işlemi için timeline event oluştur (kullanıcı tarafı: requested, shipped, cancelled)
async fn create_return_timeline_event(
    db: &DatabaseConnection,
    return_req: &return_request::Model,
    cart_id: i64,
    user_id: i64,
    action: &str, // "requested", "shipped", "cancelled"
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::modules::timeline::services::{CreateTimelineEventRequest, TimelineService};

    // Ürün bilgilerini al
    let item = CartItem::find_by_id(return_req.cart_item_id)
        .one(db)
        .await
        .ok()
        .flatten();

    // Önce product_meta_data'dan dene
    let mut resolved_title: Option<String> = item.as_ref().and_then(|i| {
        i.product_meta_data
            .as_ref()
            .and_then(|meta| meta.get("title"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    });

    // product_meta_data boşsa Content tablosundan çek
    if resolved_title.is_none() {
        if let Some(ref i) = item {
            use crate::modules::content::models::content::Entity as Content;
            if let Ok(Some(product)) = Content::find_by_id(i.product_id).one(db).await {
                resolved_title = product
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|obj| obj.get("tr").or_else(|| obj.values().next()))
                    .and_then(|lang_data| lang_data.get("title"))
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    let variant_display = item.as_ref().and_then(|i| i.variant_display.clone());

    // Ürün adı + varyant bilgisini birleştir
    let product_title_base = resolved_title.as_deref().unwrap_or("Ürün");
    let product_title_with_variant = if let Some(ref variant) = variant_display {
        if !variant.is_empty() {
            format!("{} ({})", product_title_base, variant)
        } else {
            product_title_base.to_string()
        }
    } else {
        product_title_base.to_string()
    };
    let product_title = product_title_with_variant.as_str();

    let currency = item
        .as_ref()
        .and_then(|i| i.currency.as_deref())
        .unwrap_or("TRY");

    let unit_price = item
        .as_ref()
        .and_then(|i| {
            i.original_price.map(|p| {
                use rust_decimal::prelude::ToPrimitive;
                p.to_f64().unwrap_or(0.0)
            })
        })
        .unwrap_or(0.0);

    let formatted_price = format_price(unit_price * return_req.quantity as f64, currency);

    let (title_tr, title_en, desc_tr, desc_en, icon, color) = match action {
        "requested" => (
            "İade talebi oluşturuldu",
            "Return request created",
            format!(
                "{} adet {} ürünü için iade talebi oluşturdunuz",
                return_req.quantity, product_title
            ),
            format!(
                "You created a return request for {} piece of {}",
                return_req.quantity, product_title
            ),
            "bi-arrow-return-left",
            "#ffc107",
        ),
        "shipped" => (
            "İade kargoya verildi",
            "Return product shipped",
            format!(
                "{} adet {} ürününü kargoya verdiniz{}",
                return_req.quantity,
                product_title,
                return_req
                    .return_cargo_tracking_no
                    .as_ref()
                    .map(|t| format!(" (Takip No: {})", t))
                    .unwrap_or_default()
            ),
            format!(
                "You shipped {} piece of {} for return",
                return_req.quantity, product_title
            ),
            "bi-truck",
            "#17a2b8",
        ),
        "cancelled" => (
            "İade talebi iptal edildi",
            "Return request cancelled",
            format!(
                "{} adet {} ürünü için iade talebinizi iptal ettiniz",
                return_req.quantity, product_title
            ),
            format!(
                "You cancelled your return request for {} piece of {}",
                return_req.quantity, product_title
            ),
            "bi-x-circle",
            "#6c757d",
        ),
        _ => return Ok(()),
    };

    let mut title_map = std::collections::HashMap::new();
    title_map.insert("tr".to_string(), title_tr.to_string());
    title_map.insert("en".to_string(), title_en.to_string());

    let mut desc_map = std::collections::HashMap::new();
    desc_map.insert(
        "tr".to_string(),
        format!("{} - {}", desc_tr, formatted_price),
    );
    desc_map.insert(
        "en".to_string(),
        format!("{} - {}", desc_en, formatted_price),
    );

    let _ = TimelineService::create_event(
        db,
        CreateTimelineEventRequest {
            module_type: "ecommerce".to_string(),
            content_type: "cart".to_string(),
            content_id: cart_id,
            event_type: crate::modules::timeline::models::timeline_event::TimelineEventType::Custom(
                format!("return_{}", action),
            ),
            title: title_map,
            description: Some(desc_map),
            icon: Some(icon.to_string()),
            color: Some(color.to_string()),
            user_id: Some(user_id),
            admin_user_id: None,
            metadata: Some(serde_json::json!({
                "return_id": return_req.id,
                "cart_item_id": return_req.cart_item_id,
                "product_title": product_title,
                "quantity": return_req.quantity,
                "unit_price": unit_price,
                "currency": currency,
                "action": action,
                "return_cargo_tracking_no": return_req.return_cargo_tracking_no,
                "return_cargo_company": return_req.return_cargo_company,
            })),
            is_public: Some(false),
            is_admin_only: Some(false),
        },
    )
    .await;

    Ok(())
}

async fn to_return_response(
    db: &DatabaseConnection,
    r: &return_request::Model,
) -> Result<ReturnRequestResponse, ReturnServiceError> {
    // Cart item bilgilerini al
    let item = CartItem::find_by_id(r.cart_item_id).one(db).await?;

    let (product_title, product_cover, variant_display, item_price, item_currency) = if let Some(
        ref item,
    ) = item
    {
        // Önce product_meta_data'dan dene
        let mut title = item
            .product_meta_data
            .as_ref()
            .and_then(|meta| meta.get("title"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        let mut cover = item
            .product_meta_data
            .as_ref()
            .and_then(|meta| meta.get("cover"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

        // product_meta_data boşsa veya title/cover yoksa, Content tablosundan çek
        if title.is_none() || cover.is_none() {
            use crate::modules::content::models::content::Entity as Content;
            if let Ok(Some(product)) = Content::find_by_id(item.product_id).one(db).await {
                if title.is_none() {
                    // Ürün başlığını data.langs içinden al (ilk dili kullan)
                    title = product
                        .data
                        .get("langs")
                        .and_then(|langs| langs.as_object())
                        .and_then(|obj| {
                            // Önce "tr" dene, sonra ilk dili al
                            obj.get("tr").or_else(|| obj.values().next())
                        })
                        .and_then(|lang_data| lang_data.get("title"))
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                }
                if cover.is_none() {
                    cover = crate::modules::ecommerce::services::cart_service::resolve_product_cover_image(
                            &product.data,
                            "tr",
                        );
                }
            }
        }

        let variant = item.variant_display.clone();

        let price = item.original_price.map(|p| {
            use rust_decimal::prelude::ToPrimitive;
            p.to_f64().unwrap_or(0.0)
        });

        let currency = item.currency.clone();

        (title, cover, variant, price, currency)
    } else {
        (None, None, None, None, None)
    };

    // Siparişin para birimini (cart.currency) al ve fiyatı o para birimine çevir
    // cart_item.currency ürünün orijinal para birimi, cart.currency siparişin yapıldığı para birimi
    let cart = Cart::find_by_id(r.cart_id).one(db).await?;
    let (item_price, item_currency) =
        if let (Some(price), Some(ref product_currency)) = (item_price, &item_currency) {
            let order_currency = cart
                .as_ref()
                .and_then(|c| c.currency.as_deref())
                .unwrap_or("TRY");

            if product_currency == order_currency {
                // Aynı para birimi, dönüşüm gerekmez
                (Some(price), Some(order_currency.to_string()))
            } else {
                // Farklı para birimi — kur dönüşümü yap
                if let Some(rates) =
                    crate::modules::currency::services::exchange_rate_service::get_cached_rates(db)
                        .await
                {
                    let converted =
                    crate::modules::currency::services::exchange_rate_service::convert_currency(
                        price,
                        product_currency,
                        order_currency,
                        &rates,
                    )
                    .unwrap_or(price);
                    (Some(converted), Some(order_currency.to_string()))
                } else {
                    // Kur bilgisi yoksa orijinal fiyatı orijinal para birimiyle göster
                    (Some(price), Some(product_currency.clone()))
                }
            }
        } else {
            (item_price, item_currency)
        };

    Ok(ReturnRequestResponse {
        id: r.id,
        cart_id: r.cart_id,
        cart_item_id: r.cart_item_id,
        user_id: r.user_id,
        quantity: r.quantity,
        status: r.status.clone(),
        status_text: get_status_text(&r.status),
        reason: r.reason.clone(),
        reason_text: r.reason_text.clone(),
        photos: r.photos.clone(),
        admin_notes: r.admin_notes.clone(),
        rejection_reason: r.rejection_reason.clone(),
        return_cargo_tracking_no: r.return_cargo_tracking_no.clone(),
        return_cargo_company: r.return_cargo_company.clone(),
        refund_amount: r
            .refund_amount
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        refund_currency: r.refund_currency.clone(),
        created_at: format_dt_required(&r.created_at),
        updated_at: format_dt(&r.updated_at),
        approved_at: format_dt(&r.approved_at),
        shipped_at: format_dt(&r.shipped_at),
        received_at: format_dt(&r.received_at),
        completed_at: format_dt(&r.completed_at),
        product_title,
        product_cover,
        variant_display,
        item_price,
        item_currency,
    })
}

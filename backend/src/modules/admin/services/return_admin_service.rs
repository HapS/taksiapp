// Admin Return Service - İade talebi yönetimi (admin tarafı)
use crate::modules::ecommerce::models::return_request::{
    self, status as return_status, Entity as ReturnRequest,
};
use crate::modules::ecommerce::models::{cart_item, Cart, CartItem};
use crate::modules::utils::format_price::format_price;
use crate::modules::currency::services::exchange_rate_service;
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::*;
use serde::{Deserialize, Serialize};

use crate::modules::b2b::services::credit_service;

// ─── Error Types ───

#[derive(Debug)]
pub enum AdminReturnError {
    NotFound,
    InvalidOperation(String),
    DatabaseError(DbErr),
}

impl From<DbErr> for AdminReturnError {
    fn from(err: DbErr) -> Self {
        AdminReturnError::DatabaseError(err)
    }
}

impl std::fmt::Display for AdminReturnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdminReturnError::NotFound => write!(f, "İade talebi bulunamadı"),
            AdminReturnError::InvalidOperation(msg) => write!(f, "{}", msg),
            AdminReturnError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
        }
    }
}

// ─── Request / Response Types ───

#[derive(Debug, Deserialize)]
pub struct ApproveReturnRequest {
    pub admin_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectReturnRequest {
    pub rejection_reason: String,
    pub admin_notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteReturnRequest {
    pub refund_amount: Option<f64>,
    pub refund_currency: Option<String>,
    pub admin_notes: Option<String>,
    /// Stoğa geri eklensin mi? (false veya None ise stok geri eklenmez)
    pub restore_stock: Option<bool>,
    /// Stoğa geri eklenecek adet (None veya 0 ise iade adedi kadar eklenir)
    pub restore_stock_quantity: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct AdminReturnListQuery {
    pub status: Option<String>,
    pub user_id: Option<i64>,
    pub cart_id: Option<i64>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AdminReturnResponse {
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
    // Ürün bilgileri
    pub product_title: Option<String>,
    pub product_cover: Option<String>,
    pub variant_display: Option<String>,
    pub item_price: Option<f64>,
    pub item_currency: Option<String>,
    // Kullanıcı bilgileri
    pub user_email: Option<String>,
    pub user_name: Option<String>,
    // Sipariş bilgileri
    pub order_id: Option<String>,
    pub order_status: Option<String>,
}

// ─── Service Functions ───

/// Admin: İade taleplerini listele (sayfalama + filtre)
pub async fn get_admin_return_requests(
    db: &DatabaseConnection,
    query: &AdminReturnListQuery,
) -> Result<(Vec<AdminReturnResponse>, u64), AdminReturnError> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);

    let mut condition = Condition::all();

    if let Some(ref status) = query.status {
        if !status.is_empty() {
            condition = condition.add(return_request::Column::Status.eq(status.clone()));
        }
    }

    if let Some(user_id) = query.user_id {
        condition = condition.add(return_request::Column::UserId.eq(user_id));
    }

    if let Some(cart_id) = query.cart_id {
        condition = condition.add(return_request::Column::CartId.eq(cart_id));
    }

    let total = ReturnRequest::find()
        .filter(condition.clone())
        .count(db)
        .await?;

    let returns = ReturnRequest::find()
        .filter(condition)
        .order_by_desc(return_request::Column::CreatedAt)
        .offset(((page - 1) * per_page) as u64)
        .limit(per_page as u64)
        .all(db)
        .await?;

    let mut responses = Vec::with_capacity(returns.len());
    for r in &returns {
        responses.push(to_admin_return_response(db, r).await?);
    }

    Ok((responses, total))
}

/// Admin: Tek bir iade talebini getir
pub async fn get_admin_return_request(
    db: &DatabaseConnection,
    return_id: i64,
) -> Result<AdminReturnResponse, AdminReturnError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;

    to_admin_return_response(db, &return_req).await
}

/// Admin: İade talebini onayla (requested → approved)
pub async fn approve_return_request(
    db: &DatabaseConnection,
    return_id: i64,
    request: ApproveReturnRequest,
) -> Result<AdminReturnResponse, AdminReturnError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;

    if return_req.status != return_status::REQUESTED {
        return Err(AdminReturnError::InvalidOperation(format!(
            "Bu iade talebi onaylanamaz. Mevcut durum: {}",
            get_status_text(&return_req.status)
        )));
    }

    let now: DateTimeWithTimeZone = Utc::now().into();

    let mut active: return_request::ActiveModel = return_req.clone().into();
    active.status = Set(return_status::APPROVED.to_string());
    active.approved_at = Set(Some(now));
    active.updated_at = Set(Some(now));

    if let Some(notes) = request.admin_notes {
        active.admin_notes = Set(Some(notes));
    }

    let updated = active.update(db).await?;

    // Cart item status'unu return_approved yap
    let item = CartItem::find_by_id(return_req.cart_item_id)
        .one(db)
        .await?;
    if let Some(item) = item {
        let mut item_active: cart_item::ActiveModel = item.into();
        item_active.status = Set(Some("return_approved".to_string()));
        item_active.updated_at = Set(Some(now));
        item_active.update(db).await?;
    }

    // Timeline event + email
    let response = to_admin_return_response(db, &updated).await?;
    let _ = create_return_timeline_event(db, &return_req, &response, "approved").await;
    let _ = send_return_notification(db, &return_req, &response, "approved").await;

    Ok(response)
}

/// Admin: İade talebini reddet (requested → rejected)
pub async fn reject_return_request(
    db: &DatabaseConnection,
    return_id: i64,
    request: RejectReturnRequest,
) -> Result<AdminReturnResponse, AdminReturnError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;

    // Sadece requested veya approved durumunda reddedilebilir
    match return_req.status.as_str() {
        return_status::REQUESTED | return_status::APPROVED => { /* OK */ }
        _ => {
            return Err(AdminReturnError::InvalidOperation(format!(
                "Bu iade talebi reddedilemez. Mevcut durum: {}",
                get_status_text(&return_req.status)
            )));
        }
    }

    let now: DateTimeWithTimeZone = Utc::now().into();

    let mut active: return_request::ActiveModel = return_req.clone().into();
    active.status = Set(return_status::REJECTED.to_string());
    active.rejection_reason = Set(Some(request.rejection_reason));
    active.updated_at = Set(Some(now));

    if let Some(notes) = request.admin_notes {
        active.admin_notes = Set(Some(notes));
    }

    let updated = active.update(db).await?;

    // Timeline event + email (reject öncesi response al — reject sonrası item silinebilir)
    let response = to_admin_return_response(db, &updated).await?;
    let _ = create_return_timeline_event(db, &return_req, &response, "rejected").await;
    let _ = send_return_notification(db, &return_req, &response, "rejected").await;

    // Cart item'ı geri al — kısmi iade yapılmışsa birleştir
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
            let mut active_model: cart_item::ActiveModel = original.into();
            active_model.quantity = Set(new_quantity);
            active_model.updated_at = Set(Some(now));
            active_model.update(db).await?;

            // İade item'ını sil (bölünmüş parça)
            return_item.delete(db).await?;
        } else {
            // Normal item yoksa (tümü iade edilmişti) → sadece status'u null yap
            let mut item_active: cart_item::ActiveModel = return_item.into();
            item_active.status = Set(None);
            item_active.updated_at = Set(Some(now));
            item_active.update(db).await?;
        }
    }

    Ok(response)
}

/// Admin: Ürün teslim alındı olarak işaretle (shipped → received)
pub async fn mark_return_received(
    db: &DatabaseConnection,
    return_id: i64,
    admin_notes: Option<String>,
) -> Result<AdminReturnResponse, AdminReturnError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;

    if return_req.status != return_status::SHIPPED {
        return Err(AdminReturnError::InvalidOperation(format!(
            "Bu iade talebi 'teslim alındı' olarak işaretlenemez. Mevcut durum: {}",
            get_status_text(&return_req.status)
        )));
    }

    let now: DateTimeWithTimeZone = Utc::now().into();

    let mut active: return_request::ActiveModel = return_req.clone().into();
    active.status = Set(return_status::RECEIVED.to_string());
    active.received_at = Set(Some(now));
    active.updated_at = Set(Some(now));

    if let Some(notes) = admin_notes {
        active.admin_notes = Set(Some(notes));
    }

    let updated = active.update(db).await?;

    // Cart item status'unu return_received yap
    let item = CartItem::find_by_id(return_req.cart_item_id)
        .one(db)
        .await?;
    if let Some(item) = item {
        let mut item_active: cart_item::ActiveModel = item.into();
        item_active.status = Set(Some("return_received".to_string()));
        item_active.updated_at = Set(Some(now));
        item_active.update(db).await?;
    }

    // Timeline event + email
    let response = to_admin_return_response(db, &updated).await?;
    let _ = create_return_timeline_event(db, &return_req, &response, "received").await;
    let _ = send_return_notification(db, &return_req, &response, "received").await;

    Ok(response)
}

/// Admin: İade tamamlandı olarak işaretle + refund bilgisi (received → completed)
pub async fn complete_return_request(
    db: &DatabaseConnection,
    return_id: i64,
    request: CompleteReturnRequest,
) -> Result<AdminReturnResponse, AdminReturnError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;

    if return_req.status != return_status::RECEIVED {
        return Err(AdminReturnError::InvalidOperation(format!(
            "Bu iade talebi tamamlanamaz. Mevcut durum: {}. İade tamamlamak için önce ürünün teslim alınması gerekir.",
            get_status_text(&return_req.status)
        )));
    }

    let now: DateTimeWithTimeZone = Utc::now().into();

    // Siparişin para birimini al (sale_currency at order time)
    let cart = Cart::find_by_id(return_req.cart_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;
    let cart_currency = cart.currency.clone().unwrap_or_else(|| "TRY".to_string());

    // Eğer refund_amount verilmediyse, get_cart ile hesaplanmış fiyatı kullan (kur dönüşümü dahil)
    let (refund_amount, refund_currency) = if let Some(amount) = request.refund_amount {
        (
            Some(rust_decimal::Decimal::from_f64_retain(amount).unwrap_or_default()),
            request
                .refund_currency
                .unwrap_or_else(|| cart_currency.clone()),
        )
    } else {
        // get_cart ile hesaplanmış fiyatları al (döviz çevirisi dahil, sipariş tarihindeki kurlarla)
        match crate::modules::ecommerce::services::cart_service::get_cart(
            db,
            return_req.cart_id,
            Some("tr".to_string()),
            None,
            None,
        )
        .await
        {
            Ok(cart_response) => {
                // İade edilen cart_item'ı bul (get_cart'ın hesapladığı display fiyatı ile)
                if let Some(cart_item_resp) = cart_response
                    .items
                    .iter()
                    .find(|i| i.id == return_req.cart_item_id)
                {
                    // display fiyat zaten cart.currency'ye (sale_currency) çevrilmiş
                    let unit_price = cart_item_resp.price;
                    let total = unit_price * return_req.quantity as f64;
                    let total_decimal =
                        rust_decimal::Decimal::from_f64_retain(total).unwrap_or_default();
                    (Some(total_decimal), cart_currency.clone())
                } else {
                    // Cart item bulunamadıysa DB'den orijinal fiyat ile hesapla
                    let item = CartItem::find_by_id(return_req.cart_item_id)
                        .one(db)
                        .await?;
                    if let Some(ref item) = item {
                        let saved_price = item.original_price.unwrap_or_default();
                        let saved_currency =
                            item.currency.clone().unwrap_or_else(|| "TRY".to_string());

                        // Orijinal fiyatı cart currency'ye çevir
                        let refund_decimal = if saved_currency == cart_currency {
                            saved_price * rust_decimal::Decimal::from(return_req.quantity)
                        } else {
                            // Kur dönüşümü gerekiyor
                            let saved_f64 = saved_price.to_f64().unwrap_or(0.0);
                            if let Some(rates) = crate::modules::currency::services::exchange_rate_service::get_cached_rates(db).await {
                                let converted = crate::modules::currency::services::exchange_rate_service::convert_currency(
                                    saved_f64,
                                    &saved_currency,
                                    &cart_currency,
                                    &rates,
                                ).unwrap_or(saved_f64);
                                rust_decimal::Decimal::from_f64_retain(converted * return_req.quantity as f64).unwrap_or_default()
                            } else {
                                saved_price * rust_decimal::Decimal::from(return_req.quantity)
                            }
                        };
                        (Some(refund_decimal), cart_currency.clone())
                    } else {
                        (None, cart_currency.clone())
                    }
                }
            }
            Err(_) => {
                // get_cart başarısız olursa DB'den orijinal fiyat ile hesapla
                let item = CartItem::find_by_id(return_req.cart_item_id)
                    .one(db)
                    .await?;
                if let Some(ref item) = item {
                    let price = item.original_price.unwrap_or_default();
                    let total = price * rust_decimal::Decimal::from(return_req.quantity);
                    let currency = item
                        .currency
                        .clone()
                        .unwrap_or_else(|| cart_currency.clone());
                    (Some(total), currency)
                } else {
                    (None, cart_currency.clone())
                }
            }
        }
    };

    let mut active: return_request::ActiveModel = return_req.clone().into();
    active.status = Set(return_status::COMPLETED.to_string());
    active.completed_at = Set(Some(now));
    active.updated_at = Set(Some(now));
    active.refund_amount = Set(refund_amount);
    active.refund_currency = Set(Some(refund_currency.clone()));

    if let Some(notes) = request.admin_notes {
        active.admin_notes = Set(Some(notes));
    }

    let updated = active.update(db).await?;

    // Ödeme yöntemi belirleme: cart.payment_method üzerinden
    let payment_method = cart.payment_method.clone().unwrap_or_default();
    let is_b2b_credit = cart.cart_type == "b2b" && payment_method == "b2b_credit";

    // Refund method belirleme
    let refund_method = if is_b2b_credit {
        "b2b_credit".to_string()
    } else {
        // Banka/kredi kartı vs. — admin manuel banka iadesi yapacak
        payment_method.clone()
    };

    // Refund status belirleme
    let refund_status_str = if is_b2b_credit {
        "credited_b2b".to_string()
    } else {
        // Banka iadesi admin tarafından ayrıca yapılacak, ama iade tamamlandığında
        // en azından "return_refunded" olarak işaretleyelim
        "bank_refunded".to_string()
    };

    // B2B kredi iadesi: b2b_credit_transactions'a refund kaydı oluştur ve used_credit düş
    if is_b2b_credit {
        if let Some(ref amt) = refund_amount {
            // Şirketi bul (company_users tablosu üzerinden — siparişi veren kişi şirket sahibi olmayabilir)
            use crate::modules::b2b::entities::company_users;
            let company_user = company_users::Entity::find()
                .filter(company_users::Column::UserId.eq(cart.user_id))
                .one(db)
                .await?;

            let company = if let Some(ref cu) = company_user {
                crate::modules::b2b::entities::companies::Entity::find_by_id(cu.company_id)
                    .one(db)
                    .await?
            } else {
                // Fallback: companies.user_id üzerinden ara (eski kayıtlar için)
                crate::modules::b2b::entities::companies::Entity::find()
                    .filter(
                        crate::modules::b2b::entities::companies::Column::UserId.eq(cart.user_id),
                    )
                    .one(db)
                    .await?
            };

            if let Some(company) = company {
                let order_id_str = cart
                    .order_id
                    .clone()
                    .unwrap_or_else(|| return_req.cart_id.to_string());

                let description = format!(
                    "İade #{} - Sipariş #{} (iade talebi tamamlandı)",
                    return_id, order_id_str
                );

                // İade tutarını şirketin referans para birimine çevir
                let company_currency = company.currency.clone().unwrap_or_else(|| "TRY".to_string());
                let refund_amt_converted = if refund_currency == company_currency {
                    *amt
                } else {
                    let rates = exchange_rate_service::get_cached_rates(db).await;
                    if let Some(rates) = rates {
                        let amount_f64 = amt.to_string().parse::<f64>().unwrap_or(0.0);
                        exchange_rate_service::convert_currency(amount_f64, &refund_currency, &company_currency, &rates)
                            .and_then(|v| rust_decimal::Decimal::from_f64_retain(v))
                            .unwrap_or(*amt)
                    } else {
                        *amt
                    }
                };

                match credit_service::create_refund_transaction(
                    db,
                    company.id,
                    Some(return_req.cart_id),
                    refund_amt_converted,
                    company_currency.clone(),
                    Some(description),
                )
                .await
                {
                    Ok(transaction) => {
                        println!(
                            "✅ B2B credit refund for return #{}: company_id={}, amount={} {} (original: {} {}), transaction_id={}",
                            return_id, company.id, refund_amt_converted, company_currency, amt, refund_currency, transaction.id
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️ B2B credit refund failed for return #{}: company_id={}, error={}",
                            return_id, company.id, e
                        );
                        // Kredi iadesi başarısız olsa bile iade sürecini durdurmuyoruz
                        // Admin durumu görebilir ve manuel düzeltme yapabilir
                    }
                }
            } else {
                eprintln!(
                    "⚠️ Company not found for B2B credit refund: return_id={}, user_id={}",
                    return_id, cart.user_id
                );
            }
        }
    }

    // Cart item status'unu return_completed yap + refund bilgilerini kaydet
    let item = CartItem::find_by_id(return_req.cart_item_id)
        .one(db)
        .await?;

    // Stok geri yükleme için ürün bilgilerini sakla (item consumed olmadan önce)
    let stock_product_id = item.as_ref().map(|i| i.product_id);
    let stock_variant_key = item.as_ref().and_then(|i| i.variant_key.clone());

    if let Some(item) = item {
        let mut item_active: cart_item::ActiveModel = item.into();
        item_active.status = Set(Some("return_completed".to_string()));
        item_active.refund_currency = Set(Some(refund_currency.clone()));
        item_active.refund_status = Set(Some(refund_status_str));
        item_active.refund_method = Set(Some(refund_method));
        if let Some(ref amt) = refund_amount {
            item_active.refund_amount = Set(Some(*amt));
            item_active.refund_date = Set(Some(now));
        }
        item_active.updated_at = Set(Some(now));
        item_active.update(db).await?;
    }

    // Stok geri yükleme: Admin "stoğa geri ekle" seçtiyse
    let stock_restored_quantity = if request.restore_stock.unwrap_or(false) {
        if let Some(product_id) = stock_product_id {
            // Geri eklenecek adet: admin belirlediyse onu kullan, yoksa iade adedi kadar
            let qty = request
                .restore_stock_quantity
                .filter(|&q| q > 0)
                .unwrap_or(return_req.quantity);

            // İade adedinden fazla stok eklenemez
            let qty = qty.min(return_req.quantity);

            match crate::modules::ecommerce::services::stock_restoration_service::restore_stock(
                db,
                product_id,
                stock_variant_key.as_deref(),
                qty,
            )
            .await
            {
                Ok(new_stock) => {
                    println!(
                        "✅ Stok geri yüklendi: return_id={}, product_id={}, variant={:?}, qty={}, new_stock={}",
                        return_id, product_id, stock_variant_key, qty, new_stock
                    );
                    Some(qty)
                }
                Err(e) => {
                    eprintln!(
                        "⚠️ Stok geri yükleme hatası (iade işlemi devam ediyor): return_id={}, product_id={}, error={:?}",
                        return_id, product_id, e
                    );
                    None
                }
            }
        } else {
            None
        }
    } else {
        println!(
            "ℹ️ Stok geri yükleme yapılmadı (admin seçmedi): return_id={}",
            return_id
        );
        None
    };

    // Cart total'ı yeniden hesapla (iade edilen ürünler düşülsün)
    update_cart_total_after_return(db, return_req.cart_id).await?;

    // Timeline event + email
    let response = to_admin_return_response(db, &updated).await?;
    let _ = create_return_timeline_event(db, &return_req, &response, "completed").await;
    let _ = send_return_notification(db, &return_req, &response, "completed").await;

    // Stok geri yükleme bilgisini timeline'a ekle
    if let Some(restored_qty) = stock_restored_quantity {
        let product_title = response.product_title.as_deref().unwrap_or("Ürün");
        let variant_info = response
            .variant_display
            .as_ref()
            .filter(|v| !v.is_empty())
            .map(|v| format!(" ({})", v))
            .unwrap_or_default();

        use crate::modules::timeline::services::{CreateTimelineEventRequest, TimelineService};

        let mut title_map = std::collections::HashMap::new();
        title_map.insert("tr".to_string(), "Stok geri yüklendi".to_string());
        title_map.insert("en".to_string(), "Stock restored".to_string());

        let mut desc_map = std::collections::HashMap::new();
        desc_map.insert(
            "tr".to_string(),
            format!(
                "{} adet {}{} stoğa geri eklendi",
                restored_qty, product_title, variant_info
            ),
        );
        desc_map.insert(
            "en".to_string(),
            format!(
                "{} piece of {}{} restored to stock",
                restored_qty, product_title, variant_info
            ),
        );

        let _ = TimelineService::create_event(
            db,
            CreateTimelineEventRequest {
                module_type: "ecommerce".to_string(),
                content_type: "cart".to_string(),
                content_id: return_req.cart_id,
                event_type:
                    crate::modules::timeline::models::timeline_event::TimelineEventType::Custom(
                        "stock_restored".to_string(),
                    ),
                title: title_map,
                description: Some(desc_map),
                icon: Some("bi-box-seam".to_string()),
                color: Some("#17a2b8".to_string()),
                user_id: None,
                admin_user_id: None,
                metadata: Some(serde_json::json!({
                    "return_id": return_id,
                    "product_id": stock_product_id,
                    "variant_key": stock_variant_key,
                    "restored_quantity": restored_qty,
                    "return_quantity": return_req.quantity,
                })),
                is_public: Some(false),
                is_admin_only: Some(true),
            },
        )
        .await;
    }

    Ok(response)
}

/// İade sonrası cart total'ı güncelle (get_cart ile aynı mantığı kullanır)
async fn update_cart_total_after_return(
    db: &DatabaseConnection,
    cart_id: i64,
) -> Result<(), AdminReturnError> {
    use crate::modules::ecommerce::models::cart::Entity as CartEntity;

    let cart = CartEntity::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;

    match crate::modules::ecommerce::services::cart_service::get_cart(
        db,
        cart_id,
        Some("tr".to_string()),
        None,
        None,
    )
    .await
    {
        Ok(cart_response) => {
            let total_decimal = rust_decimal::Decimal::from_f64_retain(cart_response.final_total)
                .unwrap_or_default();

            let now: DateTimeWithTimeZone = Utc::now().into();
            let mut cart_active: crate::modules::ecommerce::models::cart::ActiveModel = cart.into();
            cart_active.total_amount = Set(Some(total_decimal));
            cart_active.updated_at = Set(Some(now));
            cart_active.update(db).await?;
            Ok(())
        }
        Err(_) => Err(AdminReturnError::DatabaseError(sea_orm::DbErr::Custom(
            "Cart total hesaplama hatası".into(),
        ))),
    }
}

/// Admin: İade talebinin admin notunu güncelle (herhangi bir durumda)
pub async fn update_admin_notes(
    db: &DatabaseConnection,
    return_id: i64,
    admin_notes: String,
) -> Result<AdminReturnResponse, AdminReturnError> {
    let return_req = ReturnRequest::find_by_id(return_id)
        .one(db)
        .await?
        .ok_or(AdminReturnError::NotFound)?;

    let now: DateTimeWithTimeZone = Utc::now().into();

    let mut active: return_request::ActiveModel = return_req.into();
    active.admin_notes = Set(Some(admin_notes));
    active.updated_at = Set(Some(now));

    let updated = active.update(db).await?;

    to_admin_return_response(db, &updated).await
}

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

/// Model → AdminReturnResponse dönüşümü (ürün + kullanıcı + sipariş bilgileri ile)
async fn to_admin_return_response(
    db: &DatabaseConnection,
    r: &return_request::Model,
) -> Result<AdminReturnResponse, AdminReturnError> {
    // Cart item bilgileri
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
                    // Ürün başlığını data.langs içinden al (önce "tr", sonra ilk dili)
                    title = product
                        .data
                        .get("langs")
                        .and_then(|langs| langs.as_object())
                        .and_then(|obj| obj.get("tr").or_else(|| obj.values().next()))
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

        let price = item.original_price.map(|p| p.to_f64().unwrap_or(0.0));
        let currency = item.currency.clone();

        (title, cover, variant, price, currency)
    } else {
        (None, None, None, None, None)
    };

    // Sipariş bilgileri (cart) — önce cart'ı çekiyoruz ki order currency'yi bilelim
    use crate::modules::ecommerce::models::cart::Entity as Cart;
    let cart = Cart::find_by_id(r.cart_id).one(db).await?;

    let (order_id, order_status) = if let Some(ref cart) = cart {
        (cart.order_id.clone(), Some(cart.status.clone()))
    } else {
        (None, None)
    };

    // Fiyatı sipariş para birimine (cart.currency) çevir
    // cart_item.currency ürünün orijinal para birimi, cart.currency siparişin yapıldığı para birimi
    let (item_price, item_currency) = if let (Some(price), Some(ref product_currency)) =
        (item_price, &item_currency)
    {
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

    // Kullanıcı bilgileri
    use crate::modules::auth::models::user::Entity as User;
    let user = User::find_by_id(r.user_id).one(db).await?;

    let (user_email, user_name) = if let Some(ref user) = user {
        let email = Some(user.email.clone());
        let name = {
            let full = format!(
                "{} {}",
                user.first_name.as_deref().unwrap_or(""),
                user.last_name.as_deref().unwrap_or("")
            )
            .trim()
            .to_string();
            if full.is_empty() {
                Some(user.username.clone())
            } else {
                Some(full)
            }
        };
        (email, name)
    } else {
        (None, None)
    };

    Ok(AdminReturnResponse {
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
        refund_amount: r.refund_amount.map(|d| d.to_f64().unwrap_or(0.0)),
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
        user_email,
        user_name,
        order_id,
        order_status,
    })
}

// ─── Timeline + Email Helpers ───

/// İade aşaması için timeline event oluştur
async fn create_return_timeline_event(
    db: &DatabaseConnection,
    return_req: &return_request::Model,
    response: &AdminReturnResponse,
    action: &str, // "approved", "rejected", "received", "completed"
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::modules::timeline::services::{CreateTimelineEventRequest, TimelineService};

    let product_title_base = response.product_title.as_deref().unwrap_or("Ürün");
    let product_title_with_variant = if let Some(ref variant) = response.variant_display {
        if !variant.is_empty() {
            format!("{} ({})", product_title_base, variant)
        } else {
            product_title_base.to_string()
        }
    } else {
        product_title_base.to_string()
    };
    let product_title = product_title_with_variant.as_str();
    let currency = response.item_currency.as_deref().unwrap_or("TRY");
    let unit_price = response.item_price.unwrap_or(0.0);
    let formatted_price = format_price(unit_price * return_req.quantity as f64, currency);

    let (title_tr, title_en, desc_tr, desc_en, icon, color) = match action {
        "approved" => (
            "İade talebi onaylandı",
            "Return request approved",
            format!(
                "{} adet {} ürünü için iade talebi onaylandı",
                return_req.quantity, product_title
            ),
            format!(
                "Return request for {} piece of {} has been approved",
                return_req.quantity, product_title
            ),
            "bi-check-circle",
            "#28a745",
        ),
        "rejected" => (
            "İade talebi reddedildi",
            "Return request rejected",
            format!(
                "{} adet {} ürünü için iade talebi reddedildi{}",
                return_req.quantity,
                product_title,
                response
                    .rejection_reason
                    .as_ref()
                    .map(|r| format!(": {}", r))
                    .unwrap_or_default()
            ),
            format!(
                "Return request for {} piece of {} has been rejected",
                return_req.quantity, product_title
            ),
            "bi-x-circle",
            "#dc3545",
        ),
        "received" => (
            "İade ürünü teslim alındı",
            "Return product received",
            format!(
                "{} adet {} ürünü depoda teslim alındı, incelemeye alınıyor",
                return_req.quantity, product_title
            ),
            format!(
                "{} piece of {} received at warehouse, under inspection",
                return_req.quantity, product_title
            ),
            "bi-box-seam-fill",
            "#17a2b8",
        ),
        "completed" => {
            let refund_info = response
                .refund_amount
                .map(|amt| {
                    let cur = response.refund_currency.as_deref().unwrap_or(currency);
                    format!(" — İade tutarı: {}", format_price(amt, cur))
                })
                .unwrap_or_default();
            (
                "İade tamamlandı",
                "Return completed",
                format!(
                    "{} adet {} ürünü için iade tamamlandı{}",
                    return_req.quantity, product_title, refund_info
                ),
                format!(
                    "Return completed for {} piece of {}",
                    return_req.quantity, product_title
                ),
                "bi-check-circle-fill",
                "#28a745",
            )
        }
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
            content_id: return_req.cart_id,
            event_type: crate::modules::timeline::models::timeline_event::TimelineEventType::Custom(
                format!("return_{}", action),
            ),
            title: title_map,
            description: Some(desc_map),
            icon: Some(icon.to_string()),
            color: Some(color.to_string()),
            user_id: Some(return_req.user_id),
            admin_user_id: None,
            metadata: Some(serde_json::json!({
                "return_id": return_req.id,
                "cart_item_id": return_req.cart_item_id,
                "product_title": product_title,
                "quantity": return_req.quantity,
                "unit_price": unit_price,
                "currency": currency,
                "action": action,
                "refund_amount": response.refund_amount,
                "refund_currency": response.refund_currency,
            })),
            is_public: Some(false),
            is_admin_only: Some(false),
        },
    )
    .await;

    Ok(())
}

/// İade aşaması için müşteriye email bildirimi gönder
async fn send_return_notification(
    db: &DatabaseConnection,
    return_req: &return_request::Model,
    response: &AdminReturnResponse,
    status: &str, // "approved", "rejected", "received", "completed"
) -> Result<(), Box<dyn std::error::Error>> {
    // Kullanıcı bilgileri
    let user_email = match &response.user_email {
        Some(email) => email.clone(),
        None => return Ok(()), // Email yoksa sessizce çık
    };
    let user_name = response
        .user_name
        .as_deref()
        .unwrap_or("Değerli Müşterimiz");
    let order_id = response.order_id.as_deref().unwrap_or("N/A");
    let product_title = response.product_title.as_deref().unwrap_or("Ürün");
    let currency = response.item_currency.as_deref().unwrap_or("TRY");
    let unit_price = response.item_price.unwrap_or(0.0);
    let unit_price_formatted = format_price(unit_price, currency);

    // Refund amount formatted
    let refund_amount_formatted = response.refund_amount.map(|amt| {
        let cur = response.refund_currency.as_deref().unwrap_or(currency);
        format_price(amt, cur)
    });

    // Refund method text
    let refund_method_text = if status == "completed" {
        // Cart'tan payment_method al
        let cart = Cart::find_by_id(return_req.cart_id)
            .one(db)
            .await
            .ok()
            .flatten();
        cart.and_then(|c| {
            c.payment_method.map(|pm| match pm.as_str() {
                "b2b_credit" => "B2B Kredi Hesabı".to_string(),
                "credit_card" => "Kredi Kartı".to_string(),
                "bank_transfer" => "Banka Havale/EFT".to_string(),
                "cash_on_delivery" => "Kapıda Ödeme".to_string(),
                other => other.to_string(),
            })
        })
    } else {
        None
    };

    // Return reason text
    let return_reason = response.reason_text.as_deref().or_else(|| {
        Some(match response.reason.as_str() {
            "defective" => "Ürün kusurlu/arızalı",
            "wrong_product" => "Yanlış ürün gönderildi",
            "not_as_described" => "Ürün açıklamayla uyuşmuyor",
            "changed_mind" => "Fikir değişikliği",
            "size_issue" => "Beden/boyut uyumsuzluğu",
            "damaged_in_shipping" => "Kargoda hasar görmüş",
            "quality_issue" => "Kalite beklentileri karşılanmadı",
            "other" => "Diğer",
            _ => "Belirtilmemiş",
        })
    });

    let config = crate::config::get_config();
    let order_url = format!("{}/my-account/orders", config.get_base_url());

    match crate::modules::mailer::MailHelper::send_return_status_update(
        db,
        &user_email,
        user_name,
        order_id,
        return_req.id,
        status,
        product_title,
        response.variant_display.as_deref(),
        response.product_cover.as_deref(),
        return_req.quantity,
        &unit_price_formatted,
        return_reason,
        response.rejection_reason.as_deref(),
        refund_amount_formatted.as_deref(),
        refund_method_text.as_deref(),
        &order_url,
        "tr",
    )
    .await
    {
        Ok(mail_id) => {
            println!(
                "✅ Return notification mail queued: return_id={}, status={}, mail_id={}",
                return_req.id, status, mail_id
            );
        }
        Err(e) => {
            eprintln!(
                "⚠️ Return notification mail failed: return_id={}, status={}, error={}",
                return_req.id, status, e
            );
        }
    }

    Ok(())
}

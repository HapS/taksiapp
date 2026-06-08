use crate::modules::admin::dto::refund_dto::{
    BulkMarkBankRefundedRequest, BulkRefundToB2BCreditRequest,
};
use crate::modules::b2b::entities::{companies, credit_transactions};
use crate::modules::b2b::services::credit_service;
use crate::modules::currency::services::exchange_rate_service;
use crate::modules::ecommerce::models::cart_item;
use crate::modules::ecommerce::services::stock_restoration_service;
use crate::modules::utils::format_price::format_price;
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::*;

#[derive(Debug)]
pub enum RefundError {
    CartItemNotFound,
    InvalidAmount,
    CompanyNotFound,
    NoPendingItems,
    DatabaseError(DbErr),
    CreditServiceError(String),
}

impl From<DbErr> for RefundError {
    fn from(err: DbErr) -> Self {
        RefundError::DatabaseError(err)
    }
}

impl std::fmt::Display for RefundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefundError::CartItemNotFound => write!(f, "Sepet ürünü bulunamadı"),
            RefundError::InvalidAmount => write!(f, "Geçersiz iade tutarı"),
            RefundError::CompanyNotFound => write!(f, "Şirket bulunamadı"),
            RefundError::NoPendingItems => {
                write!(f, "İade bekleyen iptal edilmiş ürün bulunamadı")
            }
            RefundError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
            RefundError::CreditServiceError(e) => write!(f, "Kredi servisi hatası: {}", e),
        }
    }
}

/// Yardımcı struct: Bekleyen iade item bilgileri (get_cart'tan hesaplanmış fiyatlarla)
#[allow(dead_code)]
struct PendingRefundItem {
    pub id: i64,
    pub product_id: i64,
    pub variant_key: Option<String>,
    pub quantity: i32,
    pub total: f64, // Hesaplanmış toplam (döviz çevirisi dahil, price * quantity)
}

/// Bekleyen iade item'larını get_cart üzerinden al
/// Dönen tuple: (bekleyen_itemlar, bu_sefer_iade_edilecek_net_tutar)
///
/// Kümülatif kargo düşüm mantığı:
/// Tüm iptal edilen ürünlerin brüt toplamından kargo BİR KEZ düşülür,
/// daha önce iade edilmiş tutarlar çıkarılır, kalan = bu sefer iade edilecek net tutar.
/// Bu sayede kargo her zaman doğru düşülür, iade kaç seferde yapılırsa yapılsın.
async fn get_pending_refund_items(
    db: &DatabaseConnection,
    cart_id: i64,
) -> Result<(Vec<PendingRefundItem>, f64), RefundError> {
    // Cart'ı DB'den al (orijinal kargo ücreti ve kargo şirketi için)
    let cart = crate::modules::ecommerce::models::Cart::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(RefundError::CartItemNotFound)?;

    // get_cart ile hesaplanmış fiyatları al (döviz çevirisi dahil)
    let cart_response = crate::modules::ecommerce::services::cart_service::get_cart(
        db,
        cart_id,
        Some("tr".to_string()),
        None,
        None,
    )
    .await
    .map_err(|_| RefundError::DatabaseError(DbErr::Custom("Cart hesaplama hatası".into())))?;

    // İptal edilmiş ve henüz iade yapılmamış item'ları filtrele
    let pending_items: Vec<PendingRefundItem> = cart_response
        .items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_none()
        })
        .map(|item| PendingRefundItem {
            id: item.id,
            product_id: item.product_id,
            variant_key: None,
            quantity: item.quantity,
            total: item.total,
        })
        .collect();

    // Kargo bedava limitini ve ham kargo ücretini snapshot'tan al (eğer yoksa kargo şirketinden/ayarlardan fallback yap)
    let snapshot_threshold = cart.callback_data.as_ref()
        .and_then(|m| m.get("snapshot_free_shipping_threshold"))
        .and_then(|v| v.as_f64());
    
    let snapshot_raw_cargo_fee = cart.callback_data.as_ref()
        .and_then(|m| m.get("snapshot_raw_cargo_fee"))
        .and_then(|v| v.as_f64());

    // Kargo şirketinin standart kargo ücretini al (Snapshot varsa onu kullan, yoksa DB'den bak)
    let standard_cargo_fee: f64 = if let Some(fee) = snapshot_raw_cargo_fee {
        fee
    } else if let Some(company_id) = cart.cargo_company {
        use crate::modules::ecommerce::models::kargo_sirketleri::Entity as KargoEntity;
        KargoEntity::find_by_id(company_id as i32)
            .one(db)
            .await
            .ok()
            .flatten()
            .and_then(|k| k.data.get("standard_cargo_fee").and_then(|v| v.as_f64()))
            .unwrap_or(0.0)
    } else {
        0.0
    };

    // Kargo düşüm mantığı:
    // Sipariş anında kargo ücretliydi (cart.cargo_price > 0) → müşteri zaten ödemiş → düşülmez
    // Sipariş anında kargo ücretsizdi → aktif ürünler threshold altına düştüyse kargo düşülür
    //
    // Kargo düşümü "ileri dönük" yapılır:
    // - Daha önce yapılmış iadelerde kargo düşülmüşse tekrar düşülmez
    // - Daha önce kargo düşülmemişse (o sırada threshold üstündeydi), şimdi threshold altına
    //   düşüldüyse BU iadeden düşülür
    // - "Kargo daha önce düşüldü mü" kontrolü: refunded_gross vs refunded_net farkına bakılır
    //   Eğer fark > 0 ise daha önce kargo düşülmüş demektir

    // Daha önce iade edilen ürünlerin BRÜT toplamı (price * quantity)
    let refunded_gross: f64 = cart_response
        .items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_some()
        })
        .map(|item| item.total)
        .sum();

    // Daha önce iade edilen NET toplam (refund_amount, kargo düşülmüş paylar)
    let refunded_net: f64 = cart_response
        .items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_some()
        })
        .map(|item| item.refund_amount.unwrap_or(0.0))
        .sum();

    // Daha önce kargo düşülmüş mü? (brüt - net farkı > 0 ise evet)
    let cargo_already_deducted = (refunded_gross - refunded_net).abs() > 0.01;

    // Bekleyen iade item'larının brüt toplamı
    let pending_gross: f64 = pending_items.iter().map(|i| i.total).sum();

    // Kalan aktif ürün var mı? (status=None olan)
    let has_active_items = cart_response.items.iter().any(|i| i.status.is_none());

    // Daha önce düşülmüş kargo tutarı (brüt - net fark)
    let previously_deducted_cargo = if cargo_already_deducted {
        (refunded_gross - refunded_net).max(0.0)
    } else {
        0.0
    };

    let cargo_to_deduct = if !has_active_items {
        // Tüm ürünler iptal edilmiş → ortada sipariş yok, kargo düşülmez
        // Müşteriye tam iade yapılır
        // Ayrıca daha önce düşülmüş kargoyu da telafi olarak geri ekle
        0.0
    } else if cart.cargo_price.unwrap_or(0.0) > 0.0 {
        // Sipariş anında kargo ücretliydi, müşteri zaten ödemiş → düşülmez
        0.0
    } else if cargo_already_deducted {
        // Kargo daha önce bir iadede zaten düşüldü → tekrar düşülmez
        0.0
    } else {
        // Kısmi iptal: kargo henüz düşülmedi, aktif ürünler var.
        // Aktif ürünler threshold altına düştüyse kargo düşülür
        let remaining_active_total = cart_response.total;
        let threshold = snapshot_threshold.unwrap_or(cart_response.free_shipping_threshold.unwrap_or(0.0));

        if remaining_active_total >= threshold {
            // Aktif ürünler hâlâ threshold üstünde → kargo ücretsiz kalır
            0.0
        } else {
            // Threshold altına düşüldü → standart kargo düşülür (bu iadeden)
            standard_cargo_fee
        }
    };

    // Kargo telafisi: Tüm ürünler iptal edilmişse, daha önce düşülmüş kargoyu geri ekle
    // Çünkü ortada sipariş kalmadı, kargo düşümü anlamsız
    let cargo_compensation = if !has_active_items && previously_deducted_cargo > 0.0 {
        previously_deducted_cargo
    } else {
        0.0
    };

    // Bu sefer iade edilecek net tutar = bekleyen brüt - kargo düşümü + kargo telafisi
    let current_net_refund = (pending_gross - cargo_to_deduct + cargo_compensation).max(0.0);

    println!(
        "📊 Refund calculation: pending_gross={},
        cargo_to_deduct={}, cargo_compensation={},
        cargo_already_deducted={}, current_net={},
        remaining_active={},
        refunded_gross={},
        refunded_net={}",
        pending_gross,
        cargo_to_deduct,
        cargo_compensation,
        cargo_already_deducted,
        current_net_refund,
        cart_response.total,
        refunded_gross,
        refunded_net
    );

    Ok((pending_items, current_net_refund))
}

/// Net iade tutarını bekleyen item'lara orantılı dağıt
/// Dönen: Vec<(item_id, paylaşılan_tutar)> ve toplam net iade tutarı
fn calculate_refund_shares(
    items: &[PendingRefundItem],
    net_refund_total: f64,
) -> (Vec<(i64, Decimal)>, Decimal) {
    let net_refund_decimal = Decimal::from_f64_retain(net_refund_total).unwrap_or_default();

    if net_refund_total <= 0.0 || items.is_empty() {
        return (vec![], Decimal::ZERO);
    }

    let gross_total: f64 = items.iter().map(|i| i.total).sum();

    if gross_total <= 0.0 {
        return (vec![], Decimal::ZERO);
    }

    // Her item'a orantılı pay dağıt
    let mut shares: Vec<(i64, Decimal)> = Vec::new();
    let mut distributed = Decimal::ZERO;

    for (i, item) in items.iter().enumerate() {
        if i == items.len() - 1 {
            // Son item: kalan tutarı al (yuvarlama farkı burada kalır)
            let remaining = net_refund_decimal - distributed;
            shares.push((item.id, remaining));
        } else {
            // Orantılı pay: (item.total / gross_total) * net_refund
            let ratio = item.total / gross_total;
            let share = Decimal::from_f64_retain(net_refund_total * ratio).unwrap_or_default();
            // 2 ondalık basamağa yuvarla
            let share = share.round_dp(2);
            shares.push((item.id, share));
            distributed += share;
        }
    }

    (shares, net_refund_decimal)
}

/// Toplu B2B kredi iadesi
/// Tüm iptal edilmiş ve iade yapılmamış ürünler için TEK bir kredi işlemi oluşturur.
/// Kargo ücreti toplam iade tutarından düşülür.
pub async fn bulk_refund_to_b2b_credit(
    db: &DatabaseConnection,
    request: BulkRefundToB2BCreditRequest,
    _admin_user_id: i64,
) -> Result<credit_transactions::Model, RefundError> {
    // Cart'ı bul
    let cart = crate::modules::ecommerce::models::Cart::find_by_id(request.cart_id)
        .one(db)
        .await?
        .ok_or(RefundError::CartItemNotFound)?;

    // B2B cart olmalı
    if cart.cart_type != "b2b" {
        return Err(RefundError::CompanyNotFound);
    }

    // User'ın company'sini bul
    let company = crate::modules::b2b::entities::companies::Entity::find()
        .filter(crate::modules::b2b::entities::companies::Column::UserId.eq(cart.user_id))
        .one(db)
        .await?
        .ok_or(RefundError::CompanyNotFound)?;

    let company_id = company.id;

    // Şirketi kontrol et
    companies::Entity::find_by_id(company_id)
        .one(db)
        .await?
        .ok_or(RefundError::CompanyNotFound)?;

    // Bekleyen iade item'larını ve bu sefer iade edilecek net tutarı al
    let (pending_items, current_net_refund) = get_pending_refund_items(db, request.cart_id).await?;

    if pending_items.is_empty() {
        return Err(RefundError::NoPendingItems);
    }

    // Net tutarı item'lara orantılı dağıt
    let (shares, net_refund_total) = calculate_refund_shares(&pending_items, current_net_refund);

    if net_refund_total <= Decimal::ZERO {
        return Err(RefundError::InvalidAmount);
    }

    // İade tutarını company.currency'e çevir (B2B işlemleri her zaman şirket para biriminde kaydedilir)
    let company_currency = company
        .currency
        .clone()
        .unwrap_or_else(|| "TRY".to_string());
    let cart_currency_str = cart.currency.clone().unwrap_or_else(|| "TRY".to_string());

    let (refund_in_company_currency, refund_currency) = if cart_currency_str == company_currency {
        (net_refund_total, company_currency.clone())
    } else {
        let rates = exchange_rate_service::get_cached_rates(db).await;
        let converted = if let Some(rates) = rates {
            let amount_f64 = net_refund_total.to_string().parse::<f64>().unwrap_or(0.0);
            exchange_rate_service::convert_currency(
                amount_f64,
                &cart_currency_str,
                &company_currency,
                &rates,
            )
            .and_then(|v| Decimal::from_f64_retain(v))
            .unwrap_or(net_refund_total)
        } else {
            net_refund_total
        };
        (converted, company_currency.clone())
    };

    let order_id_str = cart
        .order_id
        .clone()
        .unwrap_or_else(|| request.cart_id.to_string());

    // TEK bir kredi işlemi oluştur (toplam net iade tutarı ile)
    let description = request.description.unwrap_or_else(|| {
        format!(
            "Sipariş #{} - {} ürün toplu iade (kargo düşülmüş)",
            order_id_str,
            pending_items.len()
        )
    });

    let credit_transaction = credit_service::create_refund_transaction(
        db,
        company_id,
        Some(request.cart_id),
        refund_in_company_currency,
        refund_currency.clone(),
        Some(description),
    )
    .await
    .map_err(|e| RefundError::CreditServiceError(e.to_string()))?;

    // Her item'ı payı ile birlikte refunded olarak işaretle
    for (item_id, share_amount) in &shares {
        // DB'den cart_item'ı al (stok iadesi için product_id ve variant_key lazım)
        if let Ok(Some(db_item)) = cart_item::Entity::find_by_id(*item_id).one(db).await {
            let product_id = db_item.product_id;
            let variant_key_owned = db_item.variant_key.clone();
            let quantity = db_item.quantity;

            let mut active: cart_item::ActiveModel = db_item.into();
            active.refund_status = Set(Some("credited_b2b".to_string()));
            active.refund_amount = Set(Some(*share_amount));
            active.refund_date = Set(Some(Utc::now().into()));
            active.refund_method = Set(Some("b2b_credit".to_string()));
            active.refund_currency = Set(Some(refund_currency.clone()));
            let _ = active.update(db).await;

            // Stok iadesi (non-blocking)
            match stock_restoration_service::restore_stock(
                db,
                product_id,
                variant_key_owned.as_deref(),
                quantity,
            )
            .await
            {
                Ok(_) => {
                    println!(
                        "✅ Stock restored for bulk B2B refund: cart_item_id={}, product_id={}, variant_key={:?}, quantity={}",
                        item_id, product_id, variant_key_owned, quantity
                    );
                }
                Err(e) => {
                    eprintln!(
                        "⚠️ Stock restoration failed for bulk B2B refund (non-blocking): cart_item_id={}, error={:?}",
                        item_id, e
                    );
                }
            }
        }
    }

    println!(
        "✅ Bulk B2B refund completed: cart_id={}, items={}, net={} {} (original cart currency: {})",
        request.cart_id,
        shares.len(),
        refund_in_company_currency,
        refund_currency,
        cart_currency_str
    );

    // Timeline event: Toplu B2B kredi iadesi
    let _ = create_refund_timeline_event(
        db,
        request.cart_id,
        cart.user_id,
        "b2b_credit",
        pending_items.len(),
        net_refund_total,
        &refund_currency,
    )
    .await;

    Ok(credit_transaction)
}

/// Toplu banka iadesi
/// Tüm iptal edilmiş ve iade yapılmamış ürünler için toplu banka iadesi işaretler.
/// Kargo ücreti toplam iade tutarından düşülür.
pub async fn bulk_mark_bank_refunded(
    db: &DatabaseConnection,
    request: BulkMarkBankRefundedRequest,
    _admin_user_id: i64,
) -> Result<BulkBankRefundResult, RefundError> {
    // Cart'ı bul (kargo bilgisi için)
    let cart = crate::modules::ecommerce::models::Cart::find_by_id(request.cart_id)
        .one(db)
        .await?
        .ok_or(RefundError::CartItemNotFound)?;

    // Bekleyen iade item'larını ve bu sefer iade edilecek net tutarı al
    let (pending_items, current_net_refund) = get_pending_refund_items(db, request.cart_id).await?;

    if pending_items.is_empty() {
        return Err(RefundError::NoPendingItems);
    }

    // Net tutarı item'lara orantılı dağıt
    let (shares, net_refund_total) = calculate_refund_shares(&pending_items, current_net_refund);

    if net_refund_total <= Decimal::ZERO {
        return Err(RefundError::InvalidAmount);
    }

    // İade para birimi: Siparişin para birimi (sale_currency at order time)
    let refund_currency = cart.currency.clone().unwrap_or_else(|| "TRY".to_string());

    let mut updated_count = 0usize;

    // Her item'ı payı ile birlikte bank_refunded olarak işaretle
    for (item_id, share_amount) in &shares {
        if let Ok(Some(db_item)) = cart_item::Entity::find_by_id(*item_id).one(db).await {
            let product_id = db_item.product_id;
            let variant_key_owned = db_item.variant_key.clone();
            let quantity = db_item.quantity;

            let mut active: cart_item::ActiveModel = db_item.into();
            active.refund_status = Set(Some("bank_refunded".to_string()));
            active.refund_amount = Set(Some(*share_amount));
            active.refund_date = Set(Some(Utc::now().into()));
            active.refund_method = Set(Some(request.payment_method.clone()));
            active.refund_currency = Set(Some(refund_currency.clone()));
            let _ = active.update(db).await;
            updated_count += 1;

            // Stok iadesi (non-blocking)
            match stock_restoration_service::restore_stock(
                db,
                product_id,
                variant_key_owned.as_deref(),
                quantity,
            )
            .await
            {
                Ok(_) => {
                    println!(
                        "✅ Stock restored for bulk bank refund: cart_item_id={}, product_id={}, variant_key={:?}, quantity={}",
                        item_id, product_id, variant_key_owned, quantity
                    );
                }
                Err(e) => {
                    eprintln!(
                        "⚠️ Stock restoration failed for bulk bank refund (non-blocking): cart_item_id={}, error={:?}",
                        item_id, e
                    );
                }
            }
        }
    }

    let order_id_str = cart.order_id.unwrap_or_else(|| request.cart_id.to_string());

    println!(
        "✅ Bulk bank refund completed: cart_id={}, order={}, items={}, net={} {}",
        request.cart_id, order_id_str, updated_count, net_refund_total, refund_currency
    );

    // Timeline event: Toplu banka iadesi
    let _ = create_refund_timeline_event(
        db,
        request.cart_id,
        cart.user_id,
        "bank",
        pending_items.len(),
        net_refund_total,
        &refund_currency,
    )
    .await;

    let pending_gross: f64 = pending_items.iter().map(|i| i.total).sum();
    let cargo_deducted =
        Decimal::from_f64_retain(pending_gross).unwrap_or_default() - net_refund_total;

    Ok(BulkBankRefundResult {
        refunded_count: updated_count,
        net_refund_total,
        cargo_deducted: if cargo_deducted > Decimal::ZERO {
            cargo_deducted
        } else {
            Decimal::ZERO
        },
        refund_currency,
    })
}

/// Toplu banka iadesi sonuç bilgisi
#[derive(Debug, serde::Serialize)]
pub struct BulkBankRefundResult {
    pub refunded_count: usize,
    pub net_refund_total: Decimal,
    pub cargo_deducted: Decimal,
    pub refund_currency: String,
}

/// İade işlemi (iptal sonrası toplu iade) için timeline event oluştur
async fn create_refund_timeline_event(
    db: &DatabaseConnection,
    cart_id: i64,
    user_id: i64,
    method: &str, // "b2b_credit", "bank"
    item_count: usize,
    net_amount: Decimal,
    currency: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::modules::timeline::services::{CreateTimelineEventRequest, TimelineService};
    use rust_decimal::prelude::ToPrimitive;

    let amount_f64 = net_amount.to_f64().unwrap_or(0.0);
    let formatted_amount = format_price(amount_f64, currency);

    let (title_tr, title_en, desc_tr, desc_en, icon, color) = match method {
        "b2b_credit" => (
            "B2B kredi iadesi yapıldı",
            "B2B credit refund processed",
            format!(
                "{} ürün için {} tutarında B2B kredi iadesi yapıldı",
                item_count, formatted_amount
            ),
            format!(
                "B2B credit refund of {} processed for {} item(s)",
                formatted_amount, item_count
            ),
            "bi-building-fill-check",
            "#28a745",
        ),
        "bank" => (
            "Banka iadesi yapıldı",
            "Bank refund processed",
            format!(
                "{} ürün için {} tutarında banka iadesi yapıldı",
                item_count, formatted_amount
            ),
            format!(
                "Bank refund of {} processed for {} item(s)",
                formatted_amount, item_count
            ),
            "bi-bank2",
            "#17a2b8",
        ),
        _ => return Ok(()),
    };

    let mut title_map = std::collections::HashMap::new();
    title_map.insert("tr".to_string(), title_tr.to_string());
    title_map.insert("en".to_string(), title_en.to_string());

    let mut desc_map = std::collections::HashMap::new();
    desc_map.insert("tr".to_string(), desc_tr);
    desc_map.insert("en".to_string(), desc_en);

    let _ = TimelineService::create_event(
        db,
        CreateTimelineEventRequest {
            module_type: "ecommerce".to_string(),
            content_type: "cart".to_string(),
            content_id: cart_id,
            event_type: crate::modules::timeline::models::timeline_event::TimelineEventType::Custom(
                format!("refund_{}", method),
            ),
            title: title_map,
            description: Some(desc_map),
            icon: Some(icon.to_string()),
            color: Some(color.to_string()),
            user_id: Some(user_id),
            admin_user_id: None,
            metadata: Some(serde_json::json!({
                "method": method,
                "item_count": item_count,
                "net_amount": amount_f64,
                "currency": currency,
            })),
            is_public: Some(false),
            is_admin_only: Some(false),
        },
    )
    .await;

    Ok(())
}

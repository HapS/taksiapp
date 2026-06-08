use crate::modules::auth::models::user::Entity as User;
use crate::modules::b2b::dto::company_dto::CompanyResponse;
use crate::modules::b2b::services::company_service::CompanyService;
use crate::modules::ecommerce::models::kargo_sirketleri::Entity as KargoSirketleri;
use crate::modules::ecommerce::models::{cart, Cart};
use crate::modules::utils::format_price::format_price;
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::entity::prelude::DateTimeWithTimeZone;
use sea_orm::*;
use serde::Serialize;

#[derive(Debug)]
pub enum AdminServiceError {
    NotFound,
    #[allow(dead_code)]
    DatabaseError(DbErr),
}

impl From<DbErr> for AdminServiceError {
    fn from(err: DbErr) -> Self {
        AdminServiceError::DatabaseError(err)
    }
}

/// Admin için sipariş response
#[derive(Debug, Serialize)]
pub struct AdminOrderResponse {
    pub id: i64,
    pub order_id: Option<String>,
    pub user_id: i64,
    pub user_info: AdminUserInfo,
    pub status: String,
    pub payment_method: Option<String>,
    pub total: Option<f64>,              // Ürünler toplamı (kargo hariç)
    pub total_formatted: Option<String>, // Formatlı ürünler toplamı
    pub total_amount: Option<rust_decimal::Decimal>,
    pub total_amount_formatted: Option<String>, // Formatlı toplam tutar (örneğin "100,00 TL")
    pub currency: Option<String>,               // Sipariş para birimi (sale_currency)
    pub cart_type: String,                      // b2b veya b2c
    pub item_count: i32,
    pub items: Option<Vec<AdminOrderItem>>,
    pub address_line: Option<String>,
    pub invoice_address_line: Option<String>,
    pub notes: Option<String>, // Müşteri notu
    pub cargo_company: Option<i64>,
    pub cargo_company_name: Option<String>,
    pub cargo_company_logo: Option<String>,
    pub cargo_tracking_no: Option<String>,
    pub cargo_price: Option<f64>,                 // Kargo ücreti
    pub cargo_price_formatted: Option<String>,    // Formatlı kargo ücreti
    pub is_free_shipping: bool,                   // Bedava kargo mu
    pub admin_notes: Option<String>,              // Admin notu
    pub refund_total: Option<f64>, // İade bekleyen ürünlerin toplam tutarı (kargo düşülmüş, henüz iade yapılmamış)
    pub refund_total_formatted: Option<String>, // Formatlı iade bekleyen tutarı
    pub refund_item_count: Option<i32>, // İade bekleyen ürün sayısı
    pub refunded_total: Option<f64>, // Gerçekleşen iade toplam tutarı
    pub refunded_total_formatted: Option<String>, // Formatlı gerçekleşen iade tutarı
    pub refunded_item_count: Option<i32>, // İadesi gerçekleşen ürün sayısı
    pub order_date: Option<DateTimeWithTimeZone>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
    pub campaign_summary: Option<crate::modules::ecommerce::campaign::engine::CartSummary>,
}

#[derive(Debug, Serialize)]
pub struct AdminUserInfo {
    pub id: i64,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: String,
    pub phone_number: Option<String>,
    pub phone_country_code: Option<String>,
    pub is_guest: bool,
    pub profile: Option<serde_json::Value>,
    pub company: Option<CompanyResponse>,
}

// Cart service'deki CartItemResponse'u kullanacağız
pub use crate::modules::ecommerce::services::cart_service::CartItemResponse as AdminOrderItem;

/// Admin sipariş listesi (filtreleme ve pagination ile)
pub async fn get_admin_orders(
    db: &DatabaseConnection,
    status: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    search: Option<String>,
    page: u64,
    per_page: u64,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<AdminOrderResponse>, u64), AdminServiceError> {
    let offset = (page - 1) * per_page;

    let mut select = Cart::find().filter(
        cart::Column::Status.ne(crate::modules::ecommerce::models::cart::status::OPEN_CART),
    );

    // Status filtresi
    if let Some(status) = status {
        if !status.is_empty() {
            select = select.filter(cart::Column::Status.eq(status));
        }
    }

    // Tarih filtresi
    if let Some(start_date) = start_date {
        if !start_date.is_empty() {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d") {
                let start_datetime = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
                select = select.filter(cart::Column::OrderDate.gte(start_datetime));
            }
        }
    }

    if let Some(end_date) = end_date {
        if !end_date.is_empty() {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&end_date, "%Y-%m-%d") {
                let end_datetime = date.and_hms_opt(23, 59, 59).unwrap().and_utc();
                select = select.filter(cart::Column::OrderDate.lte(end_datetime));
            }
        }
    }

    // Arama filtresi (order_id'de arama - case insensitive)
    if let Some(search) = search {
        if !search.is_empty() {
            let search_pattern = format!("%{}%", search);
            select = select.filter(
                cart::Column::OrderId
                    .is_not_null()
                    .and(cart::Column::OrderId.contains(&search_pattern)),
            );
        }
    }

    // Total count al (pagination için)
    let total = select.clone().count(db).await?;

    // Sıralama desteği
    let mut sort_applied = false;
    if let Some(sb) = sort_by {
        if !sb.is_empty() {
            // normalize order
            let order = sort_order
                .unwrap_or_else(|| "desc".to_string())
                .to_lowercase();
            match sb.as_str() {
                "id" => {
                    if order == "asc" {
                        select = select.order_by_asc(cart::Column::Id);
                    } else {
                        select = select.order_by_desc(cart::Column::Id);
                    }
                    sort_applied = true;
                }
                "order_id" => {
                    if order == "asc" {
                        select = select.order_by_asc(cart::Column::OrderId);
                    } else {
                        select = select.order_by_desc(cart::Column::OrderId);
                    }
                    sort_applied = true;
                }
                "total_amount" | "total" => {
                    if order == "asc" {
                        select = select.order_by_asc(cart::Column::TotalAmount);
                    } else {
                        select = select.order_by_desc(cart::Column::TotalAmount);
                    }
                    sort_applied = true;
                }
                "payment_method" | "payment" => {
                    if order == "asc" {
                        select = select.order_by_asc(cart::Column::PaymentMethod);
                    } else {
                        select = select.order_by_desc(cart::Column::PaymentMethod);
                    }
                    sort_applied = true;
                }
                "status" => {
                    if order == "asc" {
                        select = select.order_by_asc(cart::Column::Status);
                    } else {
                        select = select.order_by_desc(cart::Column::Status);
                    }
                    sort_applied = true;
                }
                "order_date" | "created_at" | "date" => {
                    if order == "asc" {
                        select = select.order_by_asc(cart::Column::OrderDate);
                    } else {
                        select = select.order_by_desc(cart::Column::OrderDate);
                    }
                    sort_applied = true;
                }
                _ => {
                    // Unknown sort field - ignore and fall back to default below
                }
            }
        }
    }
    // Eğer sıralama uygulanmadıysa varsayılan olarak tarih (azalan) kullan
    if !sort_applied {
        select = select.order_by_desc(cart::Column::OrderDate);
    }

    // Siparişleri çek
    let carts = select.offset(offset).limit(per_page).all(db).await?;

    // User ID'lerini topla
    let user_ids: Vec<i64> = carts.iter().map(|cart| cart.user_id).collect();

    // Kullanıcıları tek sorguda çek (N+1 problemi önlenir)
    let users = User::find()
        .filter(crate::modules::auth::models::user::Column::Id.is_in(user_ids))
        .all(db)
        .await?;

    // User map oluştur
    let user_map: std::collections::HashMap<i64, _> =
        users.into_iter().map(|user| (user.id, user)).collect();

    // Cart item count'ları çek
    let cart_ids: Vec<i64> = carts.iter().map(|cart| cart.id).collect();
    let item_counts = get_cart_item_counts(db, &cart_ids).await?;

    // Response'ları oluştur
    let mut order_responses = Vec::new();
    for cart in carts {
        if let Some(user) = user_map.get(&cart.user_id) {
            let item_count = item_counts.get(&cart.id).unwrap_or(&0);
            let currency = cart.currency.clone().unwrap_or_else(|| "TRY".to_string());

            order_responses.push(AdminOrderResponse {
                id: cart.id,
                order_id: cart.order_id,
                user_id: cart.user_id,
                user_info: AdminUserInfo {
                    id: user.id,
                    username: user.username.clone(),
                    first_name: user.first_name.clone(),
                    last_name: user.last_name.clone(),
                    email: user.email.clone(),
                    phone_number: user.phone_number.clone(),
                    phone_country_code: user.phone_country_code.clone(),
                    is_guest: user.is_guest,
                    profile: user.profile.clone(),
                    company: if cart.cart_type == "b2b" {
                        CompanyService::get_company_by_user_id(db, user.id)
                            .await
                            .ok()
                            .flatten()
                    } else {
                        None
                    },
                },
                status: cart.status,
                payment_method: cart.payment_method,
                total: None, // Listede gösterilmiyor
                total_formatted: None,
                total_amount: cart.total_amount,
                total_amount_formatted: cart
                    .total_amount
                    .as_ref()
                    .map(|amount| format_price(amount.to_f64().unwrap_or(0.0), &currency)),
                currency: Some(currency.clone()),
                cart_type: cart.cart_type,
                item_count: *item_count,
                items: None, // List için items gerekmiyor
                address_line: cart.address_line,
                invoice_address_line: cart.invoice_address_line,
                notes: cart.notes,
                cargo_company: cart.cargo_company,
                cargo_company_name: None,
                cargo_company_logo: None,
                cargo_tracking_no: cart.cargo_tracking_no,
                cargo_price: cart.cargo_price,
                cargo_price_formatted: cart.cargo_price.map(|price| format_price(price, &currency)),
                is_free_shipping: false, // Liste için hesaplanmadı
                admin_notes: cart.admin_notes,
                refund_total: None,
                refund_total_formatted: None,
                refund_item_count: None,
                refunded_total: None,
                refunded_total_formatted: None,
                refunded_item_count: None,
                order_date: cart.order_date,
                created_at: cart.created_at,
                updated_at: cart.updated_at,
                campaign_summary: None,
            });
        }
    }

    Ok((order_responses, total))
}

// #[derive(Debug, Clone, Serialize)]
// pub struct CargoCompanyAdmin {
//     pub id: i32,
//     pub title: String,
//     pub logo: String,
// }

/// Admin sipariş detayı
pub async fn get_admin_order(
    db: &DatabaseConnection,
    order_id: i64,
) -> Result<AdminOrderResponse, AdminServiceError> {
    let cart = Cart::find_by_id(order_id)
        .one(db)
        .await?
        .ok_or(AdminServiceError::NotFound)?;

    let kargo = if let Some(company_id) = cart.cargo_company {
        KargoSirketleri::find_by_id(company_id as i32)
            .one(db)
            .await?
    } else {
        None
    };

    let kargo_sirketi_title = kargo.clone().map(|company| company.title);
    let kargo_sirketi_logo = kargo.clone().map(|company| company.logo);

    // Kullanıcı bilgilerini çek
    let user = User::find_by_id(cart.user_id)
        .one(db)
        .await?
        .ok_or(AdminServiceError::NotFound)?;

    // Item count çek (iptal edilmişler hariç)
    let item_count = get_cart_item_count(db, cart.id).await?;

    // Cart items'ları çek
    let cart_items = get_admin_cart_items(db, cart.id).await?;

    // get_cart fonksiyonunu kullanarak final_total'ı al
    // Bu, kullanıcının gördüğü ile aynı olacak (kargo dahil)
    let cart_response = crate::modules::ecommerce::services::cart_service::get_cart(
        db,
        cart.id,
        Some("tr".to_string()),
        None, // Admin panelde B2C fiyatları
        None,
    )
    .await
    .map_err(|_| AdminServiceError::DatabaseError(DbErr::Custom("Hesaplama hatası".into())))?;

    let calculated_total_decimal =
        rust_decimal::Decimal::from_f64_retain(cart_response.final_total).unwrap_or_default();

    let currency = cart.currency.clone().unwrap_or_else(|| "TRY".to_string());

    // İade BEKLEYEN ürünlerin toplam tutarını hesapla (iptal edilmiş ama henüz iade yapılmamış)
    let pending_refund_total: f64 = cart_items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_none()
        })
        .map(|item| item.price * item.quantity as f64)
        .sum();

    // İade bekleyen ürün sayısı
    let pending_refund_item_count: i32 = cart_items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_none()
        })
        .map(|item| item.quantity)
        .sum();

    // Gerçekleşen iadelerin toplam tutarını hesapla (refund_status dolu olan) - NET tutar (kargo düşülmüş paylar)
    let refunded_total: f64 = cart_items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_some()
        })
        .map(|item| item.refund_amount.unwrap_or(0.0))
        .sum();

    // Daha önce iade edilen ürünlerin BRÜT toplamı (price * quantity, kargo düşümü öncesi)
    let refunded_gross: f64 = cart_items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_some()
        })
        .map(|item| item.price * item.quantity as f64)
        .sum();

    // İadesi gerçekleşen ürün sayısı
    let refunded_item_count: i32 = cart_items
        .iter()
        .filter(|item| {
            item.status.as_deref() == Some("cancel_accept") && item.refund_status.is_some()
        })
        .map(|item| item.quantity)
        .sum();

    Ok(AdminOrderResponse {
        id: cart.id,
        order_id: cart.order_id,
        user_id: cart.user_id,
        user_info: AdminUserInfo {
            id: user.id,
            username: user.username,
            first_name: user.first_name,
            last_name: user.last_name,
            email: user.email,
            phone_number: user.phone_number,
            phone_country_code: user.phone_country_code,
            is_guest: user.is_guest,
            profile: user.profile,
            company: if cart.cart_type == "b2b" {
                CompanyService::get_company_by_user_id(db, user.id)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            },
        },
        status: cart.status,
        payment_method: cart.payment_method,
        total: Some(cart_response.total), // Ürünler toplamı
        total_formatted: Some(format_price(cart_response.total, &currency)),
        total_amount: Some(calculated_total_decimal),
        total_amount_formatted: Some(format_price(cart_response.final_total, &currency)),
        currency: Some(currency.clone()),
        cart_type: cart.cart_type,
        item_count,
        items: Some(cart_items),
        address_line: cart.address_line,
        invoice_address_line: cart.invoice_address_line,
        notes: cart.notes,
        cargo_company: cart.cargo_company,
        cargo_company_name: kargo_sirketi_title,
        cargo_company_logo: kargo_sirketi_logo,
        cargo_tracking_no: cart.cargo_tracking_no,
        // Kargo gösterimi: get_cart artık doğru hesaplıyor
        // total >= threshold → ücretsiz, total < threshold → standart ücret
        cargo_price: cart_response.standart_cargo_fee,
        cargo_price_formatted: cart_response.standart_cargo_fee_formatted.clone(),
        is_free_shipping: cart_response.is_free_shipping,
        admin_notes: cart.admin_notes,
        refund_total: if pending_refund_total > 0.0 {
            // Kargo düşüm mantığı (refund_service ile aynı):
            // - Tüm ürünler iptal edilmişse → sipariş yok, kargo düşülmez + daha önce düşülen kargo telafi edilir
            // - Sipariş anında kargo ücretliydi → düşülmez
            // - Kargo daha önce bir iadede düşüldüyse → tekrar düşülmez
            // - Aktif ürünler hâlâ threshold üstündeyse → düşülmez
            // - Aksi halde (kısmi iptal, threshold altı) → bu iadeden standart kargo düşülür
            let has_active_items = cart_response.items.iter().any(|i| i.status.is_none());
            let cargo_already_deducted = (refunded_gross - refunded_total).abs() > 0.01;
            let previously_deducted_cargo = if cargo_already_deducted {
                (refunded_gross - refunded_total).max(0.0)
            } else {
                0.0
            };

            let cargo_to_deduct = if !has_active_items {
                0.0
            } else if cart.cargo_price.unwrap_or(0.0) > 0.0 {
                0.0
            } else if cargo_already_deducted {
                0.0
            } else {
                let remaining_active = cart_response.total;
                let threshold = cart_response.free_shipping_threshold.unwrap_or(0.0);
                if remaining_active >= threshold {
                    0.0
                } else {
                    cart_response.standart_cargo_fee.unwrap_or(0.0)
                }
            };

            // Kargo telafisi: tüm ürünler iptal → daha önce düşülen kargo geri eklenir
            let cargo_compensation = if !has_active_items && previously_deducted_cargo > 0.0 {
                previously_deducted_cargo
            } else {
                0.0
            };

            let pending_net =
                (pending_refund_total - cargo_to_deduct + cargo_compensation).max(0.0);
            Some(pending_net)
        } else {
            None
        },
        refund_total_formatted: if pending_refund_total > 0.0 {
            let has_active_items = cart_response.items.iter().any(|i| i.status.is_none());
            let cargo_already_deducted = (refunded_gross - refunded_total).abs() > 0.01;
            let previously_deducted_cargo = if cargo_already_deducted {
                (refunded_gross - refunded_total).max(0.0)
            } else {
                0.0
            };

            let cargo_to_deduct = if !has_active_items {
                0.0
            } else if cart.cargo_price.unwrap_or(0.0) > 0.0 {
                0.0
            } else if cargo_already_deducted {
                0.0
            } else {
                let remaining_active = cart_response.total;
                let threshold = cart_response.free_shipping_threshold.unwrap_or(0.0);
                if remaining_active >= threshold {
                    0.0
                } else {
                    cart_response.standart_cargo_fee.unwrap_or(0.0)
                }
            };

            let cargo_compensation = if !has_active_items && previously_deducted_cargo > 0.0 {
                previously_deducted_cargo
            } else {
                0.0
            };

            let pending_net =
                (pending_refund_total - cargo_to_deduct + cargo_compensation).max(0.0);
            Some(format_price(pending_net, &currency))
        } else {
            None
        },
        refund_item_count: if pending_refund_item_count > 0 {
            Some(pending_refund_item_count)
        } else {
            None
        },
        refunded_total: if refunded_total > 0.0 {
            Some(refunded_total)
        } else {
            None
        },
        refunded_total_formatted: if refunded_total > 0.0 {
            Some(format_price(refunded_total, &currency))
        } else {
            None
        },
        refunded_item_count: if refunded_item_count > 0 {
            Some(refunded_item_count)
        } else {
            None
        },
        order_date: cart.order_date,
        created_at: cart.created_at,
        updated_at: cart.updated_at,
        campaign_summary: cart_response.campaign_summary,
    })
}

/// Sipariş durumu güncelle (admin)
pub async fn update_order_status(
    db: &DatabaseConnection,
    order_id: i64,
    new_status: String,
    admin_notes: Option<String>,
    cargo_company: Option<i64>,
    cargo_tracking_no: Option<String>,
    admin_user_id: Option<i64>,
) -> Result<AdminOrderResponse, AdminServiceError> {
    let cart = Cart::find_by_id(order_id)
        .one(db)
        .await?
        .ok_or(AdminServiceError::NotFound)?;

    // Status ve diğer alanları güncelle
    let mut cart_active: cart::ActiveModel = cart.into();
    cart_active.status = Set(new_status.clone());
    cart_active.updated_at = Set(Some(chrono::Utc::now().into()));

    // Admin notları varsa güncelle
    if let Some(notes) = admin_notes.clone() {
        if !notes.trim().is_empty() {
            cart_active.admin_notes = Set(Some(notes));
        }
    }

    // Kargo bilgileri varsa güncelle
    if let Some(company) = cargo_company.clone() {
        cart_active.cargo_company = Set(Some(company));
    }

    if let Some(tracking) = cargo_tracking_no.clone() {
        if !tracking.trim().is_empty() {
            cart_active.cargo_tracking_no = Set(Some(tracking));
        }
    }

    let updated_cart = cart_active.update(db).await?;

    // Cart iptal edildiğinde tüm itemları da iptal et
    if new_status == crate::modules::ecommerce::models::cart::status::CANCELLED {
        accept_cart_cancellation(db, order_id).await?;
    }

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
    let mut tr_desc = format!(
        "Sipariş durumu {} olarak güncellendi",
        status_message.to_lowercase()
    );
    let mut en_desc = format!("Order status updated to {}", new_status);

    // Admin notu varsa description'a ekle
    if let Some(ref notes) = admin_notes {
        if !notes.trim().is_empty() {
            tr_desc.push_str(&format!("\n\nAdmin Notu: {}", notes));
            en_desc.push_str(&format!("\n\nAdmin Note: {}", notes));
        }
    }

    // Kargo bilgileri varsa ekle — ID yerine kargo şirketi adını çöz
    if let Some(ref company_id) = cargo_company {
        let cargo_name = {
            use crate::modules::ecommerce::models::kargo_sirketleri::Entity as KargoEntity;
            KargoEntity::find_by_id(*company_id as i32)
                .one(db)
                .await
                .ok()
                .flatten()
                .map(|k| k.title)
                .unwrap_or_else(|| format!("Kargo #{}", company_id))
        };
        tr_desc.push_str(&format!("\nKargo Şirketi: {}", cargo_name));
        en_desc.push_str(&format!("\nCargo Company: {}", cargo_name));
    }

    if let Some(ref tracking) = cargo_tracking_no {
        if !tracking.trim().is_empty() {
            tr_desc.push_str(&format!("\nTakip No: {}", tracking));
            en_desc.push_str(&format!("\nTracking No: {}", tracking));
        }
    }

    desc_map.insert("tr".to_string(), tr_desc);
    desc_map.insert("en".to_string(), en_desc);

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
                "new_status": new_status,
                "changed_by": admin_user_id
            })),
            is_public: Some(false),
            is_admin_only: Some(false),
        },
    )
    .await;

    get_admin_order(db, updated_cart.id).await
}

/// Cart item count'ları toplu olarak çek
async fn get_cart_item_counts(
    db: &DatabaseConnection,
    cart_ids: &[i64],
) -> Result<std::collections::HashMap<i64, i32>, AdminServiceError> {
    use crate::modules::ecommerce::models::cart_item::{
        Column as CartItemColumn, Entity as CartItem,
    };

    let counts = CartItem::find()
        .select_only()
        .column(CartItemColumn::CartId)
        .column_as(CartItemColumn::Quantity.sum(), "total_quantity")
        .filter(CartItemColumn::CartId.is_in(cart_ids.iter().cloned()))
        .group_by(CartItemColumn::CartId)
        .into_tuple::<(i64, Option<i64>)>()
        .all(db)
        .await?;

    let mut result = std::collections::HashMap::new();
    for (cart_id, quantity) in counts {
        result.insert(cart_id, quantity.unwrap_or(0) as i32);
    }

    Ok(result)
}

/// Tek cart için item count
async fn get_cart_item_count(
    db: &DatabaseConnection,
    cart_id: i64,
) -> Result<i32, AdminServiceError> {
    use crate::modules::ecommerce::models::cart_item::{
        Column as CartItemColumn, Entity as CartItem,
    };

    let count = CartItem::find()
        .select_only()
        .column_as(CartItemColumn::Quantity.sum(), "total_quantity")
        .filter(CartItemColumn::CartId.eq(cart_id))
        .into_tuple::<Option<i64>>()
        .one(db)
        .await?
        .flatten()
        .unwrap_or(0) as i32;

    Ok(count)
}

/// Admin için cart items'ları çek (cart service kullanarak)
async fn get_admin_cart_items(
    db: &DatabaseConnection,
    cart_id: i64,
) -> Result<Vec<AdminOrderItem>, AdminServiceError> {
    // Cart service'i kullanarak items'ları çek (admin için B2C fiyatları)
    match crate::modules::ecommerce::services::cart_service::get_cart(
        db,
        cart_id,
        Some("tr".to_string()),
        None, // Admin panelde B2C fiyatları göster
        None,
    )
    .await
    {
        Ok(cart_response) => {
            // Sadece bu cart'a ait items'ları filtrele
            // Cart service zaten doğru cart'ı döndürüyor
            let filtered_items: Vec<AdminOrderItem> = cart_response.items;

            Ok(filtered_items)
        }
        Err(_) => Ok(vec![]), // Hata durumunda boş liste döndür
    }
}

/// İptal talebini onayla
pub async fn accept_cancel_request(
    db: &DatabaseConnection,
    cart_id: i64,
    cart_item_id: i64,
) -> Result<(), AdminServiceError> {
    use crate::modules::ecommerce::models::cart_item as cart_item_model;

    // CartItem'ı bul ve güncelle
    let item = cart_item_model::Entity::find_by_id(cart_item_id)
        .one(db)
        .await?
        .ok_or(AdminServiceError::NotFound)?;

    // Sadece cancel_request olanları onayla
    if item.status.as_ref() != Some(&cart_item_model::status::CANCEL_REQUEST.to_string()) {
        return Err(AdminServiceError::NotFound);
    }

    // Cart'ın bu item'a ait olduğunu kontrol et
    if item.cart_id != cart_id {
        return Err(AdminServiceError::NotFound);
    }

    // Item bilgilerini al (meta data'dan) - item'ı kullanmadan önce
    let item_quantity = item.quantity;
    let _product_title = item
        .product_meta_data
        .as_ref()
        .and_then(|m| m.get("product_title"))
        .and_then(|v| v.as_str())
        .unwrap_or("Ürün")
        .to_string();

    let _price = item
        .product_meta_data
        .as_ref()
        .and_then(|m| m.get("price"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut active_model: cart_item_model::ActiveModel = item.into();
    active_model.status = Set(Some(cart_item_model::status::CANCEL_ACCEPT.to_string()));
    active_model.updated_at = Set(Some(Utc::now().into()));

    let _updated_item = active_model.update(db).await?;

    // Cart'ın toplamını güncelle
    update_cart_total_after_cancel(db, cart_id).await?;

    // Güncellenmiş order'ı al (item bilgileri için)
    let updated_order = get_admin_order(db, cart_id).await?;

    // İptal edilen item'ı bul
    let cancelled_item = updated_order
        .items
        .as_ref()
        .and_then(|items| items.iter().find(|i| i.id == cart_item_id));

    let (product_title, variant_display, product_cover, unit_price) =
        if let Some(item) = cancelled_item {
            (
                item.product_title.clone(),
                item.variant_display.clone(),
                item.product_cover.clone(),
                item.price,
            )
        } else {
            (format!("Ürün #{}", cart_item_id), None, String::new(), 0.0)
        };

    // Cart bilgilerini al (email için)
    let cart = Cart::find_by_id(cart_id).one(db).await?;

    if let Some(cart_data) = cart {
        // Timeline event oluştur
        let _ = create_cancel_timeline_event(
            db,
            cart_id,
            cart_data.user_id,
            "accept",
            &product_title,
            item_quantity,
            unit_price,
            cart_data.currency.as_deref().unwrap_or("TRY"),
        )
        .await;

        // Email gönder (background task olarak)
        let db_clone = db.clone();

        if let Ok(Some(user)) = crate::modules::auth::models::User::find_by_id(cart_data.user_id)
            .one(db)
            .await
        {
            let email = user.email.clone();
            let user_name = format!(
                "{} {}",
                user.first_name.as_deref().unwrap_or(""),
                user.last_name.as_deref().unwrap_or("")
            )
            .trim()
            .to_string();

            let order_id = cart_data
                .order_id
                .clone()
                .unwrap_or_else(|| cart_id.to_string());
            let currency = cart_data
                .currency
                .clone()
                .unwrap_or_else(|| "TRY".to_string());
            let refund_amount = unit_price * item_quantity as f64;
            let product_title_clone = product_title.clone();
            let variant_disp = variant_display.clone();
            let cover_img = if product_cover.is_empty() {
                None
            } else {
                Some(product_cover.clone())
            };

            tokio::spawn(async move {
                let _ = crate::modules::mailer::MailHelper::send_cancel_request_accepted(
                    &db_clone,
                    &email,
                    &user_name,
                    &order_id,
                    &product_title_clone,
                    variant_disp.as_deref(),
                    cover_img.as_deref(),
                    item_quantity,
                    unit_price,
                    refund_amount,
                    &currency,
                    &format!(
                        "{}/my-account/orders",
                        crate::config::get_config().get_base_url()
                    ),
                    "tr",
                )
                .await;
            });
        }

        // NOT: İade işlemi burada yapılmaz.
        // İade (B2B kredi veya banka) her zaman bulk refund üzerinden yapılır.
        // Böylece kargo düşümü ve refund_status/refund_amount kaydı tek bir yerde tutulur.
    }

    Ok(())
}

/// İptal sonrası cart total_amount güncelle
/// NOT: cargo_price'a dokunmuyoruz! Orijinal kargo ücreti sipariş tamamlandığında
/// cart.cargo_price'a kaydedilir ve iade hesaplamalarında kullanılır.
/// Burada sadece total_amount (aktif ürünlerin toplamı) güncellenir.
async fn update_cart_total_after_cancel(
    db: &DatabaseConnection,
    cart_id: i64,
) -> Result<(), AdminServiceError> {
    use crate::modules::ecommerce::models::cart::Entity as CartEntity;

    // Cart'ı bul
    let cart = CartEntity::find_by_id(cart_id)
        .one(db)
        .await?
        .ok_or(AdminServiceError::NotFound)?;

    // get_cart fonksiyonunu kullanarak toplamı hesapla
    // Bu, sipariş görüntüleme ile aynı mantığı kullanır
    match crate::modules::ecommerce::services::cart_service::get_cart(
        db,
        cart_id,
        Some("tr".to_string()),
        None, // Admin panelde B2C fiyatları
        None,
    )
    .await
    {
        Ok(cart_response) => {
            // final_total: aktif ürünlerin toplamı (iptal edilenler hariç)
            let total_decimal = rust_decimal::Decimal::from_f64_retain(cart_response.final_total)
                .unwrap_or_default();

            let mut cart_active: crate::modules::ecommerce::models::cart::ActiveModel = cart.into();
            cart_active.total_amount = Set(Some(total_decimal));
            cart_active.updated_at = Set(Some(Utc::now().into()));

            cart_active.update(db).await?;
            Ok(())
        }
        Err(_) => Err(AdminServiceError::DatabaseError(DbErr::Custom(
            "Hesaplama hatası".into(),
        ))),
    }
}

/// Cart tamamen iptal edildiğinde tüm itemları iptal et
async fn accept_cart_cancellation(
    db: &DatabaseConnection,
    cart_id: i64,
) -> Result<(), AdminServiceError> {
    use crate::modules::ecommerce::models::cart_item as cart_item_model;
    use crate::modules::ecommerce::models::cart_item::Column;

    let items = cart_item_model::Entity::find()
        .filter(Column::CartId.eq(cart_id))
        .all(db)
        .await?;

    let mut updated_count = 0;

    for item in items {
        let current_status = item.status.as_deref();

        // Sadece null veya cancel_request olanları cancel_accept yap
        if current_status.is_none()
            || current_status == Some(cart_item_model::status::CANCEL_REQUEST)
        {
            let mut active_model: cart_item_model::ActiveModel = item.into();
            active_model.status = Set(Some(cart_item_model::status::CANCEL_ACCEPT.to_string()));
            active_model.updated_at = Set(Some(Utc::now().into()));
            active_model.update(db).await?;
            updated_count += 1;
        }
    }

    if updated_count > 0 {
        update_cart_total_after_cancel(db, cart_id).await?;
    }

    Ok(())
}

/// İptal işlemi için timeline event oluştur
async fn create_cancel_timeline_event(
    db: &DatabaseConnection,
    cart_id: i64,
    user_id: i64,
    action: &str, // "request", "accept", "reject", "cancel"
    product_title: &str,
    quantity: i32,
    price: f64,
    currency: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::modules::timeline::services::{CreateTimelineEventRequest, TimelineService};

    let (title_tr, title_en, desc_tr, desc_en, icon, color) = match action {
        "request" => (
            "İptal talebi oluşturuldu",
            "Cancel request created",
            format!(
                "{} adet {} ürünü için iptal talebi oluşturdunuz",
                quantity, product_title
            ),
            format!(
                "You created a cancel request for {} piece of {}",
                quantity, product_title
            ),
            "bi-arrow-return-left",
            "#ffc107",
        ),
        "accept" => (
            "İptal talebi onaylandı",
            "Cancel request accepted",
            format!(
                "{} adet {} ürününün iptali onaylandı",
                quantity, product_title
            ),
            format!(
                "Cancel request for {} piece of {} has been accepted",
                quantity, product_title
            ),
            "bi-check-circle",
            "#28a745",
        ),
        "reject" => (
            "İptal talebi reddedildi",
            "Cancel request rejected",
            format!(
                "{} adet {} ürününün iptal talebi reddedildi",
                quantity, product_title
            ),
            format!(
                "Cancel request for {} piece of {} has been rejected",
                quantity, product_title
            ),
            "bi-x-circle",
            "#dc3545",
        ),
        "cancel" => (
            "İptal talebi iptal edildi",
            "Cancel request cancelled",
            format!(
                "{} adet {} ürünü için iptal talebinizi geri çektiniz",
                quantity, product_title
            ),
            format!(
                "You cancelled your cancel request for {} piece of {}",
                quantity, product_title
            ),
            "bi-arrow-counterclockwise",
            "#6c757d",
        ),
        _ => return Ok(()),
    };

    let formatted_price = format_price(price * quantity as f64, currency);

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
                format!("cancel_{}", action),
            ),
            title: title_map,
            description: Some(desc_map),
            icon: Some(icon.to_string()),
            color: Some(color.to_string()),
            user_id: Some(user_id),
            admin_user_id: None,
            metadata: Some(serde_json::json!({
                "product_title": product_title,
                "quantity": quantity,
                "price": price,
                "total_price": price * quantity as f64,
                "currency": currency,
                "action": action
            })),
            is_public: Some(false),
            is_admin_only: Some(false),
        },
    )
    .await;

    Ok(())
}

/// İptal talebini reddet
pub async fn reject_cancel_request(
    db: &DatabaseConnection,
    cart_id: i64,
    cart_item_id: i64,
) -> Result<(), AdminServiceError> {
    use crate::modules::ecommerce::models::cart_item as cart_item_model;

    // Önce güncel order'ı al (item bilgileri için - işlemden önce!)
    let current_order = get_admin_order(db, cart_id).await?;

    // İptal edilecek item'ı bul
    let cancel_item_info = current_order
        .items
        .as_ref()
        .and_then(|items| items.iter().find(|i| i.id == cart_item_id));

    let (
        cancel_product_title,
        cancel_variant_display,
        cancel_product_cover,
        cancel_price,
        cancel_item_quantity,
    ) = if let Some(item) = cancel_item_info {
        (
            item.product_title.clone(),
            item.variant_display.clone(),
            item.product_cover.clone(),
            item.price,
            item.quantity,
        )
    } else {
        // Fallback - product_meta_data dene
        let cancel_item = cart_item_model::Entity::find_by_id(cart_item_id)
            .one(db)
            .await?
            .ok_or(AdminServiceError::NotFound)?;

        let meta_price = cancel_item
            .product_meta_data
            .as_ref()
            .and_then(|m| m.get("price"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        (
            cancel_item
                .product_meta_data
                .as_ref()
                .and_then(|m| m.get("product_title"))
                .and_then(|v| v.as_str())
                .unwrap_or("Ürün")
                .to_string(),
            cancel_item.variant_display.clone(),
            String::new(),
            meta_price,
            cancel_item.quantity,
        )
    };

    // CartItem'ı bul ve güncelle
    let cancel_item = cart_item_model::Entity::find_by_id(cart_item_id)
        .one(db)
        .await?
        .ok_or(AdminServiceError::NotFound)?;

    // Sadece cancel_request olanları reddet
    if cancel_item.status.as_ref() != Some(&cart_item_model::status::CANCEL_REQUEST.to_string()) {
        return Err(AdminServiceError::NotFound);
    }

    // Cart'ın bu item'a ait olduğunu kontrol et
    if cancel_item.cart_id != cart_id {
        return Err(AdminServiceError::NotFound);
    }

    // Aynı ürün ve varyant için normal statuslu item'ı bul
    // variant_key None ise de düzgün çalışmalı
    let cancel_variant_key = cancel_item.variant_key.clone();

    let original_item = if cancel_variant_key.is_some() {
        cart_item_model::Entity::find()
            .filter(cart_item_model::Column::CartId.eq(cart_id))
            .filter(cart_item_model::Column::ProductId.eq(cancel_item.product_id))
            .filter(cart_item_model::Column::VariantKey.eq(cancel_variant_key.clone()))
            .filter(cart_item_model::Column::Status.is_null())
            .one(db)
            .await?
    } else {
        cart_item_model::Entity::find()
            .filter(cart_item_model::Column::CartId.eq(cart_id))
            .filter(cart_item_model::Column::ProductId.eq(cancel_item.product_id))
            .filter(cart_item_model::Column::VariantKey.is_null())
            .filter(cart_item_model::Column::Status.is_null())
            .one(db)
            .await?
    };

    if let Some(original) = original_item {
        // Normal item varsa, quantity'lerini birleştir
        let new_quantity = original.quantity + cancel_item_quantity;

        let mut active_model: cart_item_model::ActiveModel = (original).into();
        active_model.quantity = Set(new_quantity);
        active_model.updated_at = Set(Some(Utc::now().into()));
        active_model.update(db).await?;

        // Cancel item'ı sil
        let cancel_item_entity = cart_item_model::Entity::find_by_id(cart_item_id)
            .one(db)
            .await?
            .ok_or(AdminServiceError::NotFound)?;
        cancel_item_entity.delete(db).await?;
    } else {
        // Normal item yoksa (tümü iptal edilmişti), sadece status'u normal yap
        let mut active_model: cart_item_model::ActiveModel = cancel_item.into();
        active_model.status = Set(None);
        active_model.updated_at = Set(Some(Utc::now().into()));
        active_model.update(db).await?;
    }

    // Use pre-fetched data for email - no need to call get_admin_order again

    // Cart bilgilerini al (email için)
    let cart = Cart::find_by_id(cart_id).one(db).await?;

    if let Some(cart_data) = cart {
        // Timeline event oluştur
        let _ = create_cancel_timeline_event(
            db,
            cart_id,
            cart_data.user_id,
            "reject",
            &cancel_product_title,
            cancel_item_quantity,
            cancel_price,
            cart_data.currency.as_deref().unwrap_or("TRY"),
        )
        .await;

        // Email gönder (background task olarak)
        let db_clone = db.clone();

        if let Ok(Some(user)) = crate::modules::auth::models::User::find_by_id(cart_data.user_id)
            .one(db)
            .await
        {
            let email = user.email.clone();
            let user_name = format!(
                "{} {}",
                user.first_name.as_deref().unwrap_or(""),
                user.last_name.as_deref().unwrap_or("")
            )
            .trim()
            .to_string();

            let order_id = cart_data
                .order_id
                .clone()
                .unwrap_or_else(|| cart_id.to_string());
            let currency = cart_data
                .currency
                .clone()
                .unwrap_or_else(|| "TRY".to_string());
            let product_title_clone = cancel_product_title.clone();
            let variant_disp = cancel_variant_display.clone();
            let cover_img = if cancel_product_cover.is_empty() {
                None
            } else {
                Some(cancel_product_cover.clone())
            };

            tokio::spawn(async move {
                let _ = crate::modules::mailer::MailHelper::send_cancel_request_rejected(
                    &db_clone,
                    &email,
                    &user_name,
                    &order_id,
                    &product_title_clone,
                    variant_disp.as_deref(),
                    cover_img.as_deref(),
                    cancel_item_quantity,
                    cancel_price,
                    &currency,
                    &format!(
                        "{}/my-account/orders",
                        crate::config::get_config().get_base_url()
                    ),
                    "tr",
                )
                .await;
            });
        }
    }

    Ok(())
}

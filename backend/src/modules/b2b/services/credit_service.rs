use crate::modules::b2b::entities::{companies, credit_transactions};
use crate::modules::currency::services::exchange_rate_service;
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::*;

/// Verilen tutarı company referans para birimine çevirir.
/// Aynı para birimiyse conversion yapılmaz.
/// Kur bilgisi yoksa orijinal tutarı döndürür (fallback).
async fn to_company_currency(
    db: &DatabaseConnection,
    amount: Decimal,
    from_currency: &str,
    company_currency: &str,
) -> Decimal {
    if from_currency.to_uppercase() == company_currency.to_uppercase() {
        return amount;
    }

    let rates = exchange_rate_service::get_cached_rates(db).await;
    if let Some(rates) = rates {
        let amount_f64 = amount.to_string().parse::<f64>().unwrap_or(0.0);
        if let Some(converted) = exchange_rate_service::convert_currency(
            amount_f64,
            from_currency,
            company_currency,
            &rates,
        ) {
            return Decimal::from_f64_retain(converted).unwrap_or(amount);
        }
    }

    // Kur bilgisi yoksa orijinal tutarı kullan (en kötü durum)
    eprintln!(
        "⚠️ Exchange rate not available: cannot convert {} {} to {}. Using original amount.",
        amount, from_currency, company_currency
    );
    amount
}

#[derive(Debug)]
pub enum CreditServiceError {
    DatabaseError(DbErr),
    CompanyNotFound,
    InsufficientCredit,
    InvalidAmount,
}

impl From<DbErr> for CreditServiceError {
    fn from(err: DbErr) -> Self {
        CreditServiceError::DatabaseError(err)
    }
}

impl std::fmt::Display for CreditServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreditServiceError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
            CreditServiceError::CompanyNotFound => write!(f, "Şirket bulunamadı"),
            CreditServiceError::InsufficientCredit => write!(f, "Yetersiz kredi limiti"),
            CreditServiceError::InvalidAmount => write!(f, "Geçersiz tutar"),
        }
    }
}

/// Şirketin mevcut kredi bakiyesini al (kullanılabilir kredi)
/// Döndürür: (credit_limit, used_credit, available_credit)
///
/// NOT: used_credit negatif olabilir (şirket bizden alacaklı)
/// - used_credit > 0: Şirket bizden borçlu
/// - used_credit = 0: Borç/alacak yok
/// - used_credit < 0: Şirket bizden alacaklı (biz ona borçluyuz)
///
/// available_credit = credit_limit - used_credit
/// Eğer used_credit negatifse, available_credit > credit_limit olur
pub async fn get_company_balance(
    db: &DatabaseConnection,
    company_id: i64,
) -> Result<(Decimal, Decimal, Decimal), CreditServiceError> {
    let company = companies::Entity::find_by_id(company_id)
        .one(db)
        .await?
        .ok_or(CreditServiceError::CompanyNotFound)?;

    let available_credit = company.credit_limit - company.used_credit;

    Ok((company.credit_limit, company.used_credit, available_credit))
}

/// Şirketin bekleyen ödemelerini company referans para biriminde hesapla.
/// Bekleyen ödeme = Henüz tamamlanmamış B2B kredili siparişler
/// (status = 'open_cart' VE payment_method = 'b2b_credit')
/// Farklı para birimlerindeki sepetler company.currency'e çevrilerek toplanır.
// pub async fn get_pending_payments(
//     db: &DatabaseConnection,
//     company_id: i64,
// ) -> Result<Decimal, CreditServiceError> {
//     use crate::modules::b2b::entities::company_users;
//     use crate::modules::ecommerce::models::cart::{self, Entity as Cart};

//     // Company referans para birimini al
//     let company = companies::Entity::find_by_id(company_id)
//         .one(db)
//         .await?
//         .ok_or(CreditServiceError::CompanyNotFound)?;
//     let company_currency = company.currency.clone().unwrap_or_else(|| "TRY".to_string());

//     // Şirkete ait kullanıcıları bul
//     let company_user_ids: Vec<i64> = company_users::Entity::find()
//         .filter(company_users::Column::CompanyId.eq(company_id))
//         .all(db)
//         .await?
//         .into_iter()
//         .map(|cu| cu.user_id)
//         .collect();

//     if company_user_ids.is_empty() {
//         return Ok(Decimal::ZERO);
//     }

//     // Bu kullanıcıların B2B kredili ve henüz tamamlanmamış siparişlerini bul
//     // open_cart, pending, confirmed, preparing, shipped - ödeme henüz alınmamış siparişler
//     let pending_carts = Cart::find()
//         .filter(cart::Column::UserId.is_in(company_user_ids))
//         .filter(cart::Column::Status.is_in(["open_cart", "pending", "confirmed", "preparing", "shipped"]))
//         .filter(cart::Column::PaymentMethod.eq("b2b_credit"))
//         .filter(cart::Column::CartType.eq("b2b"))
//         .all(db)
//         .await?;

//     // Her sepetin tutarını company.currency'e çevirerek topla
//     let mut total_pending = Decimal::ZERO;
//     for pending_cart in &pending_carts {
//         if let Some(amount) = pending_cart.total_amount {
//             let cart_currency = pending_cart
//                 .currency
//                 .clone()
//                 .unwrap_or_else(|| "TRY".to_string());
//             let converted = to_company_currency(db, amount, &cart_currency, &company_currency).await;
//             total_pending += converted;
//         }
//     }

//     Ok(total_pending)
// }

/// Kredi limitinin yeterli olup olmadığını kontrol et.
/// amount ve currency, siparişin para biriminde verilir;
/// company'nin referans para birimine çevrilerek karşılaştırılır.
pub async fn check_credit_availability(
    db: &DatabaseConnection,
    company_id: i64,
    amount: Decimal,
    currency: &str,
) -> Result<bool, CreditServiceError> {
    let company = companies::Entity::find_by_id(company_id)
        .one(db)
        .await?
        .ok_or(CreditServiceError::CompanyNotFound)?;

    let company_currency = company
        .currency
        .clone()
        .unwrap_or_else(|| "TRY".to_string());
    let amount_in_company_currency =
        to_company_currency(db, amount, currency, &company_currency).await;

    let available_credit = company.credit_limit - company.used_credit;
    Ok(available_credit >= amount_in_company_currency)
}

/// Kredili alışveriş işlemi oluştur (sipariş verildiğinde)
/// Bu fonksiyon:
/// 1. Sipariş tutarını company referans para birimine çevirir
/// 2. Şirketin used_credit'ini company.currency cinsinden artırır
/// 3. İşlem kaydı oluşturur (amount orijinal currency'de, balance company currency'de)
pub async fn create_purchase_transaction(
    db: &DatabaseConnection,
    company_id: i64,
    cart_id: i64,
    amount: Decimal,
    currency: String,
    description: Option<String>,
) -> Result<credit_transactions::Model, CreditServiceError> {
    if amount <= Decimal::ZERO {
        return Err(CreditServiceError::InvalidAmount);
    }

    // Company referans para birimini al
    let company_currency = {
        let company = companies::Entity::find_by_id(company_id)
            .one(db)
            .await?
            .ok_or(CreditServiceError::CompanyNotFound)?;
        company
            .currency
            .clone()
            .unwrap_or_else(|| "TRY".to_string())
    };

    // Sipariş tutarını company.currency'e çevir (balance güncellemesi için)
    let converted_amount = to_company_currency(db, amount, &currency, &company_currency).await;

    // Transaction başlat
    let txn = db.begin().await?;

    // Şirketi bul ve kilitle (FOR UPDATE)
    let company = companies::Entity::find_by_id(company_id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(CreditServiceError::CompanyNotFound)?;

    // Kredi kontrolü (company.currency cinsinden)
    let available_credit = company.credit_limit - company.used_credit;
    if available_credit < converted_amount {
        return Err(CreditServiceError::InsufficientCredit);
    }

    // balance_before / balance_after her zaman company.currency cinsinden
    let balance_before = company.used_credit;
    let balance_after = balance_before + converted_amount;

    // Şirketin used_credit'ini güncelle (company.currency cinsinden)
    let mut company_active: companies::ActiveModel = company.into();
    company_active.used_credit = Set(balance_after);
    company_active.update(&txn).await?;

    // İşlem kaydı: amount ve currency orijinal (cart currency), balance company.currency
    let transaction = credit_transactions::ActiveModel {
        company_id: Set(company_id),
        cart_id: Set(Some(cart_id)),
        transaction_type: Set(credit_transactions::transaction_type::PURCHASE.to_string()),
        amount: Set(amount),
        currency: Set(currency),
        balance_before: Set(balance_before),
        balance_after: Set(balance_after),
        description: Set(description),
        reference_number: Set(None),
        created_by: Set(None),
        created_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let transaction = transaction.insert(&txn).await?;

    // Transaction commit
    txn.commit().await?;

    Ok(transaction)
}

/// Ödeme işlemi oluştur (şirket borç ödediğinde)
/// Bu fonksiyon:
/// 1. Ödeme tutarını company referans para birimine çevirir
/// 2. Şirketin used_credit'ini company.currency cinsinden azaltır
/// 3. İşlem kaydı oluşturur
///
/// NOT: Fazla ödeme yapılabilir, bu durumda used_credit negatif olur
/// Örnek: used_credit = 1000, ödeme = 1500 → yeni used_credit = -500 (şirket 500 alacaklı)
pub async fn create_payment_transaction(
    db: &DatabaseConnection,
    company_id: i64,
    amount: Decimal,
    currency: String,
    reference_number: Option<String>,
    description: Option<String>,
    created_by: Option<i64>, // Admin user_id
) -> Result<credit_transactions::Model, CreditServiceError> {
    if amount <= Decimal::ZERO {
        return Err(CreditServiceError::InvalidAmount);
    }

    // Company referans para birimini al
    let company_currency = {
        let company = companies::Entity::find_by_id(company_id)
            .one(db)
            .await?
            .ok_or(CreditServiceError::CompanyNotFound)?;
        company
            .currency
            .clone()
            .unwrap_or_else(|| "TRY".to_string())
    };

    // Ödeme tutarını company.currency'e çevir
    let converted_amount = to_company_currency(db, amount, &currency, &company_currency).await;

    // Transaction başlat
    let txn = db.begin().await?;

    // Şirketi bul ve kilitle
    let company = companies::Entity::find_by_id(company_id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(CreditServiceError::CompanyNotFound)?;

    // balance_before / balance_after company.currency cinsinden
    let balance_before = company.used_credit;
    let balance_after = balance_before - converted_amount;

    // Şirketin used_credit'ini güncelle (company.currency cinsinden)
    let mut company_active: companies::ActiveModel = company.into();
    company_active.used_credit = Set(balance_after);
    company_active.update(&txn).await?;

    // İşlem kaydı: amount ve currency orijinal, balance company.currency
    let transaction = credit_transactions::ActiveModel {
        company_id: Set(company_id),
        cart_id: Set(None), // Ödeme işleminde cart_id yok
        transaction_type: Set(credit_transactions::transaction_type::PAYMENT.to_string()),
        amount: Set(amount),
        currency: Set(currency),
        balance_before: Set(balance_before),
        balance_after: Set(balance_after),
        description: Set(description),
        reference_number: Set(reference_number),
        created_by: Set(created_by),
        created_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let transaction = transaction.insert(&txn).await?;

    // Transaction commit
    txn.commit().await?;

    Ok(transaction)
}

/// İade işlemi oluştur (sipariş iptal edildiğinde)
/// Bu fonksiyon:
/// 1. İade tutarını company referans para birimine çevirir
/// 2. Şirketin used_credit'ini company.currency cinsinden azaltır
/// 3. İşlem kaydı oluşturur
///
/// NOT: İade sonucu used_credit negatif olabilir (şirket alacaklı olur)
pub async fn create_refund_transaction(
    db: &DatabaseConnection,
    company_id: i64,
    cart_id: Option<i64>,
    amount: Decimal,
    currency: String,
    description: Option<String>,
) -> Result<credit_transactions::Model, CreditServiceError> {
    if amount <= Decimal::ZERO {
        return Err(CreditServiceError::InvalidAmount);
    }

    // Company referans para birimini al
    let company_currency = {
        let company = companies::Entity::find_by_id(company_id)
            .one(db)
            .await?
            .ok_or(CreditServiceError::CompanyNotFound)?;
        company
            .currency
            .clone()
            .unwrap_or_else(|| "TRY".to_string())
    };

    // İade tutarını company.currency'e çevir
    let converted_amount = to_company_currency(db, amount, &currency, &company_currency).await;

    // Transaction başlat
    let txn = db.begin().await?;

    // Şirketi bul ve kilitle
    let company = companies::Entity::find_by_id(company_id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(CreditServiceError::CompanyNotFound)?;

    // balance_before / balance_after company.currency cinsinden
    let balance_before = company.used_credit;
    let balance_after = balance_before - converted_amount;

    // Şirketin used_credit'ini güncelle (company.currency cinsinden)
    let mut company_active: companies::ActiveModel = company.into();
    company_active.used_credit = Set(balance_after);
    company_active.update(&txn).await?;

    // İşlem kaydı: amount ve currency orijinal (cart currency), balance company.currency
    let transaction = credit_transactions::ActiveModel {
        company_id: Set(company_id),
        cart_id: Set(cart_id),
        transaction_type: Set(credit_transactions::transaction_type::REFUND.to_string()),
        amount: Set(amount),
        currency: Set(currency),
        balance_before: Set(balance_before),
        balance_after: Set(balance_after),
        description: Set(description),
        reference_number: Set(None),
        created_by: Set(None),
        created_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let transaction = transaction.insert(&txn).await?;

    // Transaction commit
    txn.commit().await?;

    Ok(transaction)
}

/// Manuel düzeltme işlemi (admin tarafından)
/// amount pozitif ise used_credit artar (borç ekle)
/// amount negatif ise used_credit azalır (borç sil / alacak ekle)
///
/// NOT: used_credit negatif olabilir (şirket alacaklı)
/// Tutar company.currency'e çevrilerek bakiyeye uygulanır.
pub async fn create_adjustment_transaction(
    db: &DatabaseConnection,
    company_id: i64,
    amount: Decimal, // Pozitif veya negatif olabilir
    currency: String,
    description: Option<String>,
    created_by: i64, // Admin user_id (zorunlu)
) -> Result<credit_transactions::Model, CreditServiceError> {
    if amount == Decimal::ZERO {
        return Err(CreditServiceError::InvalidAmount);
    }

    // Company referans para birimini al
    let company_currency = {
        let company = companies::Entity::find_by_id(company_id)
            .one(db)
            .await?
            .ok_or(CreditServiceError::CompanyNotFound)?;
        company
            .currency
            .clone()
            .unwrap_or_else(|| "TRY".to_string())
    };

    // Tutarı company.currency'e çevir (işaret korunarak)
    let sign = if amount < Decimal::ZERO {
        -Decimal::ONE
    } else {
        Decimal::ONE
    };
    let converted_amount =
        to_company_currency(db, amount.abs(), &currency, &company_currency).await * sign;

    // Transaction başlat
    let txn = db.begin().await?;

    // Şirketi bul ve kilitle
    let company = companies::Entity::find_by_id(company_id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(CreditServiceError::CompanyNotFound)?;

    // balance_before / balance_after company.currency cinsinden
    let balance_before = company.used_credit;
    let balance_after = balance_before + converted_amount;

    // Şirketin used_credit'ini güncelle (company.currency cinsinden)
    let mut company_active: companies::ActiveModel = company.into();
    company_active.used_credit = Set(balance_after);
    company_active.update(&txn).await?;

    // İşlem kaydı: amount ve currency orijinal, balance company.currency
    let transaction = credit_transactions::ActiveModel {
        company_id: Set(company_id),
        cart_id: Set(None),
        transaction_type: Set(credit_transactions::transaction_type::ADJUSTMENT.to_string()),
        amount: Set(amount),
        currency: Set(currency),
        balance_before: Set(balance_before),
        balance_after: Set(balance_after),
        description: Set(description),
        reference_number: Set(None),
        created_by: Set(Some(created_by)),
        created_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let transaction = transaction.insert(&txn).await?;

    // Transaction commit
    txn.commit().await?;

    Ok(transaction)
}

use crate::modules::b2b::entities::{commission_transactions, company_representatives};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::*;

#[derive(Debug)]
pub enum CommissionServiceError {
    DatabaseError(DbErr),
    RepresentativeNotFound,
    InvalidAmount,
    InsufficientCommission,
}

impl From<DbErr> for CommissionServiceError {
    fn from(err: DbErr) -> Self {
        CommissionServiceError::DatabaseError(err)
    }
}

impl std::fmt::Display for CommissionServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommissionServiceError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
            CommissionServiceError::RepresentativeNotFound => write!(f, "Temsilci bulunamadı"),
            CommissionServiceError::InvalidAmount => write!(f, "Geçersiz tutar"),
            CommissionServiceError::InsufficientCommission => {
                write!(f, "Yetersiz komisyon bakiyesi. Ödeme tutarı bekleyen komisyondan fazla olamaz")
            }
        }
    }
}

/// Komisyon kazanıldı (sipariş tamamlandığında otomatik çağrılır)
/// Bu fonksiyon:
/// 1. Şirketin temsilcisini bulur
/// 2. Komisyon hesaplar
/// 3. accumulated_commission ve total_sales_amount günceller
/// 4. İşlem kaydı oluşturur
/// 
/// Eğer şirketin temsilcisi yoksa None döner (hata değil)
pub async fn create_earned_commission(
    db: &DatabaseConnection,
    company_id: i64,
    cart_id: i64,
    order_amount: Decimal,
    currency: String,
) -> Result<Option<commission_transactions::Model>, CommissionServiceError> {
    if order_amount <= Decimal::ZERO {
        return Err(CommissionServiceError::InvalidAmount);
    }

    // Şirketin temsilcisini bul
    let representative = company_representatives::Entity::find()
        .filter(company_representatives::Column::CompanyId.eq(company_id))
        .filter(company_representatives::Column::IsActive.eq(true))
        .one(db)
        .await?;

    // Temsilci yoksa None dön (hata değil, normal durum)
    let representative = match representative {
        Some(rep) => rep,
        None => return Ok(None),
    };

    // Komisyon hesapla
    let commission_amount = representative.calculate_commission(order_amount);

    // Transaction başlat
    let txn = db.begin().await?;

    // Temsilciyi kilitle (FOR UPDATE)
    let representative = company_representatives::Entity::find_by_id(representative.id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(CommissionServiceError::RepresentativeNotFound)?;

    let balance_before = representative.accumulated_commission;
    let balance_after = balance_before + commission_amount;

    // Temsilcinin komisyon bakiyesini güncelle
    let mut representative_active: company_representatives::ActiveModel = representative.into();
    representative_active.accumulated_commission = Set(balance_after);
    representative_active.total_sales_amount =
        Set(representative_active.total_sales_amount.unwrap() + order_amount);
    representative_active.updated_at = Set(Utc::now().into());
    let updated_rep = representative_active.update(&txn).await?;

    // İşlem kaydı oluştur
    let transaction = commission_transactions::ActiveModel {
        representative_id: Set(updated_rep.id),
        company_id: Set(company_id),
        cart_id: Set(Some(cart_id)),
        transaction_type: Set(commission_transactions::transaction_type::EARNED.to_string()),
        amount: Set(commission_amount),
        order_amount: Set(Some(order_amount)),
        commission_rate: Set(Some(updated_rep.commission_rate)),
        currency: Set(currency),
        balance_before: Set(balance_before),
        balance_after: Set(balance_after),
        description: Set(Some(format!(
            "Sipariş #{} için %{} komisyon",
            cart_id, updated_rep.commission_rate
        ))),
        reference_number: Set(None),
        created_by: Set(None),
        created_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let transaction = transaction.insert(&txn).await?;

    // Transaction commit
    txn.commit().await?;

    Ok(Some(transaction))
}

/// Komisyon ödemesi (admin tarafından)
/// Bu fonksiyon:
/// 1. Temsilcinin accumulated_commission'ını azaltır
/// 2. İşlem kaydı oluşturur
pub async fn create_commission_payment(
    db: &DatabaseConnection,
    representative_id: i64,
    amount: Decimal,
    currency: String,
    reference_number: Option<String>,
    description: Option<String>,
    created_by: i64, // Admin user_id
) -> Result<commission_transactions::Model, CommissionServiceError> {
    if amount <= Decimal::ZERO {
        return Err(CommissionServiceError::InvalidAmount);
    }

    // Transaction başlat
    let txn = db.begin().await?;

    // Temsilciyi bul ve kilitle
    let representative = company_representatives::Entity::find_by_id(representative_id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(CommissionServiceError::RepresentativeNotFound)?;

    let balance_before = representative.accumulated_commission;
    
    // Ödeme tutarı bekleyen komisyondan fazla olamaz
    if amount > balance_before {
        return Err(CommissionServiceError::InsufficientCommission);
    }
    
    let balance_after = balance_before - amount;

    // Temsilcinin komisyon bakiyesini güncelle
    let mut representative_active: company_representatives::ActiveModel = representative.clone().into();
    representative_active.accumulated_commission = Set(balance_after);
    representative_active.updated_at = Set(Utc::now().into());
    representative_active.update(&txn).await?;

    // İşlem kaydı oluştur
    let transaction = commission_transactions::ActiveModel {
        representative_id: Set(representative_id),
        company_id: Set(representative.company_id),
        cart_id: Set(None),
        transaction_type: Set(commission_transactions::transaction_type::PAYMENT.to_string()),
        amount: Set(amount),
        order_amount: Set(None),
        commission_rate: Set(None),
        currency: Set(currency),
        balance_before: Set(balance_before),
        balance_after: Set(balance_after),
        description: Set(description),
        reference_number: Set(reference_number),
        created_by: Set(Some(created_by)),
        created_at: Set(Some(Utc::now().into())),
        ..Default::default()
    };

    let transaction = transaction.insert(&txn).await?;

    // Transaction commit
    txn.commit().await?;

    Ok(transaction)
}

/// Manuel düzeltme işlemi (admin tarafından)
/// amount pozitif ise accumulated_commission artar (komisyon ekle)
/// amount negatif ise accumulated_commission azalır (komisyon sil)
pub async fn create_commission_adjustment(
    db: &DatabaseConnection,
    representative_id: i64,
    amount: Decimal, // Pozitif veya negatif olabilir
    currency: String,
    description: String,
    created_by: i64, // Admin user_id (zorunlu)
) -> Result<commission_transactions::Model, CommissionServiceError> {
    if amount == Decimal::ZERO {
        return Err(CommissionServiceError::InvalidAmount);
    }

    // Transaction başlat
    let txn = db.begin().await?;

    // Temsilciyi bul ve kilitle
    let representative = company_representatives::Entity::find_by_id(representative_id)
        .lock_exclusive()
        .one(&txn)
        .await?
        .ok_or(CommissionServiceError::RepresentativeNotFound)?;

    let balance_before = representative.accumulated_commission;
    let balance_after = balance_before + amount; // amount negatif olabilir

    // Temsilcinin komisyon bakiyesini güncelle
    let mut representative_active: company_representatives::ActiveModel = representative.clone().into();
    representative_active.accumulated_commission = Set(balance_after);
    representative_active.updated_at = Set(Utc::now().into());
    representative_active.update(&txn).await?;

    // İşlem kaydı oluştur
    let transaction = commission_transactions::ActiveModel {
        representative_id: Set(representative_id),
        company_id: Set(representative.company_id),
        cart_id: Set(None),
        transaction_type: Set(commission_transactions::transaction_type::ADJUSTMENT.to_string()),
        amount: Set(amount),
        order_amount: Set(None),
        commission_rate: Set(None),
        currency: Set(currency),
        balance_before: Set(balance_before),
        balance_after: Set(balance_after),
        description: Set(Some(description)),
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

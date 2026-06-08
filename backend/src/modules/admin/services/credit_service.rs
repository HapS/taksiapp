use crate::modules::admin::dto::credit_dto::{
    CompanyCreditSummary, CreateAdjustmentRequest, CreatePaymentRequest, CreditTransactionResponse,
};
use crate::modules::b2b::entities::{companies, credit_transactions};
use crate::modules::b2b::services::credit_service;
use crate::modules::ecommerce::models::cart::Entity as Cart;
use crate::modules::utils::format_price::format_price;
use chrono::Local;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::*;

/// Tüm kredi işlemlerini getir (admin için)
pub async fn get_all_credit_transactions(
    db: &DatabaseConnection,
    company_id: Option<i64>,
    transaction_type: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<Vec<CreditTransactionResponse>, DbErr> {
    let mut query = credit_transactions::Entity::find();

    // Filtreler
    if let Some(cid) = company_id {
        query = query.filter(credit_transactions::Column::CompanyId.eq(cid));
    }

    if let Some(ttype) = transaction_type {
        if !ttype.is_empty() {
            query = query.filter(credit_transactions::Column::TransactionType.eq(ttype));
        }
    }

    if let Some(start) = start_date {
        if !start.is_empty() {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&start, "%Y-%m-%d") {
                let datetime = date
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_local_timezone(chrono::Local)
                    .unwrap();
                query = query.filter(credit_transactions::Column::CreatedAt.gte(datetime));
            }
        }
    }

    if let Some(end) = end_date {
        if !end.is_empty() {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&end, "%Y-%m-%d") {
                let datetime = date
                    .and_hms_opt(23, 59, 59)
                    .unwrap()
                    .and_local_timezone(chrono::Local)
                    .unwrap();
                query = query.filter(credit_transactions::Column::CreatedAt.lte(datetime));
            }
        }
    }

    // Sıralama
    let sort_column = sort_by.unwrap_or_else(|| "created_at".to_string());
    let sort_dir = sort_order.unwrap_or_else(|| "desc".to_string());

    query = match sort_column.as_str() {
        "id" => {
            if sort_dir == "asc" {
                query.order_by_asc(credit_transactions::Column::Id)
            } else {
                query.order_by_desc(credit_transactions::Column::Id)
            }
        }
        "amount" => {
            if sort_dir == "asc" {
                query.order_by_asc(credit_transactions::Column::Amount)
            } else {
                query.order_by_desc(credit_transactions::Column::Amount)
            }
        }
        "created_at" => {
            if sort_dir == "asc" {
                query.order_by_asc(credit_transactions::Column::CreatedAt)
            } else {
                query.order_by_desc(credit_transactions::Column::CreatedAt)
            }
        }
        _ => query.order_by_desc(credit_transactions::Column::CreatedAt),
    };

    // Limit ve offset
    if let Some(l) = limit {
        query = query.limit(l);
    }

    if let Some(o) = offset {
        query = query.offset(o);
    }

    let transactions = query.all(db).await?;

    // Response'a dönüştür
    let mut responses = Vec::new();
    for transaction in transactions {
        // Şirket bilgisini al (bakiye formatlaması için currency lazım)
        let company = companies::Entity::find_by_id(transaction.company_id)
            .one(db)
            .await?;
        let company_currency = company
            .as_ref()
            .and_then(|c| c.currency.clone())
            .unwrap_or_else(|| "TRY".to_string());
        let company_name = company
            .map(|c| c.company_name)
            .unwrap_or_else(|| "Bilinmeyen Şirket".to_string());

        // Cart varsa order_id'yi al
        let order_id = if let Some(cart_id) = transaction.cart_id {
            Cart::find_by_id(cart_id)
                .one(db)
                .await?
                .and_then(|c| c.order_id)
        } else {
            None
        };

        // Created by user name
        let created_by_name = if let Some(user_id) = transaction.created_by {
            crate::modules::auth::models::User::find_by_id(user_id)
                .one(db)
                .await?
                .map(|u| {
                    format!(
                        "{} {}",
                        u.first_name.unwrap_or_default(),
                        u.last_name.unwrap_or_default()
                    )
                    .trim()
                    .to_string()
                })
        } else {
            None
        };

        // Transaction type display
        let transaction_type_display = match transaction.transaction_type.as_str() {
            "purchase" => "Kredili Alışveriş",
            "payment" => "Ödeme",
            "adjustment" => "Düzeltme",
            "refund" => "İade",
            _ => "Bilinmeyen",
        }
        .to_string();

        // Format dates
        let created_at_formatted = transaction
            .created_at
            .map(|dt| {
                dt.with_timezone(&Local)
                    .format("%d.%m.%Y %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "-".to_string());

        responses.push(CreditTransactionResponse {
            id: transaction.id,
            company_id: transaction.company_id,
            company_name,
            cart_id: transaction.cart_id,
            order_id,
            transaction_type: transaction.transaction_type.clone(),
            transaction_type_display,
            // Tutar: orijinal para biriminde (hangi currency ile işlem yapıldı)
            amount: format_price(
                transaction.amount.to_f64().unwrap_or(0.0),
                &transaction.currency,
            ),
            amount_raw: transaction.amount.to_f64().unwrap_or(0.0),
            currency: transaction.currency.clone(),
            // Bakiye: her zaman company.currency cinsinden (convert edilmiş)
            balance_before: format_price(
                transaction.balance_before.to_f64().unwrap_or(0.0),
                &company_currency,
            ),
            balance_after: format_price(
                transaction.balance_after.to_f64().unwrap_or(0.0),
                &company_currency,
            ),
            description: transaction.description,
            reference_number: transaction.reference_number,
            created_by: transaction.created_by,
            created_by_name,
            created_at: transaction.created_at,
            created_at_formatted,
        });
    }

    Ok(responses)
}

/// Şirket kredi özetini getir
pub async fn get_company_credit_summary(
    db: &DatabaseConnection,
    company_id: i64,
) -> Result<CompanyCreditSummary, DbErr> {
    let company = companies::Entity::find_by_id(company_id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Company not found".to_string()))?;

    // Şirketin referans para birimini kullan (balance, limit, toplamlar hep bu birimde)
    let currency = company
        .currency
        .clone()
        .unwrap_or_else(|| "TRY".to_string());

    // Şirket sahibi kullanıcı bilgilerini al
    let user = crate::modules::auth::models::User::find_by_id(company.user_id)
        .one(db)
        .await?;

    let contact_person = user.as_ref().map(|u| {
        format!(
            "{} {}",
            u.first_name.as_deref().unwrap_or(""),
            u.last_name.as_deref().unwrap_or("")
        )
        .trim()
        .to_string()
    });

    let contact_email = user.map(|u| u.email);

    // Toplam işlem hacimlerini balance farkından hesapla (company.currency cinsinden)
    // balance_before/balance_after her zaman company.currency'de tutulduğundan
    // sum(|balance_after - balance_before|) doğru company.currency toplamını verir
    let all_transactions = credit_transactions::Entity::find()
        .filter(credit_transactions::Column::CompanyId.eq(company_id))
        .all(db)
        .await?;

    let mut total_purchases = rust_decimal::Decimal::ZERO;
    let mut total_payments = rust_decimal::Decimal::ZERO;
    let mut total_adjustments = rust_decimal::Decimal::ZERO;

    for tx in &all_transactions {
        let delta = (tx.balance_after - tx.balance_before).abs();
        match tx.transaction_type.as_str() {
            "purchase" => total_purchases += delta,
            "payment" => total_payments += delta,
            "adjustment" => total_adjustments += delta,
            _ => {}
        }
    }

    let total_transactions = all_transactions.len() as i64;

    let total_purchases_f64 = total_purchases.to_f64().unwrap_or(0.0);
    let total_payments_f64 = total_payments.to_f64().unwrap_or(0.0);
    let total_adjustments_f64 = total_adjustments.to_f64().unwrap_or(0.0);

    let credit_limit_f64 = company.credit_limit.to_f64().unwrap_or(0.0);
    let used_credit_f64 = company.used_credit.to_f64().unwrap_or(0.0);
    let available_credit_f64 = credit_limit_f64 - used_credit_f64;

    Ok(CompanyCreditSummary {
        company_id,
        company_name: company.company_name,
        tax_number: company.tax_number,
        contact_person,
        contact_email,
        credit_limit: format_price(credit_limit_f64, &currency),
        credit_limit_raw: credit_limit_f64,
        used_credit: format_price(used_credit_f64, &currency),
        used_credit_raw: used_credit_f64,
        available_credit: format_price(available_credit_f64, &currency),
        available_credit_raw: available_credit_f64,
        currency: currency.clone(),
        total_purchases: format_price(total_purchases_f64, &currency),
        total_payments: format_price(total_payments_f64, &currency),
        total_transactions,
        total_adjustments: format_price(total_adjustments_f64, &currency),
    })
}

/// Manuel ödeme kaydet (admin)
pub async fn create_manual_payment(
    db: &DatabaseConnection,
    request: CreatePaymentRequest,
    admin_user_id: i64,
) -> Result<credit_transactions::Model, credit_service::CreditServiceError> {
    credit_service::create_payment_transaction(
        db,
        request.company_id,
        request.amount,
        "TRY".to_string(), // Default currency
        request.reference_number,
        request.description,
        Some(admin_user_id),
    )
    .await
}

/// Kredi düzeltme (admin)
pub async fn create_credit_adjustment(
    db: &DatabaseConnection,
    request: CreateAdjustmentRequest,
    admin_user_id: i64,
) -> Result<credit_transactions::Model, credit_service::CreditServiceError> {
    let company = companies::Entity::find_by_id(request.company_id)
        .one(db)
        .await?
        .ok_or(credit_service::CreditServiceError::CompanyNotFound)?;

    let req_currency = request.currency.clone().expect("Req Cur Kullanmıyoruz");
    println!("Düzeltme Cur Post Admin {:?}", req_currency);
    credit_service::create_adjustment_transaction(
        db,
        request.company_id,
        request.amount,
        company
            .currency
            .clone()
            .unwrap_or_else(|| "TRY".to_string()),
        Some(request.description),
        admin_user_id,
    )
    .await
}

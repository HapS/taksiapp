use crate::modules::admin::dto::commission_dto::{
    CommissionTransactionResponse, CreateCommissionAdjustmentRequest,
    CreateCommissionPaymentRequest, RepresentativeCommissionSummary, RepresentativeListResponse,
};
use crate::modules::b2b::entities::{commission_transactions, company_representatives, companies};
use crate::modules::b2b::services::commission_service;
use crate::modules::ecommerce::models::cart::{Entity as Cart};
use crate::modules::utils::format_price::format_price;
use chrono::Local;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::*;

/// Tüm komisyon işlemlerini getir (admin için)
pub async fn get_all_commission_transactions(
    db: &DatabaseConnection,
    representative_id: Option<i64>,
    company_id: Option<i64>,
    transaction_type: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<Vec<CommissionTransactionResponse>, DbErr> {
    let mut query = commission_transactions::Entity::find();

    // Filtreler
    if let Some(rid) = representative_id {
        query = query.filter(commission_transactions::Column::RepresentativeId.eq(rid));
    }

    if let Some(cid) = company_id {
        query = query.filter(commission_transactions::Column::CompanyId.eq(cid));
    }

    if let Some(ttype) = transaction_type {
        if !ttype.is_empty() {
            query = query.filter(commission_transactions::Column::TransactionType.eq(ttype));
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
                query = query.filter(commission_transactions::Column::CreatedAt.gte(datetime));
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
                query = query.filter(commission_transactions::Column::CreatedAt.lte(datetime));
            }
        }
    }

    // Sıralama
    let sort_column = sort_by.unwrap_or_else(|| "created_at".to_string());
    let sort_dir = sort_order.unwrap_or_else(|| "desc".to_string());

    query = match sort_column.as_str() {
        "id" => {
            if sort_dir == "asc" {
                query.order_by_asc(commission_transactions::Column::Id)
            } else {
                query.order_by_desc(commission_transactions::Column::Id)
            }
        }
        "amount" => {
            if sort_dir == "asc" {
                query.order_by_asc(commission_transactions::Column::Amount)
            } else {
                query.order_by_desc(commission_transactions::Column::Amount)
            }
        }
        "created_at" => {
            if sort_dir == "asc" {
                query.order_by_asc(commission_transactions::Column::CreatedAt)
            } else {
                query.order_by_desc(commission_transactions::Column::CreatedAt)
            }
        }
        _ => query.order_by_desc(commission_transactions::Column::CreatedAt),
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
        // Temsilci bilgilerini al
        let representative = company_representatives::Entity::find_by_id(transaction.representative_id)
            .one(db)
            .await?;
        
        let (representative_name, company_name) = if let Some(rep) = representative {
            let user = crate::modules::auth::models::User::find_by_id(rep.user_id)
                .one(db)
                .await?;
            let user_name = user.map(|u| {
                format!(
                    "{} {}",
                    u.first_name.unwrap_or_default(),
                    u.last_name.unwrap_or_default()
                )
                .trim()
                .to_string()
            }).unwrap_or_else(|| "Bilinmeyen".to_string());

            let company = companies::Entity::find_by_id(rep.company_id)
                .one(db)
                .await?;
            let company_name = company
                .map(|c| c.company_name)
                .unwrap_or_else(|| "Bilinmeyen Şirket".to_string());

            (user_name, company_name)
        } else {
            ("Bilinmeyen".to_string(), "Bilinmeyen Şirket".to_string())
        };

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
            "earned" => "Komisyon Kazanıldı",
            "payment" => "Komisyon Ödendi",
            "adjustment" => "Düzeltme",
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

        responses.push(CommissionTransactionResponse {
            id: transaction.id,
            representative_id: transaction.representative_id,
            representative_name,
            company_id: transaction.company_id,
            company_name,
            cart_id: transaction.cart_id,
            order_id,
            transaction_type: transaction.transaction_type.clone(),
            transaction_type_display,
            amount: format_price(
                transaction.amount.to_f64().unwrap_or(0.0),
                &transaction.currency,
            ),
            amount_raw: transaction.amount.to_f64().unwrap_or(0.0),
            order_amount: transaction.order_amount.map(|a| {
                format_price(a.to_f64().unwrap_or(0.0), &transaction.currency)
            }),
            commission_rate: transaction
                .commission_rate
                .map(|r| format!("%{}", r)),
            currency: transaction.currency.clone(),
            balance_before: format_price(
                transaction.balance_before.to_f64().unwrap_or(0.0),
                &transaction.currency,
            ),
            balance_after: format_price(
                transaction.balance_after.to_f64().unwrap_or(0.0),
                &transaction.currency,
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

/// Temsilci komisyon özetini getir
pub async fn get_representative_commission_summary(
    db: &DatabaseConnection,
    representative_id: i64,
) -> Result<RepresentativeCommissionSummary, DbErr> {
    let representative = company_representatives::Entity::find_by_id(representative_id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "Representative not found".to_string(),
        ))?;

    let currency = "TRY"; // Varsayılan para birimi

    // Temsilci bilgilerini al
    let user = crate::modules::auth::models::User::find_by_id(representative.user_id)
        .one(db)
        .await?;
    let representative_name = user
        .as_ref()
        .map(|u| {
            format!(
                "{} {}",
                u.first_name.as_deref().unwrap_or(""),
                u.last_name.as_deref().unwrap_or("")
            )
            .trim()
            .to_string()
        })
        .unwrap_or_else(|| "Bilinmeyen".to_string());
    let representative_email = user.map(|u| u.email);

    // Şirket bilgilerini al
    let company = companies::Entity::find_by_id(representative.company_id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Company not found".to_string()))?;

    let commission_rate_f64 = representative.commission_rate.to_f64().unwrap_or(0.0);
    let accumulated_commission_f64 = representative
        .accumulated_commission
        .to_f64()
        .unwrap_or(0.0);
    let total_sales_amount_f64 = representative.total_sales_amount.to_f64().unwrap_or(0.0);

    // Toplam kazanılan komisyon (earned)
    let total_earned: Option<rust_decimal::Decimal> = commission_transactions::Entity::find()
        .filter(commission_transactions::Column::RepresentativeId.eq(representative_id))
        .filter(commission_transactions::Column::TransactionType.eq("earned"))
        .select_only()
        .column_as(commission_transactions::Column::Amount.sum(), "total")
        .into_tuple::<Option<rust_decimal::Decimal>>()
        .one(db)
        .await?
        .flatten();

    // Toplam ödenen komisyon (payment)
    let total_paid: Option<rust_decimal::Decimal> = commission_transactions::Entity::find()
        .filter(commission_transactions::Column::RepresentativeId.eq(representative_id))
        .filter(commission_transactions::Column::TransactionType.eq("payment"))
        .select_only()
        .column_as(commission_transactions::Column::Amount.sum(), "total")
        .into_tuple::<Option<rust_decimal::Decimal>>()
        .one(db)
        .await?
        .flatten();

    let total_earned_f64 = total_earned.unwrap_or_default().to_f64().unwrap_or(0.0);
    let total_paid_f64 = total_paid.unwrap_or_default().to_f64().unwrap_or(0.0);
    
    // Bekleyen komisyon = accumulated_commission (bu değer tüm işlemleri içerir: earned, payment, adjustment)
    let pending_commission = accumulated_commission_f64;

    Ok(RepresentativeCommissionSummary {
        representative_id,
        representative_name,
        representative_email,
        company_id: representative.company_id,
        company_name: company.company_name,
        commission_rate: format!("{}", commission_rate_f64),
        commission_rate_raw: commission_rate_f64,
        accumulated_commission: format_price(accumulated_commission_f64, currency),
        accumulated_commission_raw: accumulated_commission_f64,
        total_sales_amount: format_price(total_sales_amount_f64, currency),
        total_sales_amount_raw: total_sales_amount_f64,
        currency: currency.to_string(),
        total_earned: format_price(total_earned_f64, currency),
        total_paid: format_price(total_paid_f64, currency),
        pending_commission: format_price(pending_commission, currency),
        pending_commission_raw: pending_commission,
    })
}

/// Manuel komisyon ödemesi (admin)
pub async fn create_manual_commission_payment(
    db: &DatabaseConnection,
    request: CreateCommissionPaymentRequest,
    admin_user_id: i64,
) -> Result<commission_transactions::Model, commission_service::CommissionServiceError> {
    commission_service::create_commission_payment(
        db,
        request.representative_id,
        request.amount,
        "TRY".to_string(), // Default currency
        request.reference_number,
        request.description,
        admin_user_id,
    )
    .await
}

/// Komisyon düzeltme (admin)
pub async fn create_commission_adjustment(
    db: &DatabaseConnection,
    request: CreateCommissionAdjustmentRequest,
    admin_user_id: i64,
) -> Result<commission_transactions::Model, commission_service::CommissionServiceError> {
    commission_service::create_commission_adjustment(
        db,
        request.representative_id,
        request.amount,
        "TRY".to_string(), // Default currency
        request.description,
        admin_user_id,
    )
    .await
}

/// Tüm temsilcileri listele
pub async fn list_representatives(
    db: &DatabaseConnection,
    page: u64,
    per_page: u64,
    is_active: Option<bool>,
) -> Result<(Vec<RepresentativeListResponse>, u64), DbErr> {
    let mut query = company_representatives::Entity::find();

    if let Some(active) = is_active {
        query = query.filter(company_representatives::Column::IsActive.eq(active));
    }

    let paginator = query
        .order_by_desc(company_representatives::Column::CreatedAt)
        .paginate(db, per_page);

    let total = paginator.num_items().await?;
    let representatives = paginator.fetch_page(page - 1).await?;

    let mut results = Vec::new();
    for rep in representatives {
        // User bilgilerini al
        let user = crate::modules::auth::models::User::find_by_id(rep.user_id)
            .one(db)
            .await?;
        let (user_name, user_email) = if let Some(u) = user {
            let name = format!(
                "{} {}",
                u.first_name.unwrap_or_default(),
                u.last_name.unwrap_or_default()
            )
            .trim()
            .to_string();
            (name, u.email)
        } else {
            ("Bilinmeyen".to_string(), "".to_string())
        };

        // Company bilgilerini al
        let company = companies::Entity::find_by_id(rep.company_id)
            .one(db)
            .await?;
        let company_name = company
            .map(|c| c.company_name)
            .unwrap_or_else(|| "Bilinmeyen Şirket".to_string());

        let currency = "TRY";
        results.push(RepresentativeListResponse {
            id: rep.id,
            user_id: rep.user_id,
            user_name,
            user_email,
            company_id: rep.company_id,
            company_name,
            commission_rate: rep.commission_rate,
            commission_rate_formatted: format!("%{}", rep.commission_rate),
            accumulated_commission: rep.accumulated_commission,
            accumulated_commission_formatted: format_price(
                rep.accumulated_commission.to_f64().unwrap_or(0.0),
                currency,
            ),
            total_sales_amount: rep.total_sales_amount,
            total_sales_amount_formatted: format_price(
                rep.total_sales_amount.to_f64().unwrap_or(0.0),
                currency,
            ),
            is_active: rep.is_active,
            created_at: rep.created_at.to_string(),
        });
    }

    Ok((results, total))
}

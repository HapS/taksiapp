use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, DatabaseConnection, Set, PaginatorTrait};

use crate::modules::ecommerce::models::{
    campaign, coupon, CampaignModel, CouponModel,
};

use crate::modules::ecommerce::campaign::dto::{CreateCampaignRequest, UpdateCampaignRequest, CreateCouponRequest, CampaignListQuery};
use crate::modules::ecommerce::campaign::errors::CampaignError;
use crate::modules::ecommerce::campaign::scenario::ScenarioType;

pub async fn create_campaign(
    db: &DatabaseConnection,
    req: CreateCampaignRequest,
) -> Result<CampaignModel, CampaignError> {
    validate_params(&req.scenario_type, &req.params)?;

    // Adım 1: Tarih validasyonu (Başlangıç < Bitiş)
    if let (Some(starts), Some(ends)) = (req.starts_at, req.ends_at) {
        if starts >= ends {
            return Err(CampaignError::InvalidParams("Bitiş tarihi başlangıç tarihinden sonra olmalıdır".into()));
        }
    }

    let active_model = campaign::ActiveModel {
        name: Set(req.name),
        description: Set(req.description),
        scenario_type: Set(req.scenario_type.to_string()),
        params: Set(req.params),
        campaign_type: Set(req.campaign_type),
        starts_at: Set(req.starts_at.map(|dt| dt.into())),
        ends_at: Set(req.ends_at.map(|dt| dt.into())),
        is_active: Set(true),
        priority: Set(req.priority.unwrap_or(0)),
        stackable: Set(req.stackable.unwrap_or(false)),
        max_uses: Set(req.max_uses),
        max_uses_per_user: Set(req.max_uses_per_user),
        target_cart_type: Set(req.target_cart_type),
        usage_count: Set(0),
        ..Default::default()
    };

    let result = campaign::Entity::insert(active_model)
        .exec(db)
        .await
        .map_err(CampaignError::from)?;

    let model = campaign::Entity::find_by_id(result.last_insert_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    Ok(model)
}

pub async fn update_campaign(
    db: &DatabaseConnection,
    campaign_id: i64,
    req: UpdateCampaignRequest,
) -> Result<CampaignModel, CampaignError> {
    let model = campaign::Entity::find_by_id(campaign_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    let mut active: campaign::ActiveModel = model.into();

    if let Some(name) = req.name {
        active.name = Set(name);
    }
    if let Some(description) = req.description {
        active.description = Set(Some(description));
    }
    if let Some(params) = req.params {
        let scenario_str = active.scenario_type.as_ref().clone();
        let scenario_type = ScenarioType::all()
            .into_iter()
            .find(|s| s.as_str() == scenario_str)
            .ok_or(CampaignError::InvalidParams("Invalid scenario_type".to_string()))?;
        validate_params(&scenario_type, &params)?;
        active.params = Set(params);
    }
    if let Some(is_active) = req.is_active {
        active.is_active = Set(is_active);
    }
    if let Some(starts_at) = req.starts_at {
        active.starts_at = Set(Some(starts_at.into()));
    }
    if let Some(ends_at) = req.ends_at {
        active.ends_at = Set(Some(ends_at.into()));
    }

    // Güncelleme sonrası tarih validasyonu
    let starts = match active.starts_at.as_ref() {
        Some(dt) => Some(dt.clone()),
        None => None,
    };
    let ends = match active.ends_at.as_ref() {
        Some(dt) => Some(dt.clone()),
        None => None,
    };
    if let (Some(s), Some(e)) = (starts, ends) {
        if s >= e {
            return Err(CampaignError::InvalidParams("Bitiş tarihi başlangıç tarihinden sonra olmalıdır".into()));
        }
    }
    if let Some(priority) = req.priority {
        active.priority = Set(priority);
    }
    if let Some(stackable) = req.stackable {
        active.stackable = Set(stackable);
    }
    if let Some(max_uses) = req.max_uses {
        active.max_uses = Set(Some(max_uses));
    }
    if let Some(max_uses_per_user) = req.max_uses_per_user {
        active.max_uses_per_user = Set(Some(max_uses_per_user));
    }
    if let Some(target_cart_type) = req.target_cart_type {
        active.target_cart_type = Set(target_cart_type);
    }

    let updated = active.update(db).await.map_err(CampaignError::from)?;
    Ok(updated)
}

pub async fn delete_campaign(
    db: &DatabaseConnection,
    campaign_id: i64,
) -> Result<(), CampaignError> {
    let model = campaign::Entity::find_by_id(campaign_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    let mut active: campaign::ActiveModel = model.into();
    active.is_active = Set(false);
    active.update(db).await.map_err(CampaignError::from)?;

    Ok(())
}

pub async fn get_campaign(
    db: &DatabaseConnection,
    campaign_id: i64,
) -> Result<CampaignModel, CampaignError> {
    campaign::Entity::find_by_id(campaign_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)
}

pub async fn list_campaigns(
    db: &DatabaseConnection,
    query: CampaignListQuery,
) -> Result<(Vec<CampaignModel>, u64), CampaignError> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);

    let mut q = campaign::Entity::find();

    if let Some(is_active) = query.is_active {
        q = q.filter(campaign::Column::IsActive.eq(is_active));
    }
    if let Some(scenario_type) = &query.scenario_type {
        q = q.filter(campaign::Column::ScenarioType.eq(scenario_type.as_str()));
    }
    if let Some(campaign_type) = &query.campaign_type {
        q = q.filter(campaign::Column::CampaignType.eq(campaign_type.as_str()));
    }

    let paginator = q
        .order_by_desc(campaign::Column::Priority)
        .paginate(db, per_page);

    let total = paginator.num_items().await.map_err(CampaignError::from)?;
    let items = paginator
        .fetch_page(page.saturating_sub(1))
        .await
        .map_err(CampaignError::from)?;

    Ok((items, total))
}

pub async fn create_coupons(
    db: &DatabaseConnection,
    campaign_id: i64,
    req: CreateCouponRequest,
) -> Result<Vec<CouponModel>, CampaignError> {
    let campaign = campaign::Entity::find_by_id(campaign_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    if campaign.campaign_type != campaign::campaign_type::COUPON {
        return Err(CampaignError::InvalidParams("Kupon oluşturmak için kampanya tipi 'coupon' olmalıdır".into()));
    }

    if req.codes.is_empty() {
        return Err(CampaignError::InvalidParams("En az bir kupon kodu belirtilmelidir".into()));
    }

    let mut coupons = Vec::new();

    for code in req.codes {
        let active_model = coupon::ActiveModel {
            campaign_id: Set(campaign_id),
            code: Set(code.to_uppercase()),
            user_id: Set(req.user_id),
            max_usage: Set(req.max_usage),
            usage_count: Set(0),
            valid_until: Set(req.valid_until.map(|dt| dt.into())),
            is_active: Set(true),
            ..Default::default()
        };

        let result = coupon::Entity::insert(active_model)
            .exec(db)
            .await
            .map_err(CampaignError::from)?;

        let coupon = coupon::Entity::find_by_id(result.last_insert_id)
            .one(db)
            .await
            .map_err(CampaignError::from)?
            .ok_or(CampaignError::NotFound)?;

        coupons.push(coupon);
    }

    Ok(coupons)
}

pub async fn generate_coupons(
    db: &DatabaseConnection,
    campaign_id: i64,
    codes: Vec<String>,
    req: &crate::modules::ecommerce::campaign::dto::GenerateCouponsRequest,
) -> Result<Vec<CouponModel>, CampaignError> {
    let campaign = campaign::Entity::find_by_id(campaign_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    if campaign.campaign_type != campaign::campaign_type::COUPON {
        return Err(CampaignError::InvalidParams("Kupon oluşturmak için kampanya tipi 'coupon' olmalıdır".into()));
    }

    if codes.is_empty() {
        return Err(CampaignError::InvalidParams("Üretilecek kupon kodu bulunamadı".into()));
    }

    let mut coupons = Vec::new();
    for code in codes {
        let active_model = coupon::ActiveModel {
            campaign_id: Set(campaign_id),
            code: Set(code),
            user_id: Set(req.user_id),
            max_usage: Set(req.max_usage),
            usage_count: Set(0),
            valid_until: Set(req.valid_until.map(|dt| dt.into())),
            is_active: Set(true),
            ..Default::default()
        };

        let result = coupon::Entity::insert(active_model)
            .exec(db)
            .await
            .map_err(CampaignError::from)?;

        let coupon = coupon::Entity::find_by_id(result.last_insert_id)
            .one(db)
            .await
            .map_err(CampaignError::from)?
            .ok_or(CampaignError::NotFound)?;

        coupons.push(coupon);
    }

    Ok(coupons)
}

pub async fn list_coupons(
    db: &DatabaseConnection,
    campaign_id: i64,
) -> Result<Vec<CouponModel>, CampaignError> {
    coupon::Entity::find()
        .filter(coupon::Column::CampaignId.eq(campaign_id))
        .all(db)
        .await
        .map_err(CampaignError::from)
}

pub async fn delete_coupon(
    db: &DatabaseConnection,
    coupon_id: i64,
) -> Result<(), CampaignError> {
    let model = coupon::Entity::find_by_id(coupon_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    let mut active: coupon::ActiveModel = model.into();
    active.is_active = Set(false);
    active.update(db).await.map_err(CampaignError::from)?;

    Ok(())
}

pub async fn update_coupon(
    db: &DatabaseConnection,
    coupon_id: i64,
    is_active: Option<bool>,
    max_usage: Option<i32>,
    valid_until: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<CouponModel, CampaignError> {
    let model = coupon::Entity::find_by_id(coupon_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    let mut active: coupon::ActiveModel = model.into();
    if let Some(ia) = is_active {
        active.is_active = Set(ia);
    }
    if let Some(mu) = max_usage {
        active.max_usage = Set(Some(mu));
    }
    if let Some(vu) = valid_until {
        active.valid_until = Set(Some(vu.into()));
    }

    let updated = active.update(db).await.map_err(CampaignError::from)?;
    Ok(updated)
}

pub async fn get_campaign_stats(
    db: &DatabaseConnection,
    campaign_id: i64,
) -> Result<crate::modules::ecommerce::campaign::dto::CampaignStatsResponse, CampaignError> {
    use crate::modules::ecommerce::models::{campaign_usage, cart_item, cart_discount, cart};
    use crate::modules::content::models::content;
    use sea_orm::{JoinType, QuerySelect, EntityTrait, ColumnTrait, QueryFilter, RelationTrait};
    use sea_orm::sea_query::Expr;

    let campaign = campaign::Entity::find_by_id(campaign_id)
        .one(db)
        .await
        .map_err(CampaignError::from)?
        .ok_or(CampaignError::NotFound)?;

    // 1. Kullanım Sayısı
    let usage_count = campaign.usage_count as u64;

    // 2. Top Ürünler (Bu kampanyanın kullanıldığı sepetlerdeki ürünler)
    let cart_ids_query = campaign_usage::Entity::find()
        .select_only()
        .column(campaign_usage::Column::CartId)
        .filter(campaign_usage::Column::CampaignId.eq(campaign_id))
        .into_tuple::<i64>()
        .all(db)
        .await
        .map_err(CampaignError::from)?;

    let mut top_products = Vec::new();

    if !cart_ids_query.is_empty() {
        let top_products_data = cart_item::Entity::find()
            .select_only()
            .column(cart_item::Column::ProductId)
            .column_as(cart_item::Column::ProductId.count(), "usage_count")
            .filter(cart_item::Column::CartId.is_in(cart_ids_query))
            .group_by(cart_item::Column::ProductId)
            .order_by_desc(Expr::cust("usage_count"))
            .limit(5)
            .into_tuple::<(i64, i64)>()
            .all(db)
            .await
            .map_err(CampaignError::from)?;

        for (product_id, count) in top_products_data {
            let product_title = content::Entity::find_by_id(product_id)
                .one(db)
                .await
                .ok()
                .flatten()
                .and_then(|p| {
                    p.data.get("langs")
                        .and_then(|l| l.as_object())
                        .and_then(|o| o.values().next())
                        .and_then(|v| v.get("title"))
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                });

            top_products.push(crate::modules::ecommerce::campaign::dto::TopProductInfo {
                product_id,
                product_title,
                usage_count: count as u64,
            });
        }
    }

    // 3. Verilen Toplam İndirim (Para birimine göre gruplayarak doğru hesapla)
    let total_discounts_by_currency = cart_discount::Entity::find()
        .select_only()
        .column(cart_discount::Column::Currency)
        .column_as(cart_discount::Column::Amount.sum(), "total_amount")
        .join(JoinType::InnerJoin, cart_discount::Relation::Cart.def())
        .filter(cart_discount::Column::CampaignId.eq(campaign_id))
        .filter(cart::Column::CompletedAt.is_not_null())
        .group_by(cart_discount::Column::Currency)
        .into_tuple::<(String, Option<rust_decimal::Decimal>)>()
        .all(db)
        .await
        .map_err(CampaignError::from)?;

    let total_discount_given = if total_discounts_by_currency.is_empty() {
        "0.00 TRY".to_string()
    } else {
        total_discounts_by_currency
            .into_iter()
            .map(|(curr, amt)| format!("{:.2} {}", amt.unwrap_or_default(), curr))
            .collect::<Vec<_>>()
            .join(", ")
    };
    
    Ok(crate::modules::ecommerce::campaign::dto::CampaignStatsResponse {
        usage_count,
        total_discount_given,
        top_products,
    })
}

fn validate_params(
    scenario_type: &ScenarioType,
    params: &serde_json::Value,
) -> Result<(), CampaignError> {
    match scenario_type {
        ScenarioType::BuyXGetYFree => {
            serde_json::from_value::<crate::modules::ecommerce::campaign::scenario::BuyXGetYFreeParams>(params.clone())
                .map_err(|e| CampaignError::InvalidParams(e.to_string()))?;
        }
        ScenarioType::QuantityDiscountPercent => {
            serde_json::from_value::<crate::modules::ecommerce::campaign::scenario::QuantityDiscountPercentParams>(params.clone())
                .map_err(|e| CampaignError::InvalidParams(e.to_string()))?;
        }
        ScenarioType::CategorySpendGetDiscount => {
            serde_json::from_value::<crate::modules::ecommerce::campaign::scenario::CategorySpendGetDiscountParams>(params.clone())
                .map_err(|e| CampaignError::InvalidParams(e.to_string()))?;
        }
        ScenarioType::CartTotalDiscount => {
            serde_json::from_value::<crate::modules::ecommerce::campaign::scenario::CartTotalDiscountParams>(params.clone())
                .map_err(|e| CampaignError::InvalidParams(e.to_string()))?;
        }
        ScenarioType::CouponCode => {
            serde_json::from_value::<crate::modules::ecommerce::campaign::scenario::CouponCodeParams>(params.clone())
                .map_err(|e| CampaignError::InvalidParams(e.to_string()))?;
        }
        ScenarioType::FreeShipping => {
            serde_json::from_value::<crate::modules::ecommerce::campaign::scenario::FreeShippingParams>(params.clone())
                .map_err(|e| CampaignError::InvalidParams(e.to_string()))?;
        }
        ScenarioType::FirstOrderDiscount => {
            serde_json::from_value::<crate::modules::ecommerce::campaign::scenario::FirstOrderDiscountParams>(params.clone())
                .map_err(|e| CampaignError::InvalidParams(e.to_string()))?;
        }
    }
    Ok(())
}
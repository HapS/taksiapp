// Admin Dashboard Web Controller - HTML pages
use crate::app_state::AppState;
use crate::modules::auth::models::{user::Column as UserColumn, User};
use crate::modules::content::models::{content::Column as ContentColumn, Content};
use crate::modules::ecommerce::models::cart::{status, Column as CartColumn, Entity as CartEntity};
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use tera::Context;
use tower_sessions::Session;

#[derive(Deserialize, Serialize, Debug)]
pub struct FilterParams {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

// Use common RBAC helper
use crate::modules::admin::services::settings_service::get_sale_currency;
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use crate::modules::currency::services::exchange_rate_service::{
    convert_currency, get_cached_rates, get_rates_at_date,
};
use crate::modules::utils::format_price::format_price;

#[derive(Serialize)]
struct DashboardStats {
    // Content stats
    total_contents: u64,
    total_pages: u64,
    total_blogs: u64,
    total_news: u64,
    total_products: u64,
    published_contents: u64,
    draft_contents: u64,

    // User stats
    total_users: u64,
    registered_users: u64,
    guest_users: u64,
    new_users_today: u64,
    new_users_week: u64,

    // Order stats (from carts table)
    total_orders: u64,
    pending_orders: u64,
    confirmed_orders: u64,
    preparing_orders: u64,
    shipped_orders: u64,
    delivered_orders: u64,
    cancelled_orders: u64,
    orders_today: u64,
    orders_week: u64,

    // Cart stats
    active_carts: u64,
    abandoned_carts: u64,

    // Revenue stats (formatted strings)
    total_revenue: String,
    revenue_today: String,
    revenue_week: String,
    revenue_month: String,
}

#[derive(Serialize)]
struct RecentOrder {
    id: i64,
    order_id: String,
    user_email: Option<String>,
    total_amount: String,
    status: String,
    created_at: String,
}

/// Admin Dashboard - Main admin page with statistics
pub async fn dashboard(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<FilterParams>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    // Get settings and currency info
    let sale_currency = get_sale_currency(&state.db)
        .await
        .unwrap_or_else(|| "TRY".to_string());
    let rates = get_cached_rates(&state.db).await;

    // bugün
    let tr_offset = chrono::FixedOffset::east_opt(3 * 3600).unwrap();
    let now = Utc::now();
    let now_tr = now.with_timezone(&tr_offset);
    let today_start = now_tr
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(tr_offset)
        .unwrap()
        .with_timezone(&Utc);

    println!("Now TR: {}", now_tr);
    println!("Today Start: {}", today_start);

    let week_start = now - Duration::days(7);
    let month_start = now - Duration::days(30);

    // Parse filter dates
    let filter_start = params
        .start_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());

    let filter_end = params
        .end_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    println!(
        "📊 Dashboard filters: start={:?}, end={:?}",
        filter_start, filter_end
    );

    // Date range for queries - if not provided, show total/default
    let query_start = filter_start;
    let query_end = filter_end;

    // Helper to apply date filter to Select
    fn apply_date_filter<E>(
        query: Select<E>,
        column: impl ColumnTrait,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Select<E>
    where
        E: EntityTrait,
    {
        let mut q = query;
        if let Some(s) = start {
            q = q.filter(column.gte(s));
        }
        if let Some(e) = end {
            q = q.filter(column.lte(e));
        }
        q
    }

    // Content Statistics
    let total_contents_query = apply_date_filter(
        Content::find(),
        ContentColumn::CreatedAt,
        query_start,
        query_end,
    );
    let total_contents = total_contents_query.count(&state.db).await.unwrap_or(0);

    let total_pages = apply_date_filter(
        Content::find().filter(ContentColumn::ContentType.eq("page")),
        ContentColumn::CreatedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);
    let total_blogs = apply_date_filter(
        Content::find().filter(ContentColumn::ContentType.eq("blog")),
        ContentColumn::CreatedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);
    let total_news = apply_date_filter(
        Content::find().filter(ContentColumn::ContentType.eq("news")),
        ContentColumn::CreatedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);
    let total_products = apply_date_filter(
        Content::find().filter(ContentColumn::ContentType.eq("product")),
        ContentColumn::CreatedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);
    let published_contents = apply_date_filter(
        Content::find().filter(ContentColumn::Publish.eq(true)),
        ContentColumn::CreatedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);
    let draft_contents = total_contents.saturating_sub(published_contents);

    // User Statistics
    let total_users =
        apply_date_filter(User::find(), UserColumn::CreatedAt, query_start, query_end)
            .count(&state.db)
            .await
            .unwrap_or(0);
    let registered_users = apply_date_filter(
        User::find().filter(UserColumn::IsGuest.eq(false)),
        UserColumn::CreatedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);
    let guest_users = total_users.saturating_sub(registered_users);

    let new_users_today = User::find()
        .filter(UserColumn::CreatedAt.gte(today_start))
        .count(&state.db)
        .await
        .unwrap_or(0);
    let new_users_week = User::find()
        .filter(UserColumn::CreatedAt.gte(week_start))
        .count(&state.db)
        .await
        .unwrap_or(0);

    // Order Statistics (from carts with status != open_cart)
    // For orders, we should look at order_date or completed_at, not when the cart was first created
    let total_orders = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.ne(status::OPEN_CART)),
        CartColumn::OrderDate, // Use OrderDate for general order counts
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);

    let pending_orders = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.eq(status::PENDING)),
        CartColumn::CreatedAt, // Pending orders might not have order_date yet, or use CreatedAt
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);

    let confirmed_orders = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.eq(status::CONFIRMED)),
        CartColumn::CompletedAt, // Confirmed orders should be counted when they were completed
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);

    println!("✅ Confirmed orders count: {}", confirmed_orders);

    let preparing_orders = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.eq(status::PREPARING)),
        CartColumn::CompletedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);

    let shipped_orders = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.eq(status::SHIPPED)),
        CartColumn::CompletedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);

    let delivered_orders = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.eq(status::DELIVERED)),
        CartColumn::CompletedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);

    let cancelled_orders = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.eq(status::CANCELLED)),
        CartColumn::CompletedAt,
        query_start,
        query_end,
    )
    .count(&state.db)
    .await
    .unwrap_or(0);

    let orders_today = CartEntity::find()
        .filter(CartColumn::Status.ne(status::OPEN_CART))
        .filter(CartColumn::OrderDate.gte(today_start))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let orders_week = CartEntity::find()
        .filter(CartColumn::Status.ne(status::OPEN_CART))
        .filter(CartColumn::OrderDate.gte(week_start))
        .count(&state.db)
        .await
        .unwrap_or(0);

    // Cart Statistics (only open carts)
    let active_carts = CartEntity::find()
        .filter(CartColumn::Status.eq(status::OPEN_CART))
        .filter(CartColumn::UpdatedAt.gte(week_start))
        .count(&state.db)
        .await
        .unwrap_or(0);

    let abandoned_carts = CartEntity::find()
        .filter(CartColumn::Status.eq(status::OPEN_CART))
        .filter(CartColumn::UpdatedAt.lt(week_start))
        .count(&state.db)
        .await
        .unwrap_or(0);

    // Revenue calculations (from completed orders) - using historical exchange rates
    let completed_statuses = vec![status::DELIVERED, status::CONFIRMED];

    // Helper to sum carts using historical rates
    async fn sum_with_historical_rates(
        db: &DatabaseConnection,
        carts: Vec<crate::modules::ecommerce::models::cart::Model>,
        target_currency: &str,
        default_rates: &Option<crate::modules::currency::models::ExchangeRateModel>,
    ) -> f64 {
        let mut total = 0.0;
        for cart in carts {
            let amount = cart
                .total_amount
                .as_ref()
                .map(|a| a.to_string().parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);

            let cart_currency = cart.currency.as_deref().unwrap_or("TRY");
            if cart_currency == target_currency {
                total += amount;
                continue;
            }

            // Find rate at the time of order/completion
            let target_time = cart.completed_at.or(cart.order_date).or(cart.created_at);
            let rate_to_use = if let Some(dt) = target_time {
                get_rates_at_date(db, dt.into()).await.unwrap_or(None)
            } else {
                None
            };

            // Fallback to latest rates if historical not found
            let final_rate = rate_to_use.or(default_rates.clone());

            if let Some(r) = final_rate {
                total +=
                    convert_currency(amount, cart_currency, target_currency, &r).unwrap_or(amount);
            } else {
                total += amount;
            }
        }
        total
    }

    let all_orders_revenue_query = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.is_in(completed_statuses.clone())),
        CartColumn::CompletedAt,
        query_start,
        query_end,
    );
    let all_orders_revenue = all_orders_revenue_query
        .all(&state.db)
        .await
        .unwrap_or_default();
    let total_revenue =
        sum_with_historical_rates(&state.db, all_orders_revenue, &sale_currency, &rates).await;

    let orders_today_revenue = CartEntity::find()
        .filter(CartColumn::Status.is_in(completed_statuses.clone()))
        .filter(CartColumn::CompletedAt.gte(today_start))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let revenue_today =
        sum_with_historical_rates(&state.db, orders_today_revenue, &sale_currency, &rates).await;

    let orders_week_revenue = CartEntity::find()
        .filter(CartColumn::Status.is_in(completed_statuses.clone()))
        .filter(CartColumn::CompletedAt.gte(week_start))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let revenue_week =
        sum_with_historical_rates(&state.db, orders_week_revenue, &sale_currency, &rates).await;

    let orders_month_revenue = CartEntity::find()
        .filter(CartColumn::Status.is_in(completed_statuses))
        .filter(CartColumn::CompletedAt.gte(month_start))
        .all(&state.db)
        .await
        .unwrap_or_default();
    let revenue_month =
        sum_with_historical_rates(&state.db, orders_month_revenue, &sale_currency, &rates).await;

    let stats = DashboardStats {
        total_contents,
        total_pages,
        total_blogs,
        total_news,
        total_products,
        published_contents,
        draft_contents,
        total_users,
        registered_users,
        guest_users,
        new_users_today,
        new_users_week,
        total_orders,
        pending_orders,
        confirmed_orders,
        preparing_orders,
        shipped_orders,
        delivered_orders,
        cancelled_orders,
        orders_today,
        orders_week,
        active_carts,
        abandoned_carts,
        total_revenue: format_price(total_revenue, &sale_currency),
        revenue_today: format_price(revenue_today, &sale_currency),
        revenue_week: format_price(revenue_week, &sale_currency),
        revenue_month: format_price(revenue_month, &sale_currency),
    };

    // Recent pages
    let recent_pages_query = apply_date_filter(
        Content::find(),
        ContentColumn::CreatedAt,
        query_start,
        query_end,
    );
    let recent_pages_raw = recent_pages_query
        .order_by_desc(ContentColumn::CreatedAt)
        .limit(5)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let config = crate::config::get_config();
    let mut recent_pages = Vec::new();
    for p in recent_pages_raw {
        recent_pages.push(
            crate::modules::content::helpers::page_helper::to_page_response(
                &p,
                &config.default_language,
                &state.db,
            )
            .await,
        );
    }

    // Recent orders (from carts)
    let recent_orders_query = apply_date_filter(
        CartEntity::find().filter(CartColumn::Status.ne(status::OPEN_CART)),
        CartColumn::CreatedAt,
        query_start,
        query_end,
    );
    let recent_orders_raw = recent_orders_query
        .order_by_desc(CartColumn::CreatedAt)
        .limit(5)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut recent_orders = Vec::new();
    for order in recent_orders_raw {
        // Get user email
        let user_email =
            if let Ok(Some(user)) = User::find_by_id(order.user_id).one(&state.db).await {
                Some(user.email)
            } else {
                None
            };

        recent_orders.push(RecentOrder {
            id: order.id,
            order_id: order.order_id.unwrap_or_else(|| format!("#{}", order.id)),
            user_email,
            total_amount: {
                let amount = order
                    .total_amount
                    .as_ref()
                    .map(|a| a.to_string().parse::<f64>().unwrap_or(0.0))
                    .unwrap_or(0.0);

                let order_currency = order.currency.as_deref().unwrap_or("TRY");

                // Historical rate for this order
                let target_time = order.completed_at.or(order.order_date).or(order.created_at);
                let rate_to_use = if let Some(dt) = target_time {
                    get_rates_at_date(&state.db, dt.into())
                        .await
                        .unwrap_or(None)
                } else {
                    None
                };

                let final_rate = rate_to_use.or(rates.clone());

                let converted_amount = if let Some(ref r) = final_rate {
                    convert_currency(amount, order_currency, &sale_currency, r).unwrap_or(amount)
                } else {
                    amount
                };

                format_price(converted_amount, &sale_currency)
            },
            status: order.status.clone(),
            created_at: order
                .created_at
                .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string())
                .unwrap_or_default(),
        });
    }

    let mut context = Context::new();
    context.insert("title", "Admin Dashboard");
    context.insert("stats", &stats);
    context.insert("recent_pages", &recent_pages);
    context.insert("recent_orders", &recent_orders);
    context.insert("current_path", "/admin");
    context.insert("filters", &params); // Pass filters back to template

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/dashboard.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

use crate::app_state::AppState;
use crate::modules::auth::models::{user::Column as UserColumn, User};
use crate::modules::content::models::{content::Column as ContentColumn, Content};
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Duration, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use tera::Context;
use tower_sessions::Session;

#[derive(Deserialize, Serialize, Debug)]
pub struct FilterParams {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

#[derive(Serialize)]
struct DashboardStats {
    total_contents: u64,
    total_pages: u64,
    total_blogs: u64,
    total_news: u64,
    total_products: u64,
    published_contents: u64,
    draft_contents: u64,
    total_users: u64,
    registered_users: u64,
    guest_users: u64,
    new_users_today: u64,
    new_users_week: u64,
}

pub async fn dashboard(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<FilterParams>,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

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

    let week_start = now - Duration::days(7);

    let filter_start = params
        .start_date
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());

    let filter_end = params
        .end_date
        .as_deref()
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    let query_start = filter_start;
    let query_end = filter_end;

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
    };

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

    let mut context = Context::new();
    context.insert("title", "Admin Dashboard");
    context.insert("stats", &stats);
    context.insert("recent_pages", &recent_pages);
    context.insert("current_path", "/admin");
    context.insert("filters", &params);

    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/dashboard.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

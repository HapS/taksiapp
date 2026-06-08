use crate::app_state::AppState;
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

/// GET /admin/b2b/credit/transactions - Kredi işlem geçmişi sayfası
pub async fn credit_transactions_page(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "B2B Kredi İşlem Geçmişi");
    context.insert("page", "credit_transactions");
    context.insert("current_path", "/admin/b2b/credit/transactions");

    // Add user data to context
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/b2b/credit_transactions.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

/// GET /admin/b2b/credit/payment/:company_id - Manuel ödeme kaydetme sayfası
pub async fn credit_payment_page(
    State(state): State<AppState>,
    session: Session,
    Path(company_id): Path<i64>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Manuel Ödeme Kaydet");
    context.insert("page", "credit_payment");
    context.insert("current_path", "/admin/b2b/credit/payment");

    // Şirketin var olduğunu kontrol et
    use crate::modules::b2b::entities::companies;
    use sea_orm::EntityTrait;

    match companies::Entity::find_by_id(company_id)
        .one(&state.db)
        .await
    {
        Ok(Some(_)) => {
            context.insert("company_id", &company_id);
        }
        Ok(None) => {
            context.insert("error", "Şirket bulunamadı");
        }
        Err(_) => {
            context.insert("error", "Veritabanı hatası");
        }
    }

    // Add user data to context
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/b2b/credit_payment.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

/// GET /admin/b2b/credit/adjustment/:company_id - Kredi düzeltme sayfası
pub async fn credit_adjustment_page(
    State(state): State<AppState>,
    session: Session,
    Path(company_id): Path<i64>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Kredi Düzeltme");
    context.insert("page", "credit_adjustment");
    context.insert("current_path", "/admin/b2b/credit/adjustment");

    // Şirketin var olduğunu kontrol et
    use crate::modules::b2b::entities::companies;
    use sea_orm::EntityTrait;

    match companies::Entity::find_by_id(company_id)
        .one(&state.db)
        .await
    {
        Ok(Some(_)) => {
            context.insert("company_id", &company_id);
        }
        Ok(None) => {
            context.insert("error", "Şirket bulunamadı");
        }
        Err(_) => {
            context.insert("error", "Veritabanı hatası");
        }
    }

    // Add user data to context
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/b2b/credit_adjustment.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

/// GET /admin/credits - B2C User Credits management page
pub async fn user_credits_page(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "B2C User Credits");
    context.insert("page", "user_credits");
    context.insert("current_path", "/admin/credits");

    // Add user data to context
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/credits/list.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

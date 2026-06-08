use crate::app_state::AppState;
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use tera::Context;
use tower_sessions::Session;

/// GET /admin/b2b/commission/transactions - Komisyon işlem geçmişi sayfası
pub async fn commission_transactions_page(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Temsilci Komisyon İşlem Geçmişi");
    context.insert("page", "commission_transactions");
    context.insert("current_path", "/admin/b2b/commission/transactions");

    // Add user data to context
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/b2b/commission_transactions.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

/// GET /admin/b2b/commission/payment/:representative_id - Komisyon ödeme sayfası
pub async fn commission_payment_page(
    State(state): State<AppState>,
    session: Session,
    Path(representative_id): Path<i64>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Komisyon Ödemesi");
    context.insert("page", "commission_payment");
    context.insert("current_path", "/admin/b2b/commission/payment");

    // Temsilcinin var olduğunu kontrol et
    use crate::modules::b2b::entities::company_representatives;
    use sea_orm::EntityTrait;

    match company_representatives::Entity::find_by_id(representative_id)
        .one(&state.db)
        .await
    {
        Ok(Some(_)) => {
            context.insert("representative_id", &representative_id);
        }
        Ok(None) => {
            // Temsilci bulunamadı - hata sayfası
            context.insert("error", "Temsilci bulunamadı");
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

    match state.render_template("admin/b2b/commission_payment.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

/// GET /admin/b2b/commission/adjustment/:representative_id - Komisyon düzeltme sayfası
pub async fn commission_adjustment_page(
    State(state): State<AppState>,
    session: Session,
    Path(representative_id): Path<i64>,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Komisyon Düzeltme");
    context.insert("page", "commission_adjustment");
    context.insert("current_path", "/admin/b2b/commission/adjustment");

    // Temsilcinin var olduğunu kontrol et
    use crate::modules::b2b::entities::company_representatives;
    use sea_orm::EntityTrait;

    match company_representatives::Entity::find_by_id(representative_id)
        .one(&state.db)
        .await
    {
        Ok(Some(_)) => {
            context.insert("representative_id", &representative_id);
        }
        Ok(None) => {
            // Temsilci bulunamadı - hata sayfası
            context.insert("error", "Temsilci bulunamadı");
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

    match state.render_template("admin/b2b/commission_adjustment.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

/// GET /admin/b2b/representatives - Temsilciler listesi sayfası
pub async fn representatives_list_page(
    State(state): State<AppState>,
    session: Session,
) -> Response {
    // Admin kontrolü
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Temsilciler");
    context.insert("page", "representatives");
    context.insert("current_path", "/admin/b2b/representatives");

    // Add user data to context
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/b2b/representatives.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(_) => Redirect::to("/admin").into_response(),
    }
}

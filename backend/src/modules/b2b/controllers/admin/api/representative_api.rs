use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::auth::helpers::rbac::check_admin_access_api;
use crate::modules::b2b::entities::{company_representatives, companies};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use rust_decimal::Decimal;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct CreateRepresentativeRequest {
    pub company_id: i64,
    pub user_id: i64,
    pub commission_rate: Decimal,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRepresentativeRequest {
    pub commission_rate: Decimal,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeResponse {
    pub id: i64,
    pub company_id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub user_email: String,
    pub commission_rate: String,
    pub is_active: bool,
}

/// GET /admin/api/b2b/companies/{company_id}/representatives - Şirketin temsilcilerini listele
pub async fn list_company_representatives(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(company_id): Path<i64>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    match get_company_representatives(&state.db, company_id).await {
        Ok(representatives) => (StatusCode::OK, Json(json!({ "data": representatives }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

/// POST /admin/api/b2b/representatives - Temsilci ekle
pub async fn create_representative(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<CreateRepresentativeRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Şirketin var olduğunu kontrol et
    let _company = match companies::Entity::find_by_id(request.company_id)
        .one(&state.db)
        .await
    {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Şirket bulunamadı" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {}", e) })),
            )
                .into_response()
        }
    };

    // Aynı kullanıcı zaten temsilci mi kontrol et
    let existing = company_representatives::Entity::find()
        .filter(company_representatives::Column::CompanyId.eq(request.company_id))
        .filter(company_representatives::Column::UserId.eq(request.user_id))
        .one(&state.db)
        .await;

    if let Ok(Some(_)) = existing {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Bu kullanıcı zaten bu şirketin temsilcisi" })),
        )
            .into_response();
    }

    // Yeni temsilci oluştur
    let new_rep = company_representatives::ActiveModel {
        company_id: Set(request.company_id),
        user_id: Set(request.user_id),
        commission_rate: Set(request.commission_rate),
        is_active: Set(request.is_active),
        accumulated_commission: Set(Decimal::ZERO),
        total_sales_amount: Set(Decimal::ZERO),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };

    match new_rep.insert(&state.db).await {
        Ok(rep) => {
            // Temsilci bilgilerini döndür
            match get_representative_by_id(&state.db, rep.id).await {
                Ok(Some(rep_data)) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "Temsilci eklendi",
                        "data": rep_data
                    })),
                )
                    .into_response(),
                _ => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "Temsilci eklendi"
                    })),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

/// PUT /admin/api/b2b/representatives/{id} - Temsilci güncelle
pub async fn update_representative(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
    Json(request): Json<UpdateRepresentativeRequest>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Temsilciyi bul
    let rep = match company_representatives::Entity::find_by_id(id)
        .one(&state.db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Temsilci bulunamadı" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {}", e) })),
            )
                .into_response()
        }
    };

    // Güncelle
    let mut active_model: company_representatives::ActiveModel = rep.into();
    active_model.commission_rate = Set(request.commission_rate);
    active_model.is_active = Set(request.is_active);
    active_model.updated_at = Set(chrono::Utc::now().into());

    match active_model.update(&state.db).await {
        Ok(_) => {
            match get_representative_by_id(&state.db, id).await {
                Ok(Some(rep_data)) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "Temsilci güncellendi",
                        "data": rep_data
                    })),
                )
                    .into_response(),
                _ => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "Temsilci güncellendi"
                    })),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

/// DELETE /admin/api/b2b/representatives/{id} - Temsilci sil
pub async fn delete_representative(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Admin kontrolü
    if let Err(response) = check_admin_access_api(&state, auth_user.id).await {
        return response;
    }

    // Temsilciyi bul
    let rep = match company_representatives::Entity::find_by_id(id)
        .one(&state.db)
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Temsilci bulunamadı" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error: {}", e) })),
            )
                .into_response()
        }
    };

    // Sil
    match rep.delete(&state.db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "Temsilci silindi"
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
            .into_response(),
    }
}

// Helper functions
async fn get_company_representatives(
    db: &DatabaseConnection,
    company_id: i64,
) -> Result<Vec<RepresentativeResponse>, DbErr> {
    use crate::modules::auth::models::user::Entity as User;

    let representatives = company_representatives::Entity::find()
        .filter(company_representatives::Column::CompanyId.eq(company_id))
        .find_also_related(User)
        .all(db)
        .await?;

    let mut result = Vec::new();
    for (rep, user_opt) in representatives {
        if let Some(user) = user_opt {
            result.push(RepresentativeResponse {
                id: rep.id,
                company_id: rep.company_id,
                user_id: rep.user_id,
                user_name: format!(
                    "{} {}",
                    user.first_name.unwrap_or_default(),
                    user.last_name.unwrap_or_default()
                )
                .trim()
                .to_string(),
                user_email: user.email,
                commission_rate: rep.commission_rate.to_string(),
                is_active: rep.is_active,
            });
        }
    }

    Ok(result)
}

async fn get_representative_by_id(
    db: &DatabaseConnection,
    id: i64,
) -> Result<Option<RepresentativeResponse>, DbErr> {
    use crate::modules::auth::models::user::Entity as User;

    let rep_with_user = company_representatives::Entity::find_by_id(id)
        .find_also_related(User)
        .one(db)
        .await?;

    match rep_with_user {
        Some((rep, Some(user))) => Ok(Some(RepresentativeResponse {
            id: rep.id,
            company_id: rep.company_id,
            user_id: rep.user_id,
            user_name: format!(
                "{} {}",
                user.first_name.unwrap_or_default(),
                user.last_name.unwrap_or_default()
            )
            .trim()
            .to_string(),
            user_email: user.email,
            commission_rate: rep.commission_rate.to_string(),
            is_active: rep.is_active,
        })),
        _ => Ok(None),
    }
}

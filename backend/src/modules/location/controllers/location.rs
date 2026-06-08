use crate::{
    app_state::AppState,
    modules::{
        auth::helpers::rbac::check_admin_access_api,
        location::{
            entities::locations::{self, Entity as Location},
            services::nominatim,
        },
    },
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect};
use sea_orm::sea_query::Expr;
use sea_orm::sea_query::extension::postgres::PgExpr;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public: Konum Arama (DB + Nominatim fallback)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_search_limit")]
    pub limit: Option<u32>,
}

fn default_search_limit() -> Option<u32> {
    Some(10)
}

#[derive(Debug, Serialize)]
struct LocationResult {
    id: Option<i64>,
    name: String,
    address: String,
    lat: f64,
    lon: f64,
    category: String,
    source: String,
}

/// GET /api/locations/search?q=...&limit=10
/// Public — JWT auth gerekli ama admin değil.
pub async fn search_locations(
    _claims: crate::middleware::jwt::JwtClaims,
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let query = params.q.trim();
    let limit = params.limit.unwrap_or(10).min(20) as u64;

    tracing::info!(query = %query, limit, "GET /api/locations/search");

    if query.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "empty query"}))).into_response();
    }

    let pattern = format!("%{query}%");
    let db_results = Location::find()
        .filter(locations::Column::IsActive.eq(true))
        .filter(
            sea_orm::Condition::any()
                .add(Expr::col(locations::Column::Name).ilike(&pattern))
                .add(Expr::col(locations::Column::Address).ilike(&pattern)),
        )
        .order_by_asc(locations::Column::Name)
        .limit(limit)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let db_count = db_results.len() as u32;
    let mut results: Vec<LocationResult> = db_results
        .iter()
        .map(|loc| LocationResult {
            id: Some(loc.id),
            name: loc.name.clone(),
            address: loc.address.clone(),
            lat: loc.lat,
            lon: loc.lon,
            category: loc.category.clone(),
            source: "db".to_string(),
        })
        .collect();

    if results.len() < limit as usize {
        let remaining = (limit as usize) - results.len();
        match nominatim::search_nominatim(query, remaining).await {
            Ok(geo_results) => {
                let existing_coords: std::collections::HashSet<(i64, i64)> = results
                    .iter()
                    .map(|r| ((r.lat * 1_000_000.0).round() as i64, (r.lon * 1_000_000.0).round() as i64))
                    .collect();
                for gr in geo_results {
                    let key = ((gr.lat * 1_000_000.0).round() as i64, (gr.lon * 1_000_000.0).round() as i64);
                    if !existing_coords.contains(&key) {
                        results.push(LocationResult {
                            id: None,
                            name: gr.display_name.clone(),
                            address: gr.display_name,
                            lat: gr.lat,
                            lon: gr.lon,
                            category: gr.category.unwrap_or_else(|| "diger".to_string()),
                            source: "nominatim".to_string(),
                        });
                    }
                }
            }
            Err(e) => tracing::warn!(error = %e, "Nominatim araması başarısız"),
        }
    }

    tracing::info!(query = %query, db_count, total = results.len(), "Konum araması tamamlandı");

    (StatusCode::OK, Json(serde_json::json!({
        "query": query,
        "results": results,
    }))).into_response()
}

// ---------------------------------------------------------------------------
// Admin: Konum CRUD
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateLocationRequest {
    pub name: String,
    pub address: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
struct LocationDetail {
    id: i64,
    name: String,
    address: String,
    lat: f64,
    lon: f64,
    category: String,
    is_active: bool,
}

/// POST /admin/api/locations
pub async fn admin_create_location(
    claims: crate::middleware::jwt::JwtClaims,
    State(state): State<AppState>,
    Json(body): Json<CreateLocationRequest>,
) -> impl IntoResponse {
    if let Err(resp) = check_admin_access_api(&state, claims.user_id).await {
        return resp;
    }

    tracing::info!(name = %body.name, lat = body.lat, lon = body.lon, "POST /admin/api/locations");

    if body.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "name is required"}))).into_response();
    }
    if body.lat < -90.0 || body.lat > 90.0 || body.lon < -180.0 || body.lon > 180.0 {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid coordinates"}))).into_response();
    }

    let new_loc = locations::ActiveModel {
        name: Set(body.name.trim().to_string()),
        address: Set(body.address.unwrap_or_default().trim().to_string()),
        lat: Set(body.lat),
        lon: Set(body.lon),
        category: Set(body.category.unwrap_or_else(|| "other".to_string())),
        is_active: Set(true),
        ..Default::default()
    };

    let loc = match new_loc.insert(&state.db).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, "Konum ekleme başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "insert failed"}))).into_response();
        }
    };

    let detail = LocationDetail {
        id: loc.id,
        name: loc.name,
        address: loc.address,
        lat: loc.lat,
        lon: loc.lon,
        category: loc.category,
        is_active: loc.is_active,
    };

    tracing::info!(id = detail.id, name = %detail.name, "Konum eklendi");
    (StatusCode::CREATED, Json(detail)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub category: Option<String>,
    #[serde(default = "default_list_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}

fn default_list_limit() -> u64 {
    50
}

/// GET /admin/api/locations?category=...&limit=50&offset=0
pub async fn admin_list_locations(
    claims: crate::middleware::jwt::JwtClaims,
    State(state): State<AppState>,
    Query(params): Query<ListQuery>,
) -> impl IntoResponse {
    if let Err(resp) = check_admin_access_api(&state, claims.user_id).await {
        return resp;
    }

    let mut query = Location::find()
        .filter(locations::Column::IsActive.eq(true))
        .order_by_asc(locations::Column::Name);

    if let Some(cat) = &params.category {
        query = query.filter(locations::Column::Category.eq(cat.as_str()));
    }

    let total = query.clone().count(&state.db).await.unwrap_or(0);
    let rows = query.limit(params.limit).offset(params.offset).all(&state.db).await.unwrap_or_default();

    let items: Vec<LocationDetail> = rows.iter().map(|loc| LocationDetail {
        id: loc.id,
        name: loc.name.clone(),
        address: loc.address.clone(),
        lat: loc.lat,
        lon: loc.lon,
        category: loc.category.clone(),
        is_active: loc.is_active,
    }).collect();

    (StatusCode::OK, Json(serde_json::json!({
        "total": total,
        "limit": params.limit,
        "offset": params.offset,
        "locations": items,
    }))).into_response()
}

/// GET /admin/api/locations/{id}
pub async fn admin_get_location(
    claims: crate::middleware::jwt::JwtClaims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Err(resp) = check_admin_access_api(&state, claims.user_id).await {
        return resp;
    }

    let loc = match Location::find_by_id(id).one(&state.db).await {
        Ok(Some(l)) => l,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "admin_get_location: sorgu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "db error"}))).into_response();
        }
    };

    let detail = LocationDetail {
        id: loc.id,
        name: loc.name,
        address: loc.address,
        lat: loc.lat,
        lon: loc.lon,
        category: loc.category,
        is_active: loc.is_active,
    };

    (StatusCode::OK, Json(detail)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct UpdateLocationRequest {
    pub name: Option<String>,
    pub address: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub category: Option<String>,
}

/// PUT /admin/api/locations/{id}
pub async fn admin_update_location(
    claims: crate::middleware::jwt::JwtClaims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateLocationRequest>,
) -> impl IntoResponse {
    if let Err(resp) = check_admin_access_api(&state, claims.user_id).await {
        return resp;
    }

    let loc = match Location::find_by_id(id).one(&state.db).await {
        Ok(Some(l)) => l,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "admin_update_location: sorgu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "db error"}))).into_response();
        }
    };

    let mut active: locations::ActiveModel = loc.into();
    if let Some(name) = body.name {
        active.name = Set(name.trim().to_string());
    }
    if let Some(address) = body.address {
        active.address = Set(address.trim().to_string());
    }
    if let Some(lat) = body.lat {
        active.lat = Set(lat);
    }
    if let Some(lon) = body.lon {
        active.lon = Set(lon);
    }
    if let Some(category) = body.category {
        active.category = Set(category);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = match active.update(&state.db).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "admin_update_location: güncelleme başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "update failed"}))).into_response();
        }
    };

    let detail = LocationDetail {
        id: updated.id,
        name: updated.name,
        address: updated.address,
        lat: updated.lat,
        lon: updated.lon,
        category: updated.category,
        is_active: updated.is_active,
    };

    tracing::info!(id, "Konum güncellendi");
    (StatusCode::OK, Json(detail)).into_response()
}

/// DELETE /admin/api/locations/{id} (soft delete)
pub async fn admin_delete_location(
    claims: crate::middleware::jwt::JwtClaims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Err(resp) = check_admin_access_api(&state, claims.user_id).await {
        return resp;
    }

    let loc = match Location::find_by_id(id).one(&state.db).await {
        Ok(Some(l)) => l,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "admin_delete_location: sorgu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "db error"}))).into_response();
        }
    };

    let mut active: locations::ActiveModel = loc.into();
    active.is_active = Set(false);
    active.updated_at = Set(chrono::Utc::now().into());

    if let Err(e) = active.update(&state.db).await {
        tracing::error!(error = %e, "admin_delete_location: güncelleme başarısız");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "update failed"}))).into_response();
    }

    tracing::info!(id, "Konum silindi (soft delete)");
    (StatusCode::OK, Json(serde_json::json!({"deleted": true, "id": id}))).into_response()
}
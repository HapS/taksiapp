use crate::{
    app_state::AppState,
    middleware::jwt::JwtClaims,
    modules::{
        auth::models::user::{Entity as User, Model as UserModel},
        ride::{
            dispatch,
            entities::{
                drivers::{self, Entity as Driver},
                ride_fare_configs::{self, Entity as RideFareConfig},
                rides::{self, Entity as Ride, RideStatus},
            },
        },
    },
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Ücret Hesaplama
// ---------------------------------------------------------------------------
// Ücret konfigürasyonu `ride_fare_configs` tablosundan çekilir.
// Tablo yoksa veya aktif kayıt bulunamazsa aşağıdaki fallback değerler kullanılır.
//
// Formül: min_fare + distance_km * per_km_fee
//   - min_fare    : Taban ücret — araç hareket etmese bile alınır (bindi-indi)
//   - per_km_fee  : Her km için eklenen ücret
//   - opening_fee : Taksimetre açılış ücreti (bilgi amaçlı gösterilir, formülde yok)
//
// Örnek (Sakarya): min_fare=25, per_km_fee=8
//   2 km → 25 + 2×8 = 41 ₺
//   5 km → 25 + 5×8 = 65 ₺
pub const FALLBACK_OPENING_FEE: f64 = 15.0;
pub const FALLBACK_MIN_FARE: f64 = 25.0;
pub const FALLBACK_PER_KM_FEE: f64 = 8.0;

/// Ücret konfigürasyonu — DB'den veya fallback'ten gelir.
#[derive(Debug, Clone)]
pub struct FareConfig {
    pub opening_fee: f64,
    pub min_fare: f64,
    pub per_km_fee: f64,
}

impl Default for FareConfig {
    fn default() -> Self {
        Self {
            opening_fee: FALLBACK_OPENING_FEE,
            min_fare: FALLBACK_MIN_FARE,
            per_km_fee: FALLBACK_PER_KM_FEE,
        }
    }
}

/// DB'den aktif fare config çeker. Bulunamazsa fallback döner.
///
/// Şu an tek bir aktif config varsayılır (is_active = true olan ilk kayıt).
/// İleride pickup koordinatına göre il tespiti yapılıp city_code ile filtrelenecek.
pub async fn fetch_fare_config(db: &sea_orm::DatabaseConnection) -> FareConfig {
    match RideFareConfig::find()
        .filter(ride_fare_configs::Column::IsActive.eq(true))
        .one(db)
        .await
    {
        Ok(Some(cfg)) => {
            let to_f64 = |d: Decimal| d.to_string().parse::<f64>().unwrap_or(0.0);
            FareConfig {
                opening_fee: to_f64(cfg.opening_fee),
                min_fare: to_f64(cfg.min_fare),
                per_km_fee: to_f64(cfg.per_km_fee),
            }
        }
        Ok(None) => {
            tracing::warn!("ride_fare_configs: aktif kayıt bulunamadı, fallback kullanılıyor");
            FareConfig::default()
        }
        Err(e) => {
            tracing::error!(error = %e, "ride_fare_configs: sorgu başarısız, fallback kullanılıyor");
            FareConfig::default()
        }
    }
}

/// Mesafe ve config'e göre ücret hesaplar.
///
/// Formül: max(min_fare, opening_fee + distance_km * per_km_fee)
/// - opening_fee + km * per_km_fee  : tahmini kazanç (km arttıkça artar)
/// - min_fare                        : taban ücret (kısa mesafede bile alınır)
/// Hangisi büyükse o alınır.
pub fn calculate_fare_with_config(distance_km: f64, cfg: &FareConfig) -> Decimal {
    let raw = cfg.opening_fee + distance_km * cfg.per_km_fee;
    let fare = if raw < cfg.min_fare { cfg.min_fare } else { raw };
    Decimal::from_str_exact(&format!("{:.2}", fare))
        .unwrap_or_else(|_| Decimal::from(cfg.min_fare as i64))
}

/// FareInfo JSON bloğu oluşturur — response'larda kullanılır.
pub fn build_fare_info_json(distance_km: f64, cfg: &FareConfig) -> serde_json::Value {
    let estimated = cfg.opening_fee + distance_km * cfg.per_km_fee;
    let fare = if estimated < cfg.min_fare { cfg.min_fare } else { estimated };
    let fare = (fare * 100.0).round() / 100.0;
    serde_json::json!({
        "opening_fee": cfg.opening_fee,
        "min_fare": cfg.min_fare,
        "per_km_fee": cfg.per_km_fee,
        "estimated_fare": fare,
        "currency": "TRY",
    })
}

#[derive(Debug, Deserialize)]
pub struct RideRequest {
    pub pickup_lat: f64,
    pub pickup_lon: f64,
    pub pickup_address: String,
    pub dropoff_lat: f64,
    pub dropoff_lon: f64,
    pub dropoff_address: String,
}

pub async fn request_ride(
    claims: JwtClaims,
    State(state): State<AppState>,
    Json(body): Json<RideRequest>,
) -> impl IntoResponse {
    let user_id = claims.user_id;

    tracing::info!(
        user_id,
        pickup = %format!("{},{}", body.pickup_lat, body.pickup_lon),
        dropoff = %format!("{},{}", body.dropoff_lat, body.dropoff_lon),
        "HTTP POST /api/ride/request — Yeni yolculuk talebi"
    );

    // 1. rides tablosuna insert
    let new_ride = rides::ActiveModel {
        user_id: Set(user_id),
        status: Set(RideStatus::Searching),
        pickup_lat: Set(body.pickup_lat),
        pickup_lon: Set(body.pickup_lon),
        pickup_address: Set(body.pickup_address.clone()),
        dropoff_lat: Set(body.dropoff_lat),
        dropoff_lon: Set(body.dropoff_lon),
        dropoff_address: Set(body.dropoff_address.clone()),
        requested_at: Set(chrono::Utc::now().into()),
        ..Default::default()
    };

    let ride = match new_ride.insert(&state.db).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "request_ride: insert başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "internal error"}))).into_response();
        }
    };

    let ride_id = ride.id;
    tracing::info!(ride_id, user_id, "HTTP Ride oluşturuldu");

    // 2. ORS'den mesafe/süre al, yoksa OSRM dene
    let (distance_km, duration_sec) = if let Some(ors) = state.config.ors_config() {
        fetch_ors_route(
            ors,
            body.pickup_lon, body.pickup_lat,
            body.dropoff_lon, body.dropoff_lat,
        ).await
    } else {
        (None, None)
    };

    let (distance_km, duration_sec) = if distance_km.is_some() {
        (distance_km, duration_sec)
    } else {
        // ORS yoksa veya başarısızsa OSRM dene
        fetch_osrm_distance(
            body.pickup_lon, body.pickup_lat,
            body.dropoff_lon, body.dropoff_lat,
        ).await
    };

    // 3. DB'den fare config çek, gerçek mesafeyle ücret hesapla
    let fare_cfg = fetch_fare_config(&state.db).await;
    let dist = distance_km.unwrap_or(0.0);
    let fare = calculate_fare_with_config(dist, &fare_cfg);

    tracing::info!(
        ride_id,
        distance_km = dist,
        fare = %fare,
        min_fare = fare_cfg.min_fare,
        per_km_fee = fare_cfg.per_km_fee,
        "Ücret hesaplandı"
    );

    // DB'ye yaz
    if let Ok(Some(r)) = Ride::find_by_id(ride_id).one(&state.db).await {
        let mut active: rides::ActiveModel = r.into();
        if let Some(d) = distance_km {
            active.distance_km = Set(Some(d));
        }
        if let Some(s) = duration_sec {
            active.duration_sec = Set(Some(s));
        }
        active.fare_amount = Set(Some(fare));
        active.update(&state.db).await.ok();
    }

    // 3. Dispatch başlat
    let state_arc = Arc::new(state);
    tokio::spawn(dispatch::dispatch_ride(state_arc, ride_id));

    tracing::info!(ride_id, user_id, "HTTP POST /api/ride/request → 200 OK");
    (StatusCode::OK, Json(serde_json::json!({ "ride_id": ride_id, "status": "searching" }))).into_response()
}

#[derive(serde::Deserialize)]
struct OrsResponse {
    features: Vec<OrsFeature>,
}

#[derive(serde::Deserialize)]
struct OrsFeature {
    properties: OrsProperties,
}

#[derive(serde::Deserialize)]
struct OrsProperties {
    segments: Vec<OrsSegment>,
}

#[derive(serde::Deserialize)]
struct OrsSegment {
    distance: f64,
    duration: f64,
}

async fn fetch_ors_route(
    ors: &crate::config::app_config::OrsConfig,
    from_lon: f64, from_lat: f64,
    to_lon: f64, to_lat: f64,
) -> (Option<f64>, Option<i32>) {
    let url = format!(
        "{}/v2/directions/driving-car?api_key={}&start={},{}&end={},{}",
        ors.base_url, ors.api_key,
        from_lon, from_lat,
        to_lon, to_lat,
    );

    match reqwest::get(&url).await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<OrsResponse>().await {
                Ok(data) => {
                    if let Some(segment) = data.features.first()
                        .and_then(|f| f.properties.segments.first())
                    {
                        let distance_km = segment.distance / 1000.0;
                        let duration_sec = segment.duration as i32;
                        (Some(distance_km), Some(duration_sec))
                    } else {
                        (None, None)
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "fetch_ors_route: JSON parse hatası");
                    (None, None)
                }
            }
        }
        Ok(resp) => {
            tracing::error!(status = %resp.status(), "fetch_ors_route: ORS hatası");
            (None, None)
        }
        Err(e) => {
            tracing::error!(error = %e, "fetch_ors_route: HTTP hatası");
            (None, None)
        }
    }
}

/// OSRM'den mesafe ve süre çeker — ORS yoksa veya başarısızsa kullanılır.
async fn fetch_osrm_distance(
    from_lon: f64, from_lat: f64,
    to_lon: f64, to_lat: f64,
) -> (Option<f64>, Option<i32>) {
    let url = format!(
        "https://router.project-osrm.org/route/v1/driving/{},{};{},{}?overview=false",
        from_lon, from_lat, to_lon, to_lat,
    );

    match reqwest::get(&url).await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<serde_json::Value>().await {
                Ok(data) => {
                    if let Some(route) = data["routes"].as_array().and_then(|r| r.first()) {
                        let distance_m = route["distance"].as_f64().unwrap_or(0.0);
                        let duration_s = route["duration"].as_f64().unwrap_or(0.0);
                        if distance_m > 0.0 {
                            tracing::info!("OSRM mesafe: {} km, {} sn", distance_m / 1000.0, duration_s as i32);
                            return (Some(distance_m / 1000.0), Some(duration_s as i32));
                        }
                    }
                    tracing::warn!("fetch_osrm_distance: JSON yapısı beklenmiyor");
                }
                Err(e) => tracing::error!(error = %e, "fetch_osrm_distance: JSON parse hatası"),
            }
        }
        Ok(resp) => tracing::warn!("fetch_osrm_distance: HTTP {}", resp.status()),
        Err(e) => tracing::error!(error = %e, "fetch_osrm_distance: HTTP hatası"),
    }
    (None, None)
}

#[derive(Debug, Serialize)]
struct RideDetail {
    id: i64,
    user_id: i64,
    status: String,
    pickup_lat: f64,
    pickup_lon: f64,
    dropoff_lat: f64,
    dropoff_lon: f64,
    pickup_address: String,
    dropoff_address: String,
    distance_km: Option<f64>,
    duration_sec: Option<i32>,
    fare_amount: Option<f64>,
    fare_info: serde_json::Value,
    driver: Option<DriverInfo>,
}

#[derive(Debug, Serialize)]
struct DriverInfo {
    full_name: String,
    vehicle_plate: String,
    vehicle_model: String,
    phone: String,
    current_lat: Option<f64>,
    current_lon: Option<f64>,
}

pub async fn get_ride(
    claims: JwtClaims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    tracing::info!(ride_id = id, user_id = claims.user_id, "HTTP GET /api/ride/{}", id);

    let ride = match Ride::find_by_id(id).one(&state.db).await {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "get_ride: sorgu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "internal error"}))).into_response();
        }
    };

    // Yetki kontrolü: sadece yolcu veya atanmış sürücü erişebilir
    let caller_id = claims.user_id;
    let is_passenger = ride.user_id == caller_id;
    let is_assigned_driver = if let Some(driver_id) = ride.driver_id {
        // Sürücünün user_id'si ile eşleştir
        Driver::find()
            .filter(drivers::Column::Id.eq(driver_id))
            .filter(drivers::Column::UserId.eq(caller_id))
            .one(&state.db)
            .await
            .ok()
            .flatten()
            .is_some()
    } else {
        false
    };

    if !is_passenger && !is_assigned_driver {
        tracing::warn!(ride_id = id, caller_id, "get_ride: yetkisiz erişim denemesi");
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "forbidden"}))).into_response();
    }

    let driver_info = if let Some(driver_id) = ride.driver_id {
        Driver::find_by_id(driver_id)
            .one(&state.db)
            .await
            .ok()
            .flatten()
            .map(|d| {
                // Hub'dan anlık konum al, yoksa DB'deki son konumu kullan
                let (hub_lat, hub_lon) = state.hub.drivers.get(&driver_id)
                    .map(|s| (s.lat, s.lon))
                    .unwrap_or((d.current_lat, d.current_lon));
                DriverInfo {
                    full_name: d.full_name,
                    vehicle_plate: d.vehicle_plate,
                    vehicle_model: d.vehicle_model,
                    phone: d.phone,
                    current_lat: hub_lat,
                    current_lon: hub_lon,
                }
            })
    } else {
        None
    };

    let ride_driver_id = ride.driver_id;
    let dist = ride.distance_km.unwrap_or(0.0);
    let fare_cfg = fetch_fare_config(&state.db).await;
    let detail = RideDetail {
        id: ride.id,
        user_id: ride.user_id,
        status: ride.status.as_str().to_string(),
        pickup_lat: ride.pickup_lat,
        pickup_lon: ride.pickup_lon,
        dropoff_lat: ride.dropoff_lat,
        dropoff_lon: ride.dropoff_lon,
        pickup_address: ride.pickup_address,
        dropoff_address: ride.dropoff_address,
        distance_km: ride.distance_km,
        duration_sec: ride.duration_sec,
        fare_amount: ride.fare_amount
            .as_ref()
            .and_then(|d| d.to_string().parse::<f64>().ok()),
        fare_info: build_fare_info_json(dist, &fare_cfg),
        driver: driver_info,
    };

    tracing::info!(ride_id = id, status = %detail.status, driver_id = ?ride_driver_id, "HTTP GET /api/ride/{} → 200", id);

    (StatusCode::OK, Json(detail)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusRequest {
    pub status: String,
}

pub async fn update_ride_status(
    claims: JwtClaims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateStatusRequest>,
) -> impl IntoResponse {
    tracing::info!(ride_id = id, new_status = %body.status, caller_id = claims.user_id, "HTTP POST /api/ride/{}/status", id);

    let ride = match Ride::find_by_id(id).one(&state.db).await {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "update_ride_status: sorgu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "internal error"}))).into_response();
        }
    };

    // Yetki kontrolü: yolcu veya atanmış sürücü durum güncelleyebilir
    let caller_id = claims.user_id;
    let is_passenger = ride.user_id == caller_id;
    let is_assigned_driver = if let Some(driver_id) = ride.driver_id {
        Driver::find()
            .filter(drivers::Column::Id.eq(driver_id))
            .filter(drivers::Column::UserId.eq(caller_id))
            .one(&state.db)
            .await
            .ok()
            .flatten()
            .is_some()
    } else {
        false
    };

    if !is_passenger && !is_assigned_driver {
        tracing::warn!(ride_id = id, caller_id, "update_ride_status: yetkisiz erişim denemesi");
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "forbidden"}))).into_response();
    }

    let new_status = match body.status.as_str() {
        "picked_up" if ride.status == RideStatus::Accepted && is_assigned_driver => RideStatus::PickedUp,
        "completed" if (ride.status == RideStatus::Accepted || ride.status == RideStatus::PickedUp)
            && (is_passenger || is_assigned_driver) => RideStatus::Completed,
        _ => {
            tracing::warn!(ride_id = id, current = ?ride.status, requested = %body.status, "HTTP Geçersiz durum geçişi");
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid status transition"}))).into_response();
        }
    };

    tracing::info!(ride_id = id, from = ?ride.status, to = ?new_status, "HTTP Durum geçişi");

    let mut active: rides::ActiveModel = ride.clone().into();
    active.status = Set(new_status.clone());
    if new_status == RideStatus::PickedUp {
        active.picked_up_at = Set(Some(chrono::Utc::now().into()));
    } else if new_status == RideStatus::Completed {
        active.completed_at = Set(Some(chrono::Utc::now().into()));
    }

    if let Err(e) = active.update(&state.db).await {
        tracing::error!(error = %e, "update_ride_status: güncelleme başarısız");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "update failed"}))).into_response();
    }

    state.hub.send_to_passenger(
        ride.user_id,
        &crate::modules::ride::ws::messages::ServerMessage::RideStatusChanged {
            ride_id: id,
            status: body.status.clone(),
        },
    );

    if let Some(driver_id) = ride.driver_id {
        state.hub.send_to_driver(
            driver_id,
            &crate::modules::ride::ws::messages::ServerMessage::RideStatusChanged {
                ride_id: id,
                status: body.status.clone(),
            },
        );
    }

    if new_status == RideStatus::Completed {
        state.hub.ride_rooms.remove(&id);
    }

    tracing::info!(ride_id = id, status = %body.status, "HTTP POST /api/ride/{}/status → 200", id);

    (StatusCode::OK, Json(serde_json::json!({"status": body.status}))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct CancelRequest {
    pub by: String,
}

pub async fn cancel_ride(
    claims: JwtClaims,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<CancelRequest>,
) -> impl IntoResponse {
    tracing::info!(ride_id = id, by = %body.by, caller_id = claims.user_id, "HTTP POST /api/ride/{}/cancel", id);

    let ride = match Ride::find_by_id(id).one(&state.db).await {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "cancel_ride: sorgu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "internal error"}))).into_response();
        }
    };

    // Yetki kontrolü: yolcu veya atanmış sürücü iptal edebilir
    let caller_id = claims.user_id;
    let is_passenger = ride.user_id == caller_id;
    let is_assigned_driver = if let Some(driver_id) = ride.driver_id {
        Driver::find()
            .filter(drivers::Column::Id.eq(driver_id))
            .filter(drivers::Column::UserId.eq(caller_id))
            .one(&state.db)
            .await
            .ok()
            .flatten()
            .is_some()
    } else {
        false
    };

    if !is_passenger && !is_assigned_driver {
        tracing::warn!(ride_id = id, caller_id, "cancel_ride: yetkisiz erişim denemesi");
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "forbidden"}))).into_response();
    }

    if ride.status == RideStatus::Completed || ride.status == RideStatus::Cancelled {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "ride already finished"}))).into_response();
    }

    let mut active: rides::ActiveModel = ride.clone().into();
    active.status = Set(RideStatus::Cancelled);
    active.cancelled_at = Set(Some(chrono::Utc::now().into()));

    if let Err(e) = active.update(&state.db).await {
        tracing::error!(error = %e, "cancel_ride: güncelleme başarısız");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "update failed"}))).into_response();
    }

    // Bildirim
    state.hub.send_to_passenger(
        ride.user_id,
        &crate::modules::ride::ws::messages::ServerMessage::RideStatusChanged {
            ride_id: id,
            status: "cancelled".to_string(),
        },
    );

    if let Some(driver_id) = ride.driver_id {
        state.hub.send_to_driver(
            driver_id,
            &crate::modules::ride::ws::messages::ServerMessage::RideStatusChanged {
                ride_id: id,
                status: "cancelled".to_string(),
            },
        );
    }

    state.hub.ride_rooms.remove(&id);

    (StatusCode::OK, Json(serde_json::json!({"status": "cancelled"}))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct RouteQuery {
    pub start_lat: f64,
    pub start_lon: f64,
    pub end_lat: f64,
    pub end_lon: f64,
}

#[derive(Debug, Serialize)]
struct RoutePoint {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Serialize)]
struct RouteData {
    points: Vec<RoutePoint>,
    distance_km: f64,
    duration_sec: i32,
}

fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6371.0;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos()
            * lat2.to_radians().cos()
            * (dlon / 2.0).sin().powi(2);
    R * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

/// Sürücünün aktif (accepted/picked_up) yolculuğunu döndürür.
///
/// Backend: GET /api/ride/driver/active (JWT Auth)
/// Yanıt: { active_ride: { ride_id, status, pickup_address, ... } | null }
pub async fn get_driver_active_ride(
    claims: JwtClaims,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // 1. user_id → driver kaydını bul
    let driver = match Driver::find()
        .filter(drivers::Column::UserId.eq(claims.user_id))
        .filter(drivers::Column::IsActive.eq(true))
        .one(&state.db)
        .await
    {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (StatusCode::OK, Json(serde_json::json!({
                "active_ride": null
            }))).into_response();
        }
        Err(e) => {
            tracing::error!(error = %e, "get_driver_active_ride: driver sorgusu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "internal error"
            }))).into_response();
        }
    };

    // 2. Bu sürücünün accepted/picked_up ride'ını bul
    let active_ride = match Ride::find()
        .filter(rides::Column::DriverId.eq(driver.id))
        .filter(
            Condition::any()
                .add(rides::Column::Status.eq(RideStatus::Accepted))
                .add(rides::Column::Status.eq(RideStatus::PickedUp)),
        )
        .one(&state.db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "get_driver_active_ride: ride sorgusu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "internal error"
            }))).into_response();
        }
    };

    match active_ride {
        None => {
            (StatusCode::OK, Json(serde_json::json!({
                "active_ride": null
            }))).into_response()
        }
        Some(ride) => {
            let dist = ride.distance_km.unwrap_or(0.0);
            let fare_cfg = fetch_fare_config(&state.db).await;
            (StatusCode::OK, Json(serde_json::json!({
                "active_ride": {
                    "ride_id": ride.id,
                    "status": ride.status.as_str(),
                    "pickup_address": ride.pickup_address,
                    "dropoff_address": ride.dropoff_address,
                    "pickup_lat": ride.pickup_lat,
                    "pickup_lon": ride.pickup_lon,
                    "dropoff_lat": ride.dropoff_lat,
                    "dropoff_lon": ride.dropoff_lon,
                    "distance_km": ride.distance_km,
                    "duration_sec": ride.duration_sec,
                    "fare_amount": ride.fare_amount.as_ref().and_then(|d| d.to_string().parse::<f64>().ok()),
                    "fare_info": build_fare_info_json(dist, &fare_cfg),
                    "requested_at": ride.requested_at.to_rfc3339(),
                }
            }))).into_response()
        }
    }
}

/// Yolcunun aktif (accepted/picked_up) yolculuğunu döndürür.
///
/// Backend: GET /api/ride/passenger/active (JWT Auth)
/// Yanıt: { active_ride: { ride_id, status, pickup_address, ... } | null }
pub async fn get_passenger_active_ride(
    claims: JwtClaims,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let active_ride = match Ride::find()
        .filter(rides::Column::UserId.eq(claims.user_id))
        .filter(
            Condition::any()
                .add(rides::Column::Status.eq(RideStatus::Accepted))
                .add(rides::Column::Status.eq(RideStatus::PickedUp)),
        )
        .one(&state.db)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "get_passenger_active_ride: ride sorgusu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "internal error"
            }))).into_response();
        }
    };

    match active_ride {
        None => {
            (StatusCode::OK, Json(serde_json::json!({
                "active_ride": null
            }))).into_response()
        }
        Some(ride) => {
            let dist = ride.distance_km.unwrap_or(0.0);
            let fare_cfg = fetch_fare_config(&state.db).await;
            (StatusCode::OK, Json(serde_json::json!({
                "active_ride": {
                    "ride_id": ride.id,
                    "status": ride.status.as_str(),
                    "pickup_address": ride.pickup_address,
                    "dropoff_address": ride.dropoff_address,
                    "pickup_lat": ride.pickup_lat,
                    "pickup_lon": ride.pickup_lon,
                    "dropoff_lat": ride.dropoff_lat,
                    "dropoff_lon": ride.dropoff_lon,
                    "distance_km": ride.distance_km,
                    "duration_sec": ride.duration_sec,
                    "fare_amount": ride.fare_amount.as_ref().and_then(|d| d.to_string().parse::<f64>().ok()),
                    "fare_info": build_fare_info_json(dist, &fare_cfg),
                    "requested_at": ride.requested_at.to_rfc3339(),
                }
            }))).into_response()
        }
    }
}

pub async fn get_route(
    State(state): State<AppState>,
    Query(params): Query<RouteQuery>,
) -> impl IntoResponse {
    tracing::info!(
        "GET /api/ride/route?start_lat={},start_lon={}&end_lat={},end_lon={}",
        params.start_lat, params.start_lon,
        params.end_lat, params.end_lon,
    );

    // Rota sağlayıcıdan bağımsız — header'da DB config çek
    let fare_cfg = fetch_fare_config(&state.db).await;

    // Önce ORS dene
    if let Some(ors) = state.config.ors_config() {
        let url = format!(
            "{}/v2/directions/driving-car?api_key={}&start={},{}&end={},{}",
            ors.base_url, ors.api_key,
            params.start_lon, params.start_lat,
            params.end_lon, params.end_lat,
        );
        if let Some(data) = fetch_json_route(&url, extract_ors_data).await {
            tracing::info!("Rota: ORS sağlayıcı kullanıldı, {} nokta, {} km, {} sn",
                data.points.len(), data.distance_km, data.duration_sec);
            return route_ok_response(data, &fare_cfg);
        }
        tracing::warn!("Rota: ORS başarısız, OSRM deneniyor...");
    } else {
        tracing::warn!("Rota: ORS config yok, OSRM deneniyor...");
    }

    // OSRM dene (ücretsiz, anahtar gerekmez)
    let url = format!(
        "https://router.project-osrm.org/route/v1/driving/{},{};{},{}?overview=full&geometries=geojson",
        params.start_lon, params.start_lat,
        params.end_lon, params.end_lat,
    );
    if let Some(data) = fetch_json_route(&url, extract_osrm_data).await {
        tracing::info!("Rota: OSRM sağlayıcı kullanıldı, {} nokta, {} km, {} sn",
            data.points.len(), data.distance_km, data.duration_sec);
        return route_ok_response(data, &fare_cfg);
    }

    // Hiçbiri çalışmazsa düz çizgi
    tracing::warn!("get_route: ORS ve OSRM başarısız, fallback düz çizgi");
    let dist = haversine_km(params.start_lat, params.start_lon, params.end_lat, params.end_lon);
    let dur = (dist / 30.0 * 3600.0) as i32;
    return route_ok_response(RouteData {
        points: vec![
            RoutePoint { lat: params.start_lat, lon: params.start_lon },
            RoutePoint { lat: params.end_lat, lon: params.end_lon },
        ],
        distance_km: dist,
        duration_sec: dur,
    }, &fare_cfg);
}

fn route_ok_response(data: RouteData, fare_cfg: &FareConfig) -> (StatusCode, Json<serde_json::Value>) {
    let raw = fare_cfg.opening_fee + data.distance_km * fare_cfg.per_km_fee;
    let fare = if raw < fare_cfg.min_fare { fare_cfg.min_fare } else { raw };
    let estimated_fare = (fare * 100.0).round() / 100.0;

    (StatusCode::OK, Json(serde_json::json!({
        "success": true,
        "data": data.points,
        "distance_km": (data.distance_km * 100.0).round() / 100.0,
        "duration_sec": data.duration_sec,
        "fare_info": {
            "opening_fee": fare_cfg.opening_fee,
            "min_fare": fare_cfg.min_fare,
            "per_km_fee": fare_cfg.per_km_fee,
            "estimated_fare": estimated_fare,
            "currency": "TRY",
        }
    })))
}

fn extract_ors_data(value: &serde_json::Value) -> Option<RouteData> {
    let coords = value["features"][0]["geometry"]["coordinates"].as_array()?;
    let points: Vec<RoutePoint> = coords
        .iter()
        .filter_map(|c| {
            let lon = c[0].as_f64()?;
            let lat = c[1].as_f64()?;
            Some(RoutePoint { lat, lon })
        })
        .collect();
    if points.is_empty() {
        return None;
    }
    let distance_m = value["features"][0]["properties"]["segments"][0]["distance"].as_f64()?;
    let duration_s = value["features"][0]["properties"]["segments"][0]["duration"].as_f64()?;
    Some(RouteData {
        points,
        distance_km: distance_m / 1000.0,
        duration_sec: duration_s as i32,
    })
}

fn extract_osrm_data(value: &serde_json::Value) -> Option<RouteData> {
    let coords = value["routes"][0]["geometry"]["coordinates"].as_array()?;
    let points: Vec<RoutePoint> = coords
        .iter()
        .filter_map(|c| {
            let lon = c[0].as_f64()?;
            let lat = c[1].as_f64()?;
            Some(RoutePoint { lat, lon })
        })
        .collect();
    if points.is_empty() {
        return None;
    }
    let distance_m = value["routes"][0]["distance"].as_f64()?;
    let duration_s = value["routes"][0]["duration"].as_f64()?;
    Some(RouteData {
        points,
        distance_km: distance_m / 1000.0,
        duration_sec: duration_s as i32,
    })
}

async fn fetch_json_route<F>(url: &str, extract: F) -> Option<RouteData>
where
    F: Fn(&serde_json::Value) -> Option<RouteData>,
{
    match reqwest::get(url).await {
        Ok(resp) if resp.status().is_success() => match resp.text().await {
            Ok(body) => match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(parsed) => {
                    if let Some(data) = extract(&parsed) {
                        return Some(data);
                    }
                    tracing::warn!("fetch_route: JSON yapısı beklenmiyor veya boş");
                }
                Err(e) => tracing::error!(error = %e, "fetch_route: JSON parse hatası"),
            },
            Err(e) => tracing::error!(error = %e, "fetch_route: body okuma hatası"),
        },
        Ok(resp) => tracing::warn!("fetch_route: HTTP {}", resp.status()),
        Err(e) => tracing::error!(error = %e, "fetch_route: HTTP hatası"),
    }
    None
}

// ---------------------------------------------------------------------------
// Yakındaki Sürücüler — viewport içindeki müsait sürücüleri döndürür
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct NearbyDriversQuery {
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lon: f64,
    pub max_lon: f64,
    #[serde(default = "default_available")]
    pub status: String,
}

fn default_available() -> String {
    "available".to_string()
}

#[derive(Serialize)]
struct NearbyDriver {
    id: i64,
    current_lat: f64,
    current_lon: f64,
    vehicle_model: String,
    vehicle_plate: String,
    rating: f64,
    is_on_ride: bool,
}

pub async fn get_nearby_drivers(
    _claims: JwtClaims,
    State(state): State<AppState>,
    Query(params): Query<NearbyDriversQuery>,
) -> impl IntoResponse {
    let drivers = match Driver::find()
        .filter(drivers::Column::IsActive.eq(true))
        .filter(drivers::Column::IsOnline.eq(true))
        .filter(drivers::Column::CurrentLat.is_not_null())
        .filter(drivers::Column::CurrentLon.is_not_null())
        .filter(drivers::Column::CurrentLat.gte(params.min_lat))
        .filter(drivers::Column::CurrentLat.lte(params.max_lat))
        .filter(drivers::Column::CurrentLon.gte(params.min_lon))
        .filter(drivers::Column::CurrentLon.lte(params.max_lon))
        .all(&state.db)
        .await
    {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, "get_nearby_drivers: sorgu başarısız");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "db error"}))).into_response();
        }
    };

    let result: Vec<NearbyDriver> = drivers
        .into_iter()
        .filter_map(|d| {
            let is_on_ride = state.hub.ride_rooms.iter().any(|room| room.value().0 == d.id);
            match params.status.as_str() {
                "available" if is_on_ride => None,
                "on_ride" if !is_on_ride => None,
                _ => Some(NearbyDriver {
                    id: d.id,
                    current_lat: d.current_lat.unwrap_or(0.0),
                    current_lon: d.current_lon.unwrap_or(0.0),
                    vehicle_model: d.vehicle_model,
                    vehicle_plate: d.vehicle_plate,
                    rating: d.rating,
                    is_on_ride,
                }),
            }
        })
        .collect();

    (StatusCode::OK, Json(serde_json::json!({"drivers": result}))).into_response()
}

// ---------------------------------------------------------------------------
// Geçmiş Yolculuklar — sürücü ve yolcu için tamamlanmış/iptal edilmiş yolculuklar
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    /// "driver" | "passenger" | "auto" (default) — auto: sürücü kaydı varsa sürücü
    #[serde(default = "default_history_role")]
    pub role: String,
    /// "completed" | "cancelled" | "no_driver" — verilmezse tümü
    pub status: Option<String>,
    /// Sayfa başına kayıt (default 20, max 100)
    pub limit: Option<u32>,
    /// Atlanacak kayıt sayısı (default 0) — cursor varsa yoksayılır
    pub offset: Option<u32>,
    /// Cursor-based pagination: base64("requested_at_iso|id") — bir sonraki sayfa
    pub cursor: Option<String>,
}

fn default_history_role() -> String {
    "auto".to_string()
}

/// Cursor encode/decode — "RFC3339_timestamp|ride_id" base64
/// Önceki sayfada dönen `next_cursor` sonraki istekte `?cursor=...` olarak gönderilir.
fn encode_cursor(requested_at: &chrono::DateTime<chrono::FixedOffset>, ride_id: i64) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let raw = format!("{}|{}", requested_at.to_rfc3339(), ride_id);
    URL_SAFE_NO_PAD.encode(raw.as_bytes())
}

fn decode_cursor(cursor: &str) -> Option<(chrono::DateTime<chrono::FixedOffset>, i64)> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let bytes = URL_SAFE_NO_PAD.decode(cursor).ok()?;
    let raw = String::from_utf8(bytes).ok()?;
    let (ts, id) = raw.split_once('|')?;
    let dt = chrono::DateTime::parse_from_rfc3339(ts).ok()?;
    let id: i64 = id.parse().ok()?;
    Some((dt, id))
}

#[derive(Debug, Serialize)]
struct HistoryCounterparty {
    user_id: i64,
    full_name: String,
    phone: Option<String>,
    /// Sürücü ise ekstra alanlar
    #[serde(skip_serializing_if = "Option::is_none")]
    vehicle_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vehicle_plate: Option<String>,
}

#[derive(Debug, Serialize)]
struct HistoryItem {
    ride_id: i64,
    status: String,
    pickup_address: String,
    pickup_lat: f64,
    pickup_lon: f64,
    dropoff_address: String,
    dropoff_lat: f64,
    dropoff_lon: f64,
    distance_km: Option<f64>,
    duration_sec: Option<i32>,
    fare_amount: Option<f64>,
    requested_at: String,
    accepted_at: Option<String>,
    picked_up_at: Option<String>,
    completed_at: Option<String>,
    cancelled_at: Option<String>,
    /// Görüntüleyen kişi sürücü ise: yolcu bilgisi; yolcu ise: sürücü bilgisi
    counterparty: HistoryCounterparty,
}

fn decode_dt(value: &Option<chrono::DateTime<chrono::FixedOffset>>) -> Option<String> {
    value.as_ref().map(|d| d.to_rfc3339())
}

/// Geçmiş yolculukları listeler — hem sürücü hem yolcu tarafı.
///
/// Pagination stratejisi (öncelik sırası):
/// 1. **`?cursor=...`** — Cursor-based (önerilen). Bir sonraki sayfa için response'taki
///    `next_cursor` alanını gönder. OFFSET kullanmaz, büyük dataset'lerde hızlı ve stabildir
///    (yeni kayıt eklenmesi sayfa kaymasına yol açmaz).
/// 2. **`?offset=N&limit=M`** — Legacy offset-based. İlk sayfa için kullanışlı.
///
/// Diğer parametreler:
/// - `role=auto` (default): Sürücü kaydı varsa sürücü geçmişi, yoksa yolcu
/// - `role=driver` / `role=passenger`: Zorunlu rol
/// - `status=completed|cancelled|no_driver`: Opsiyonel status filtresi
///
/// Yanıt:
/// ```json
/// {
///   "role": "driver",
///   "limit": 20,
///   "count": 20,           // bu sayfada dönen kayıt
///   "has_more": true,
///   "next_cursor": "MTcy...",
///   "total": 142,         // tüm eşleşen kayıt (filtre sonrası)
///   "rides": [ ... ]
/// }
/// ```
pub async fn get_ride_history(
    claims: JwtClaims,
    State(state): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> impl IntoResponse {
    let user_id = claims.user_id;
    tracing::info!(
        user_id,
        role = %params.role,
        status = ?params.status,
        limit = ?params.limit,
        offset = ?params.offset,
        cursor = ?params.cursor.as_ref().map(|_| "***"),
        "HTTP GET /api/ride/history — Geçmiş yolculuklar"
    );

    // Sürücü kaydı var mı?
    let driver_record = Driver::find()
        .filter(drivers::Column::UserId.eq(user_id))
        .filter(drivers::Column::IsActive.eq(true))
        .one(&state.db)
        .await
        .ok()
        .flatten();

    // Rol belirleme
    let resolved_role = match params.role.as_str() {
        "driver" => "driver",
        "passenger" => "passenger",
        _ => {
            if driver_record.is_some() {
                "driver"
            } else {
                "passenger"
            }
        }
    };

    if resolved_role == "driver" && driver_record.is_none() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "user has no active driver record"
        })))
            .into_response();
    }

    // Filtre koşulu — sadece bitmiş yolculuklar
    let mut cond = Condition::any()
        .add(rides::Column::Status.eq(RideStatus::Completed))
        .add(rides::Column::Status.eq(RideStatus::Cancelled))
        .add(rides::Column::Status.eq(RideStatus::NoDriver));

    if let Some(s) = &params.status {
        let parsed = match s.as_str() {
            "completed" => Some(RideStatus::Completed),
            "cancelled" => Some(RideStatus::Cancelled),
            "no_driver" => Some(RideStatus::NoDriver),
            _ => None,
        };
        match parsed {
            Some(rs) => {
                cond = Condition::any().add(rides::Column::Status.eq(rs));
            }
            None => {
                return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                    "error": "invalid status (allowed: completed, cancelled, no_driver)"
                })))
                    .into_response();
            }
        }
    }

    // Limit (1..=100, default 20)
    let limit: u64 = params.limit.unwrap_or(20).clamp(1, 100) as u64;

    // Cursor decode (varsa)
    let cursor_decoded = if let Some(c) = &params.cursor {
        match decode_cursor(c) {
            Some(v) => Some(v),
            None => {
                return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                    "error": "invalid cursor"
                })))
                    .into_response();
            }
        }
    } else {
        None
    };

    // Toplam sayı (filtre uygulanmış, cursor'dan bağımsız) — UI'da "X yolculuk" göstermek için
    let total: u64 = if resolved_role == "driver" {
        Ride::find()
            .filter(rides::Column::DriverId.eq(driver_record.as_ref().unwrap().id))
            .filter(cond.clone())
            .count(&state.db)
            .await
            .unwrap_or(0)
    } else {
        Ride::find()
            .filter(rides::Column::UserId.eq(user_id))
            .filter(cond.clone())
            .count(&state.db)
            .await
            .unwrap_or(0)
    };

    // Sahiplik filtresi
    let mut base_query = if resolved_role == "driver" {
        let driver_id = driver_record.as_ref().unwrap().id;
        Ride::find()
            .filter(rides::Column::DriverId.eq(driver_id))
            .filter(cond)
    } else {
        Ride::find()
            .filter(rides::Column::UserId.eq(user_id))
            .filter(cond)
    };

    // Cursor varsa keyset filtresi uygula
    // Order: requested_at DESC, id DESC
    // Keyset: (requested_at, id) < (cursor_ts, cursor_id)
    if let Some((cursor_ts, cursor_id)) = cursor_decoded {
        base_query = base_query.filter(
            Condition::any()
                .add(rides::Column::RequestedAt.lt(cursor_ts))
                .add(
                    Condition::all()
                        .add(rides::Column::RequestedAt.eq(cursor_ts))
                        .add(rides::Column::Id.lt(cursor_id)),
                ),
        );
    }

    // Sayfalama: limit+1 çek, has_more tespiti için
    let mut query = base_query
        .order_by_desc(rides::Column::RequestedAt)
        .order_by_desc(rides::Column::Id)
        .limit(limit + 1);

    // Legacy offset desteği (cursor yoksa)
    if cursor_decoded.is_none() {
        if let Some(off) = params.offset {
            query = query.offset(off as u64);
        }
    }

    let mut rows = query.all(&state.db).await.unwrap_or_default();
    let has_more = rows.len() as u64 > limit;
    if has_more {
        rows.truncate(limit as usize);
    }

    // Karşı taraf bilgilerini toplu çek (N+1 önleme)
    let mut passenger_user_ids: Vec<i64> = Vec::new();
    let mut driver_ids: Vec<i64> = Vec::new();
    for r in &rows {
        if r.user_id > 0 {
            passenger_user_ids.push(r.user_id);
        }
        if let Some(did) = r.driver_id {
            driver_ids.push(did);
        }
    }
    passenger_user_ids.sort_unstable();
    passenger_user_ids.dedup();
    driver_ids.sort_unstable();
    driver_ids.dedup();

    let user_map: std::collections::HashMap<i64, UserModel> = if !passenger_user_ids.is_empty() {
        User::find()
            .filter(crate::modules::auth::models::user::Column::Id.is_in(passenger_user_ids.clone()))
            .all(&state.db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|u| (u.id, u))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    let driver_map: std::collections::HashMap<i64, drivers::Model> = if !driver_ids.is_empty() {
        Driver::find()
            .filter(drivers::Column::Id.is_in(driver_ids.clone()))
            .all(&state.db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|d| (d.id, d))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Item'ları oluştur
    let items: Vec<HistoryItem> = rows
        .iter()
        .map(|r| {
            let counterparty = if resolved_role == "driver" {
                let u = user_map.get(&r.user_id);
                let full_name = u
                    .map(|u| {
                        let fn_ = u.first_name.clone().unwrap_or_default();
                        let ln = u.last_name.clone().unwrap_or_default();
                        let combined = format!("{} {}", fn_, ln).trim().to_string();
                        if combined.is_empty() {
                            u.username.clone()
                        } else {
                            combined
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());
                HistoryCounterparty {
                    user_id: r.user_id,
                    full_name,
                    phone: u.and_then(|u| u.phone_number.clone()),
                    vehicle_model: None,
                    vehicle_plate: None,
                }
            } else {
                let d_opt = r.driver_id.and_then(|did| driver_map.get(&did));
                let full_name = d_opt
                    .map(|d| {
                        if d.full_name.trim().is_empty() {
                            format!("Sürücü #{}", d.id)
                        } else {
                            d.full_name.clone()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());
                HistoryCounterparty {
                    user_id: d_opt.and_then(|d| d.user_id).unwrap_or(0),
                    full_name,
                    phone: d_opt.map(|d| d.phone.clone()),
                    vehicle_model: d_opt.map(|d| d.vehicle_model.clone()),
                    vehicle_plate: d_opt.map(|d| d.vehicle_plate.clone()),
                }
            };

            HistoryItem {
                ride_id: r.id,
                status: r.status.as_str().to_string(),
                pickup_address: r.pickup_address.clone(),
                pickup_lat: r.pickup_lat,
                pickup_lon: r.pickup_lon,
                dropoff_address: r.dropoff_address.clone(),
                dropoff_lat: r.dropoff_lat,
                dropoff_lon: r.dropoff_lon,
                distance_km: r.distance_km,
                duration_sec: r.duration_sec,
                fare_amount: r
                    .fare_amount
                    .as_ref()
                    .and_then(|d| d.to_string().parse::<f64>().ok()),
                requested_at: r.requested_at.to_rfc3339(),
                accepted_at: decode_dt(&r.accepted_at),
                picked_up_at: decode_dt(&r.picked_up_at),
                completed_at: decode_dt(&r.completed_at),
                cancelled_at: decode_dt(&r.cancelled_at),
                counterparty,
            }
        })
        .collect();

    // Sonraki cursor: döndüğümüz son kayıttan üret
    let next_cursor = if has_more {
        rows.last()
            .map(|r| encode_cursor(&r.requested_at, r.id))
    } else {
        None
    };

    tracing::info!(
        user_id,
        role = resolved_role,
        returned = items.len(),
        total,
        has_more,
        "HTTP GET /api/ride/history → 200"
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "role": resolved_role,
            "limit": limit,
            "count": items.len(),
            "has_more": has_more,
            "next_cursor": next_cursor,
            "total": total,
            "rides": items,
        })),
    )
        .into_response()
}

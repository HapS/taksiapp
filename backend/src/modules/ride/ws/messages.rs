use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    LocationUpdate { lat: f64, lon: f64 },
    OfferResponse { ride_id: i64, accepted: bool },
    Ping,
}

/// Ücret detayı — sürücüye teklif anında, yolcuya her aşamada gönderilir.
#[derive(Debug, Serialize, Clone)]
pub struct FareInfo {
    pub opening_fee: f64,
    pub min_fare: f64,
    pub per_km_fee: f64,
    pub estimated_fare: f64,
    pub currency: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    RideOffer {
        ride_id: i64,
        pickup_address: String,
        dropoff_address: String,
        pickup_lat: f64,
        pickup_lon: f64,
        dropoff_lat: f64,
        dropoff_lon: f64,
        distance_km: f64,
        duration_sec: i32,
        fare_amount: f64,
        fare_info: FareInfo,
        expires_in_secs: u32,
    },
    DriverLocation {
        ride_id: i64,
        lat: f64,
        lon: f64,
    },
    RideStatusChanged {
        ride_id: i64,
        status: String,
    },
    OfferExpired {
        ride_id: i64,
    },
    Pong,
    Error {
        message: String,
    },
}

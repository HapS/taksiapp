use crate::modules::ride::ws::messages::ServerMessage;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type Tx = mpsc::UnboundedSender<String>;

pub struct DriverSession {
    pub tx: Tx,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

pub struct PassengerSession {
    pub tx: Tx,
}

#[derive(Default)]
pub struct Hub {
    pub drivers: Arc<DashMap<i64, DriverSession>>,
    pub passengers: Arc<DashMap<i64, PassengerSession>>,
    /// ride_id → (driver_id, user_id)
    pub ride_rooms: Arc<DashMap<i64, (i64, i64)>>,
}

impl Hub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send_to_driver(&self, driver_id: i64, msg: &ServerMessage) {
        if let Some(s) = self.drivers.get(&driver_id) {
            if let Ok(json) = serde_json::to_string(msg) {
                tracing::info!(driver_id, outgoing = %json, "HUB → Sürücü");
                let _ = s.tx.send(json);
            }
        } else {
            tracing::warn!(driver_id, "HUB: Sürücü WS bağlı değil, mesaj gönderilemedi");
        }
    }

    pub fn send_to_passenger(&self, passenger_id: i64, msg: &ServerMessage) {
        if let Some(s) = self.passengers.get(&passenger_id) {
            if let Ok(json) = serde_json::to_string(msg) {
                tracing::info!(passenger_id, outgoing = %json, "HUB → Yolcu");
                let _ = s.tx.send(json);
            }
        } else {
            tracing::warn!(passenger_id, "HUB: Yolcu WS bağlı değil, mesaj gönderilemedi");
        }
    }

    pub fn broadcast_driver_location(&self, ride_id: i64, lat: f64, lon: f64) {
        if let Some(room) = self.ride_rooms.get(&ride_id) {
            let (_, user_id) = *room;
            tracing::info!(ride_id, user_id, lat, lon, "HUB → Sürücü konumu yolcuya iletilıyor");
            self.send_to_passenger(user_id, &ServerMessage::DriverLocation { ride_id, lat, lon });
        } else {
            tracing::warn!(ride_id, "HUB: Ride room bulunamadı, konum yayını iptal");
        }
    }
}

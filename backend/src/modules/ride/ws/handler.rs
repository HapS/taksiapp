use crate::{
    app_state::AppState,
    config::get_config,
    modules::{
        auth::helpers::jwt::{validate_access_token, JwtConfig},
        ride::{
            entities::{
                drivers::{self, Entity as Driver},
                ride_offers::{self, Entity as RideOffer, OfferStatus},
                rides::{self, Entity as Ride, RideStatus},
            },
            ws::{
                hub::DriverSession,
                messages::{ClientMessage, ServerMessage},
            },
        },
    },
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use redis::AsyncCommands;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(serde::Deserialize)]
pub struct WsAuthParams {
    pub token: String,
}

fn verify_ws_token(token: &str) -> Result<i64, ()> {
    let config = get_config();
    let jwt_config = JwtConfig {
        secret: config.jwt_secret().to_string(),
        access_token_expiry: config.jwt_access_token_expiry(),
        refresh_token_expiry: config.jwt_refresh_token_expiry(),
    };
    validate_access_token(token, &jwt_config)
        .map(|claims| claims.sub)
        .map_err(|_| ())
}

pub async fn driver_ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsAuthParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let user_id = match verify_ws_token(&params.token) {
        Ok(id) => id,
        Err(()) => {
            return (axum::http::StatusCode::UNAUTHORIZED, "Geçersiz token").into_response();
        }
    };

    let driver = match Driver::find()
        .filter(drivers::Column::UserId.eq(user_id))
        .filter(drivers::Column::IsActive.eq(true))
        .one(&state.db)
        .await
    {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (axum::http::StatusCode::FORBIDDEN, "Sürücü kaydı bulunamadı").into_response();
        }
        Err(_) => {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "DB hatası").into_response();
        }
    };

    let driver_id = driver.id;

    tracing::info!(driver_id, user_id, "=== WS SÜRÜCÜ BAĞLANDI ===");

    ws.on_upgrade(move |socket| handle_driver_socket(socket, driver_id, Arc::new(state)))
        .into_response()
}

pub async fn passenger_ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsAuthParams>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let user_id = match verify_ws_token(&params.token) {
        Ok(id) => id,
        Err(()) => {
            return (axum::http::StatusCode::UNAUTHORIZED, "Geçersiz token").into_response();
        }
    };

    tracing::info!(user_id, "=== WS YOLCU BAĞLANDI ===");

    ws.on_upgrade(move |socket| handle_passenger_socket(socket, user_id, Arc::new(state)))
        .into_response()
}

async fn handle_driver_socket(socket: WebSocket, driver_id: i64, state: Arc<AppState>) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    state.hub.drivers.insert(driver_id, DriverSession { tx, lat: None, lon: None });

    // DB: online yap (tek sorgu)
    Driver::update_many()
        .filter(drivers::Column::Id.eq(driver_id))
        .set(drivers::ActiveModel {
            is_online: Set(true),
            ..Default::default()
        })
        .exec(&state.db)
        .await
        .ok();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            tracing::info!(driver_id, outgoing = %msg, "WS SÜRÜCÜ ← GÖNDERİLEN");
            if sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = stream.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Ping(_)) => {
                // Axum otomatik pong gönderir, sadece logla
                continue;
            }
            Ok(Message::Close(_)) => {
                tracing::info!(driver_id, "WS Sürücü close frame aldı");
                break;
            }
            Ok(_) => continue,
            Err(e) => {
                tracing::warn!(driver_id, error = %e, "WS Sürücü mesaj okuma hatası");
                break;
            }
        };

        tracing::info!(driver_id, incoming = %text, "WS SÜRÜCÜ → GELEN");
        match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::LocationUpdate { lat, lon }) => {
                tracing::info!(driver_id, lat, lon, "WS Sürücü Konum Güncellemesi");
                if let Some(mut s) = state.hub.drivers.get_mut(&driver_id) {
                    s.lat = Some(lat);
                    s.lon = Some(lon);
                }

                // Redis'e yaz
                let key = format!("driver:{}:location", driver_id);
                let val = format!("{},{}", lat, lon);
                let mut redis = (*state.redis).clone();
                redis.set::<_, _, ()>(&key, &val).await.ok();

                // DB'ye yaz (tek sorgu)
                Driver::update_many()
                    .filter(drivers::Column::Id.eq(driver_id))
                    .set(drivers::ActiveModel {
                        current_lat: Set(Some(lat)),
                        current_lon: Set(Some(lon)),
                        location_updated_at: Set(Some(chrono::Utc::now().into())),
                        ..Default::default()
                    })
                    .exec(&state.db)
                    .await
                    .ok();

                // Aktif ride varsa yolcuya ilet (Accepted veya PickedUp)
                let active_ride = Ride::find()
                    .filter(rides::Column::DriverId.eq(driver_id))
                    .filter(
                        Condition::any()
                            .add(rides::Column::Status.eq(RideStatus::Accepted))
                            .add(rides::Column::Status.eq(RideStatus::PickedUp)),
                    )
                    .one(&state.db)
                    .await
                    .ok()
                    .flatten();

                if let Some(ride) = active_ride {
                    tracing::info!(ride_id = ride.id, driver_id, "WS Sürücü konumu yolcuya iletiliyor");
                    state.hub.broadcast_driver_location(ride.id, lat, lon);
                }
            }
            Ok(ClientMessage::OfferResponse { ride_id, accepted }) => {
                tracing::info!(ride_id, driver_id, accepted, "WS Sürücü Teklif Yanıtı");
                // ride_offers kaydını güncelle
                let offer = RideOffer::find()
                    .filter(ride_offers::Column::RideId.eq(ride_id))
                    .filter(ride_offers::Column::DriverId.eq(driver_id))
                    .filter(ride_offers::Column::Status.eq(OfferStatus::Pending))
                    .one(&state.db)
                    .await
                    .ok()
                    .flatten();

                if let Some(offer) = offer {
                    let mut active: ride_offers::ActiveModel = offer.into();
                    active.status = Set(if accepted { OfferStatus::Accepted } else { OfferStatus::Rejected });
                    active.responded_at = Set(Some(chrono::Utc::now().into()));
                    active.update(&state.db).await.ok();

                    if accepted {
                        // Ride hâlâ searching mi kontrol et — başka sürücü kabul etmiş olabilir
                        if let Ok(Some(ride)) = Ride::find_by_id(ride_id).one(&state.db).await {
                            if ride.status != RideStatus::Searching {
                                // Ride zaten accepted/cancelled — geç kabul, reddet
                                tracing::warn!(ride_id, driver_id, status = ?ride.status, "WS Geç Kabul Reddedildi — ride artık searching değil");
                                if let Some(driver_tx) = state.hub.drivers.get(&driver_id) {
                                    let msg = r#"{"type":"ride_status_changed","ride_id":0,"status":"offer_expired"}"#.to_string();
                                    tracing::info!(driver_id, outgoing = %msg, "WS SÜRÜCÜ ← GÖNDERİLEN");
                                    let _ = driver_tx.tx.send(msg);
                                }
                            } else {
                                let user_id = ride.user_id;

                                let mut active_ride: rides::ActiveModel = ride.into();
                                active_ride.driver_id = Set(Some(driver_id));
                                active_ride.status = Set(RideStatus::Accepted);
                                active_ride.accepted_at = Set(Some(chrono::Utc::now().into()));
                                active_ride.update(&state.db).await.ok();

                                tracing::info!(ride_id, driver_id, user_id, "WS Ride Kabul Edildi — ride_room oluşturuluyor");

                                // Hub'a ride_room kaydı ekle — konum yayını için gerekli
                                state.hub.ride_rooms.insert(ride_id, (driver_id, user_id));

                                // Yolcuya accepted bildirimi gönder
                                state.hub.send_to_passenger(
                                    user_id,
                                    &ServerMessage::RideStatusChanged {
                                        ride_id,
                                        status: "accepted".to_string(),
                                    },
                                );

                                // Sürücüye de accepted onayı gönder — ride detaylarıyla
                                if let Some(driver_tx) = state.hub.drivers.get(&driver_id) {
                                    let _ = driver_tx.tx.send(
                                        serde_json::to_string(&ServerMessage::RideStatusChanged {
                                            ride_id,
                                            status: "accepted".to_string(),
                                        }).unwrap_or_default(),
                                    );
                                }
                            }
                        } else {
                            tracing::error!(ride_id, driver_id, "WS Ride bulunamadı — kabul işlenemedi");
                        }
                    } else {
                        tracing::info!(ride_id, driver_id, "WS Sürücü teklifi reddetti");
                    }
                } else {
                    tracing::warn!(ride_id, driver_id, "WS Bekleyen teklif bulunamadı — sürücü zaten yanıtlamış olabilir");
                }
            }
            Ok(ClientMessage::Ping) => {
                if let Some(s) = state.hub.drivers.get(&driver_id) {
                    let _ = s.tx.send(r#"{"type":"pong"}"#.to_string());
                }
            }
            Err(e) => {
                tracing::warn!(driver_id, error = %e, raw = %text, "WS Bilinmeyen mesaj formatı");
            }
        }
    }

    tracing::info!(driver_id, "=== WS SÜRÜCÜ BAĞLANTISI KESİLDİ ===");

    state.hub.drivers.remove(&driver_id);

    // DB: offline yap (tek sorgu)
    Driver::update_many()
        .filter(drivers::Column::Id.eq(driver_id))
        .set(drivers::ActiveModel {
            is_online: Set(false),
            ..Default::default()
        })
        .exec(&state.db)
        .await
        .ok();

    send_task.abort();
}

async fn handle_passenger_socket(socket: WebSocket, user_id: i64, state: Arc<AppState>) {
    use crate::modules::ride::ws::hub::PassengerSession;

    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    state.hub.passengers.insert(user_id, PassengerSession { tx });

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            tracing::info!(user_id, outgoing = %msg, "WS YOLCU ← GÖNDERİLEN");
            if sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = stream.next().await {
        let text = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Ping(_)) => {
                continue;
            }
            Ok(Message::Close(_)) => {
                tracing::info!(user_id, "WS Yolcu close frame aldı");
                break;
            }
            Ok(_) => continue,
            Err(e) => {
                tracing::warn!(user_id, error = %e, "WS Yolcu mesaj okuma hatası");
                break;
            }
        };

        tracing::info!(user_id, incoming = %text, "WS YOLCU → GELEN");
        if let Ok(ClientMessage::Ping) = serde_json::from_str::<ClientMessage>(&text) {
            if let Some(s) = state.hub.passengers.get(&user_id) {
                let _ = s.tx.send(r#"{"type":"pong"}"#.to_string());
            }
        }
    }

    tracing::info!(user_id, "=== WS YOLCU BAĞLANTISI KESİLDİ ===");

    state.hub.passengers.remove(&user_id);
    send_task.abort();
}
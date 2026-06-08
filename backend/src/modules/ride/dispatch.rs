use crate::{
    app_state::AppState,
    modules::ride::{
        entities::{
            ride_offers::{self, Entity as RideOffer, OfferStatus},
            rides::{self, Entity as Ride, RideStatus},
        },
        ws::messages::ServerMessage,
    },
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, EntityTrait, FromQueryResult, Statement,
};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[derive(Debug, FromQueryResult)]
struct DriverId {
    id: i64,
}

pub async fn dispatch_ride(state: Arc<AppState>, ride_id: i64) {
    tracing::info!(ride_id, "=== DİSPATCH BAŞLADI ===");

    // 1. Ride'ı çek
    let ride = match Ride::find_by_id(ride_id).one(&state.db).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            tracing::error!(ride_id, "dispatch_ride: ride bulunamadı");
            return;
        }
        Err(e) => {
            tracing::error!(ride_id, error = %e, "dispatch_ride: ride çekilemedi");
            return;
        }
    };

    // 2. Yakın sürücüleri bul — aktif ride'ı olanları dışla
    let drivers: Vec<DriverId> = match DriverId::find_by_statement(Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::Postgres,
        r#"
        SELECT id FROM drivers
        WHERE is_online = true
          AND is_active = true
          AND current_lat IS NOT NULL
          AND current_lon IS NOT NULL
          AND ABS(current_lat - $1) < 0.05
          AND ABS(current_lon - $2) < 0.05
          AND NOT EXISTS (
              SELECT 1 FROM ride_offers
              WHERE ride_offers.driver_id = drivers.id
                AND ride_offers.ride_id = $3
          )
          AND NOT EXISTS (
              SELECT 1 FROM rides AS r
              WHERE r.driver_id = drivers.id
                AND r.status IN ('accepted', 'picked_up')
          )
        ORDER BY (ABS(current_lat - $1) + ABS(current_lon - $2)) ASC
        LIMIT 5
        "#,
        [ride.pickup_lat.into(), ride.pickup_lon.into(), ride_id.into()],
    ))
    .all(&state.db)
    .await
    {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(ride_id, error = %e, "dispatch_ride: sürücü sorgusu başarısız");
            return;
        }
    };

    tracing::info!(ride_id, driver_count = drivers.len(), pickup_lat = ride.pickup_lat, pickup_lon = ride.pickup_lon, "Dispatch: yakın sürücüler bulundu");

    // 3. Her sürücüye sırayla teklif gönder
    for (order, driver) in drivers.iter().enumerate() {
        let driver_id = driver.id;

        tracing::info!(ride_id, driver_id, sıra = order, "Dispatch: sürücüye teklif gönderiliyor...");

        // Ride hâlâ searching mi kontrol et (iptal/accepted olabilir)
        if let Ok(Some(r)) = Ride::find_by_id(ride_id).one(&state.db).await {
            if r.status != RideStatus::Searching {
                tracing::info!(ride_id, status = ?r.status, "Dispatch: ride artık searching değil, durduruluyor");
                return;
            }
        }

        // ride_offers insert
        let offer = ride_offers::ActiveModel {
            ride_id: Set(ride_id),
            driver_id: Set(driver_id),
            status: Set(OfferStatus::Pending),
            offer_order: Set(order as i32),
            offered_at: Set(chrono::Utc::now().into()),
            ..Default::default()
        };
        let inserted = match offer.insert(&state.db).await {
            Ok(o) => o,
            Err(e) => {
                tracing::error!(ride_id, driver_id, error = %e, "dispatch_ride: offer insert başarısız");
                continue;
            }
        };

        tracing::info!(ride_id, driver_id, offer_id = inserted.id, "Dispatch: ride_offer kaydı oluşturuldu");

        // Hub üzerinden teklif gönder
        let distance_km = ride.distance_km.unwrap_or(0.0);
        let duration_sec = ride.duration_sec.unwrap_or(0);
        let fare_amount = ride.fare_amount
            .as_ref()
            .and_then(|d| d.to_string().parse::<f64>().ok())
            .unwrap_or(0.0);

        // Ücret detayını hesapla — sürücü teklif anında görmeli
        let fare_cfg = crate::modules::ride::controllers::ride::fetch_fare_config(&state.db).await;
        let estimated_fare = {
            let raw = fare_cfg.opening_fee + distance_km * fare_cfg.per_km_fee;
            let fare = if raw < fare_cfg.min_fare { fare_cfg.min_fare } else { raw };
            (fare * 100.0).round() / 100.0
        };

        state.hub.send_to_driver(
            driver_id,
            &ServerMessage::RideOffer {
                ride_id,
                pickup_address: ride.pickup_address.clone(),
                dropoff_address: ride.dropoff_address.clone(),
                pickup_lat: ride.pickup_lat,
                pickup_lon: ride.pickup_lon,
                dropoff_lat: ride.dropoff_lat,
                dropoff_lon: ride.dropoff_lon,
                distance_km,
                duration_sec,
                fare_amount,
                fare_info: crate::modules::ride::ws::messages::FareInfo {
                    opening_fee: fare_cfg.opening_fee,
                    min_fare: fare_cfg.min_fare,
                    per_km_fee: fare_cfg.per_km_fee,
                    estimated_fare,
                    currency: "TRY",
                },
                expires_in_secs: 30,
            },
        );

        tracing::info!(ride_id, driver_id, "Dispatch: teklif gönderildi, 30sn bekleniyor...");

        sleep(Duration::from_secs(30)).await;

        // 30sn sonra ride hâlâ searching mi tekrar kontrol et
        if let Ok(Some(r)) = Ride::find_by_id(ride_id).one(&state.db).await {
            if r.status != RideStatus::Searching {
                tracing::info!(ride_id, status = ?r.status, "Dispatch: ride artık searching değil (30sn sonra), durduruluyor");
                return;
            }
        }

        // Teklif durumunu kontrol et
        let current_offer = match RideOffer::find_by_id(inserted.id).one(&state.db).await {
            Ok(Some(o)) => o,
            Ok(None) => continue,
            Err(e) => {
                tracing::error!(ride_id, driver_id, error = %e, "dispatch_ride: offer sorgusu başarısız");
                continue;
            }
        };

        match current_offer.status {
            OfferStatus::Accepted => {
                tracing::info!(ride_id, driver_id, "Dispatch: sürücü kabul etti! Yolculuk başlıyor.");
                return; // Kabul edildi, bitti
            }
            OfferStatus::Pending => {
                tracing::info!(ride_id, driver_id, "Dispatch: sürücü yanıtlamadı, timeout");
                // Timeout — güncelle
                let mut active: ride_offers::ActiveModel = current_offer.into();
                active.status = Set(OfferStatus::Timeout);
                if let Err(e) = active.update(&state.db).await {
                    tracing::error!(ride_id, driver_id, error = %e, "dispatch_ride: timeout güncelleme başarısız");
                }
            }
            OfferStatus::Rejected => {
                tracing::info!(ride_id, driver_id, "Dispatch: sürücü reddetti");
            }
            _ => {
                tracing::debug!(ride_id, driver_id, status = ?current_offer.status, "Dispatch: diğer durum");
            }
        }
    }

    // 4. Tüm sürücüler geçildi — no_driver
    tracing::warn!(ride_id, "Dispatch: tüm sürücüler denendi, sürücü bulunamadı");
    if let Ok(Some(ride)) = Ride::find_by_id(ride_id).one(&state.db).await {
        // Hâlâ searching durumundaysa güncelle
        if ride.status == RideStatus::Searching {
            tracing::info!(ride_id, user_id = ride.user_id, "Dispatch: ride no_driver olarak güncelleniyor, yolcuya bildirilecek");
            let mut active: rides::ActiveModel = ride.into();
            active.status = Set(RideStatus::NoDriver);
            if let Err(e) = active.update(&state.db).await {
                tracing::error!(ride_id, error = %e, "dispatch_ride: no_driver güncelleme başarısız");
                return;
            }

            // Yolcuya bildir
            if let Ok(Some(updated_ride)) = Ride::find_by_id(ride_id).one(&state.db).await {
                state.hub.send_to_passenger(
                    updated_ride.user_id,
                    &ServerMessage::RideStatusChanged {
                        ride_id,
                        status: "no_driver".to_string(),
                    },
                );
            }
        } else {
            tracing::info!(ride_id, status = ?ride.status, "Dispatch: ride zaten başka durumda, no_driver gerekmez");
        }
    } else {
        tracing::error!(ride_id, "dispatch_ride: no_driver güncellemesi için ride bulunamadı");
    }

    tracing::info!(ride_id, "=== DİSPATCH TAMAMLANDI ===");
}
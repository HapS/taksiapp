use crate::AppState;
use futures::future::join_all;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use std::{sync::Arc, time::Duration};
use tokio::time;

pub async fn start_location_flush(state: Arc<AppState>) {
    let mut interval = time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        flush_locations(&state).await;
    }
}

async fn flush_locations(state: &AppState) {
    use crate::modules::ride::entities::drivers::{self, Entity as Driver};

    let locations: Vec<(i64, f64, f64)> = state
        .hub
        .drivers
        .iter()
        .filter_map(|e| {
            let lat = e.value().lat?;
            let lon = e.value().lon?;
            Some((*e.key(), lat, lon))
        })
        .collect();

    if locations.is_empty() {
        return;
    }

    let tasks = locations.into_iter().map(|(driver_id, lat, lon)| {
        let db = state.db.clone();
        async move {
            if let Ok(Some(driver)) = Driver::find_by_id(driver_id).one(&db).await {
                let mut active: drivers::ActiveModel = driver.into();
                active.current_lat = Set(Some(lat));
                active.current_lon = Set(Some(lon));
                active.location_updated_at = Set(Some(chrono::Utc::now().into()));
                active.update(&db).await.ok();
            }
        }
    });

    join_all(tasks).await;
}

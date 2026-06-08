//! Background Tasks Module
//!
//! Uygulama başlatıldığında çalışan periyodik görevler.
//! Yeni task eklemek için:
//! 1. Bu modüle yeni bir task fonksiyonu ekle
//! 2. `start_all()` içinde `tokio::spawn` ile başlat

pub mod location_flush;

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// Task süreleri (saniye cinsinden)
pub mod intervals {
    /// Exchange rate güncelleme süresi (1 saat)
    pub const EXCHANGE_RATE_UPDATE: u64 = 60 * 60;

    /// Mail queue işleme süresi (30 saniye)
    pub const MAIL_QUEUE_PROCESS: u64 = 10;
}

/// Tüm background task'ları başlat
/// Main.rs'den bir kez çağrılır
pub fn start_all(db: Arc<DatabaseConnection>) {
    println!("🔄 Background tasks başlatılıyor...");

    // Exchange rate updater - her 1 saatte bir
    start_exchange_rate_updater(db.clone());

    // Mail queue processor - her 30 saniyede bir
    start_mail_queue_processor(db.clone());

    // Buraya yeni task'lar eklenebilir:
    // start_session_cleanup(db.clone());
    // start_cache_warmer(db.clone());

    println!("✅ Background tasks başlatıldı, main.rs çağırdı");
}

/// Exchange rate güncelleme task'ı
/// TCMB'den kurları çekip veritabanına kaydeder
fn start_exchange_rate_updater(db: Arc<DatabaseConnection>) {
    tokio::spawn(async move {
        println!(
            "💱 Exchange rate updater başlatıldı (interval: {} sn)",
            intervals::EXCHANGE_RATE_UPDATE
        );

        // İlk güncellemeyi yap
        match crate::modules::currency::services::exchange_rate_service::fetch_and_save_rates(&*db)
            .await
        {
            Ok(rates) => {
                println!(
                    "💱 İlk kur güncellemesi tamamlandı: USD/TRY={:?}",
                    rates.usd_try
                );
            }
            Err(e) => {
                eprintln!("⚠️  İlk kur güncellemesi başarısız: {}", e);
            }
        }

        // Periyodik güncelleme
        let mut interval = interval(Duration::from_secs(intervals::EXCHANGE_RATE_UPDATE));

        loop {
            interval.tick().await;

            println!("💱 Kurlar güncelleniyor...");
            match crate::modules::currency::services::exchange_rate_service::fetch_and_save_rates(
                &*db,
            )
            .await
            {
                Ok(rates) => {
                    println!(
                        "💱 Kurlar güncellendi: USD/TRY={:?}, EUR/TRY={:?}",
                        rates.usd_try, rates.eur_try
                    );
                }
                Err(e) => {
                    eprintln!("⚠️  Kur güncellemesi başarısız: {}", e);
                }
            }
        }
    });
}

/// Mail queue processor task'ı
/// Kuyruktaki mailleri gönderir
fn start_mail_queue_processor(db: Arc<DatabaseConnection>) {
    tokio::spawn(async move {
        println!(
            "📧 Mail queue processor başlatıldı (interval: {} sn)",
            intervals::MAIL_QUEUE_PROCESS
        );

        // Periyodik işleme
        let mut interval = interval(Duration::from_secs(intervals::MAIL_QUEUE_PROCESS));

        loop {
            interval.tick().await;

            let mail_service = crate::modules::mailer::services::MailService::new((*db).clone());

            match mail_service.process_queue().await {
                Ok(count) => {
                    if count > 0 {
                        println!("📧 {} mail gönderildi", count);
                    }
                }
                Err(e) => {
                    eprintln!("⚠️  Mail queue işleme hatası: {:?}", e);
                }
            }
        }
    });
}

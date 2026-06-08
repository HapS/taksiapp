use crate::modules::admin::models::settings::{
    ActiveModel as SettingsActiveModel, Entity as Settings, SettingsData,
};
use sea_orm::*;
use serde_json;

#[derive(Debug)]
#[allow(dead_code)]
pub enum SettingsServiceError {
    DatabaseError(DbErr),
    SerializationError(serde_json::Error),
}

impl From<DbErr> for SettingsServiceError {
    fn from(err: DbErr) -> Self {
        SettingsServiceError::DatabaseError(err)
    }
}

impl From<serde_json::Error> for SettingsServiceError {
    fn from(err: serde_json::Error) -> Self {
        SettingsServiceError::SerializationError(err)
    }
}

/// Settings'i getir (ID=1 sabit)
pub async fn get_settings(db: &DatabaseConnection) -> Result<SettingsData, SettingsServiceError> {
    let settings = Settings::find_by_id(1).one(db).await?;

    match settings {
        Some(settings_model) => {
            if let Some(data) = settings_model.data {
                let settings_data: SettingsData = serde_json::from_value(data)?;
                Ok(settings_data)
            } else {
                Ok(SettingsData::default())
            }
        }
        None => {
            // İlk kez çalışıyorsa varsayılan ayarları oluştur
            create_default_settings(db).await
        }
    }
}

/// Settings'i güncelle
pub async fn update_settings(
    db: &DatabaseConnection,
    settings_data: SettingsData,
) -> Result<SettingsData, SettingsServiceError> {
    let json_data = serde_json::to_value(&settings_data)?;

    // Mevcut kaydı bul veya oluştur
    let existing = Settings::find_by_id(1).one(db).await?;

    match existing {
        Some(settings_model) => {
            // Güncelle
            let mut active_model: SettingsActiveModel = settings_model.into();
            active_model.data = Set(Some(json_data));
            active_model.updated_at = Set(Some(chrono::Utc::now().into()));

            active_model.update(db).await?;
        }
        None => {
            // Yeni oluştur
            let new_settings = SettingsActiveModel {
                id: Set(1),
                data: Set(Some(json_data)),
                created_at: Set(Some(chrono::Utc::now().into())),
                updated_at: Set(Some(chrono::Utc::now().into())),
            };

            new_settings.insert(db).await?;
        }
    }

    Ok(settings_data)
}

/// Varsayılan ayarları oluştur
async fn create_default_settings(
    db: &DatabaseConnection,
) -> Result<SettingsData, SettingsServiceError> {
    let default_data = SettingsData::default();
    update_settings(db, default_data.clone()).await?;
    Ok(default_data)
}

/// Vocabulary ID'sini al
pub async fn get_vocab_id(db: &DatabaseConnection, vocab_type: &str) -> Option<i64> {
    match get_settings(db).await {
        Ok(settings) => settings.get_vocab_id(vocab_type),
        Err(_) => {
            // Hata durumunda varsayılan değerleri döndür
            match vocab_type {
                "navbar_menu" => Some(1),
                "footer_menu" => Some(2),
                "product_categories" => Some(3),
                "blog_categories" => Some(4),
                "news_categories" => Some(5),
                "page_categories" => Some(6),
                "tags_categories" => Some(7),
                // "vocab_payment_methods" => Some(12),
                _ => None,
            }
        }
    }
}
/// Varsayılan para birimini al (default_currency, yoksa TRY)
pub async fn get_sale_currency(db: &DatabaseConnection) -> Option<String> {
    match get_settings(db).await {
        Ok(settings) => settings.default_currency.or(Some("TRY".to_string())),
        Err(_) => Some("TRY".to_string()),
    }
}

/// Desteklenen para birimlerini al (varsayılan: satış para birimi veya ["TRY"])
pub async fn get_free_shipping_threshold(db: &DatabaseConnection) -> Option<f64> {
    match get_settings(db).await {
        Ok(settings) => settings.free_shipping_threshold.or(Some(500.0)),
        Err(_) => Some(500.0),
    }
}

pub async fn save_extra_settings(
    db: &DatabaseConnection,
    extra_settings: &serde_json::Value,
) -> Result<(), SettingsServiceError> {
    let mut settings = get_settings(db).await?;
    // println!("{:?}", extra_settings);

    settings.free_shipping_threshold = extra_settings
        .get("free_shipping_threshold")
        .and_then(|v| v.as_f64())
        .or(Some(500.0));

    update_settings(db, settings).await?;

    Ok(())
}

use crate::modules::admin::services::settings_service;
use crate::modules::payment_provider::models::*;
use crate::modules::payment_provider::providers::{iyzico::IyzicoProvider, garanti::GarantiProvider, paytr::PaytrProvider};
use sea_orm::DatabaseConnection;
use serde_json;

#[derive(Debug)]
pub enum PaymentProviderError {
    ConfigurationError(String),
    ProviderError(String),
    DatabaseError(sea_orm::DbErr),
    SettingsError(String),
    SerializationError(serde_json::Error),
}

impl std::fmt::Display for PaymentProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentProviderError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            PaymentProviderError::ProviderError(msg) => write!(f, "Provider error: {}", msg),
            PaymentProviderError::DatabaseError(err) => write!(f, "Veritabanı hatası: {}", err),
            PaymentProviderError::SettingsError(msg) => write!(f, "Settings error: {}", msg),
            PaymentProviderError::SerializationError(err) => write!(f, "Serialization error: {}", err),
        }
    }
}

impl std::error::Error for PaymentProviderError {}

impl From<sea_orm::DbErr> for PaymentProviderError {
    fn from(err: sea_orm::DbErr) -> Self {
        PaymentProviderError::DatabaseError(err)
    }
}

impl From<serde_json::Error> for PaymentProviderError {
    fn from(err: serde_json::Error) -> Self {
        PaymentProviderError::SerializationError(err)
    }
}

/// Payment Provider Service
pub struct PaymentProviderService;

impl PaymentProviderService {
    /// Error'u log'la ve döndür
    fn log_error(error: PaymentProviderError) -> PaymentProviderError {
        eprintln!("Payment Provider Error: {}", error);
        error
    }
    
    /// Default payment provider'ı al
    pub async fn get_default_provider(db: &DatabaseConnection) -> Result<PaymentProviderType, PaymentProviderError> {
        let settings = settings_service::get_settings(db).await
            .map_err(|e| Self::log_error(PaymentProviderError::SettingsError(format!("Settings alınamadı: {:?}", e))))?;
        
        let default_provider = settings.default_payment_provider
            .unwrap_or_else(|| "iyzico".to_string());
        
        eprintln!("Default payment provider: {}", default_provider);
        
        PaymentProviderType::from_str(&default_provider)
            .ok_or_else(|| Self::log_error(PaymentProviderError::ConfigurationError("Geçersiz payment provider".to_string())))
    }
    
    /// Payment provider konfigürasyonunu al
    pub async fn get_provider_config(
        db: &DatabaseConnection, 
        provider_type: PaymentProviderType
    ) -> Result<PaymentProviderConfig, PaymentProviderError> {
        let settings = settings_service::get_settings(db).await
            .map_err(|e| Self::log_error(PaymentProviderError::SettingsError(format!("Settings alınamadı: {:?}", e))))?;
        
        let providers_json = settings.payment_providers
            .unwrap_or_else(|| serde_json::json!({}));
        
        let provider_key = provider_type.as_str();
        let provider_config = providers_json.get(provider_key)
            .ok_or_else(|| Self::log_error(PaymentProviderError::ConfigurationError(
                format!("{} provider konfigürasyonu bulunamadı", provider_key)
            )))?;
        
        let config: PaymentProviderConfig = serde_json::from_value(provider_config.clone())
            .map_err(|e| Self::log_error(PaymentProviderError::SerializationError(e)))?;
        Ok(config)
    }
    
    /// Ödeme başlat
    pub async fn initiate_payment(
        db: &DatabaseConnection,
        provider_type: Option<PaymentProviderType>,
        payment_request: PaymentRequest,
        card_data: Option<std::collections::HashMap<String, String>>, // Kredi kartı bilgileri
    ) -> Result<PaymentResponse, PaymentProviderError> {
        // Provider type belirtilmemişse default'u kullan
        let provider = match provider_type {
            Some(p) => {
                eprintln!("Using specified provider: {:?}", p);
                p
            },
            None => {
                let default_provider = Self::get_default_provider(db).await?;
                eprintln!("Using default provider: {:?}", default_provider);
                default_provider
            },
        };
        
        let config = Self::get_provider_config(db, provider.clone()).await?;
        eprintln!("Provider config loaded: enabled={}, test_mode={}", config.enabled, config.test_mode);
        
        if !config.enabled {
            eprintln!("Provider {} is disabled", provider.as_str());
            return Err(Self::log_error(PaymentProviderError::ConfigurationError(
                format!("{} provider devre dışı", provider.as_str())
            )));
        }
        
        match provider {
            PaymentProviderType::Iyzico => {
                eprintln!("Initiating Iyzico payment");
                let iyzico_config: IyzicoConfig = serde_json::from_value(config.config)
                    .map_err(|e| Self::log_error(PaymentProviderError::SerializationError(e)))?;
                IyzicoProvider::initiate_payment(iyzico_config, payment_request, card_data).await
            },
            PaymentProviderType::Garanti => {
                eprintln!("Initiating Garanti payment");
                let garanti_config: GarantiConfig = serde_json::from_value(config.config)
                    .map_err(|e| Self::log_error(PaymentProviderError::SerializationError(e)))?;
                GarantiProvider::initiate_payment(garanti_config, payment_request, card_data).await
            },
            PaymentProviderType::PayTR => {
                eprintln!("Initiating PayTR payment");
                let paytr_config: PaytrConfig = serde_json::from_value(config.config)
                    .map_err(|e| Self::log_error(PaymentProviderError::SerializationError(e)))?;
                PaytrProvider::initiate_payment(paytr_config, payment_request, config.test_mode).await
            },
        }
    }
}
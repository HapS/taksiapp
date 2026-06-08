use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum CampaignError {
    NotFound,
    InvalidParams(String),
    CouponNotFound,
    CouponExpired,
    CouponUsageLimitExceeded,
    CampaignNotActive,
    CampaignUsageLimitExceeded,
    DatabaseError(String),
    Unauthorized,
    CartNotFound,
}

impl std::fmt::Display for CampaignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Kampanya bulunamadı"),
            Self::InvalidParams(msg) => write!(f, "Geçersiz parametreler: {}", msg),
            Self::CouponNotFound => write!(f, "Kupon kodu bulunamadı"),
            Self::CouponExpired => write!(f, "Kupon kodunun süresi dolmuş"),
            Self::CouponUsageLimitExceeded => write!(f, "Kupon kullanım limiti aşıldı"),
            Self::CampaignNotActive => write!(f, "Kampanya aktif değil"),
            Self::CampaignUsageLimitExceeded => write!(f, "Kampanya kullanım limiti aşıldı"),
            Self::DatabaseError(msg) => write!(f, "Veritabanı hatası: {}", msg),
            Self::Unauthorized => write!(f, "Yetkisiz erişim"),
            Self::CartNotFound => write!(f, "Sepet bulunamadı"),
        }
    }
}

impl From<sea_orm::DbErr> for CampaignError {
    fn from(err: sea_orm::DbErr) -> Self {
        CampaignError::DatabaseError(err.to_string())
    }
}

impl IntoResponse for CampaignError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            Self::InvalidParams(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::CouponNotFound => (StatusCode::NOT_FOUND, self.to_string()),
            Self::CouponExpired => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::CouponUsageLimitExceeded => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::CampaignNotActive => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::CampaignUsageLimitExceeded => (StatusCode::BAD_REQUEST, self.to_string()),
            Self::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Sunucu hatası".to_string(),
            ),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            Self::CartNotFound => (StatusCode::NOT_FOUND, self.to_string()),
        };

        (
            status,
            Json(json!({
                "success": false,
                "error": message
            })),
        )
            .into_response()
    }
}

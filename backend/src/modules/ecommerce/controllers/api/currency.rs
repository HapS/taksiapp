// Currency API Controller - Frontend para birimi listesi ve değiştirme
use crate::app_state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

/// Para birimi bilgisi (frontend dropdown için)
#[derive(Debug, Serialize, Clone)]
pub struct CurrencyInfo {
    pub code: String,
    pub name: String,
    pub symbol: String,
    pub flag: String,
}

#[derive(Debug, Serialize)]
pub struct CurrencyListResponse {
    pub success: bool,
    pub currencies: Vec<CurrencyInfo>,
    pub current_currency: String,
    pub sale_currency: String,
}

#[derive(Debug, Deserialize)]
pub struct SetCurrencyRequest {
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct SetCurrencyResponse {
    pub success: bool,
    pub currency: String,
    pub message: Option<String>,
}

/// Para birimi kodu -> detay bilgisi eşlemesi
fn get_currency_details(code: &str) -> CurrencyInfo {
    match code {
        "TRY" => CurrencyInfo {
            code: "TRY".to_string(),
            name: "Türk Lirası".to_string(),
            symbol: "₺".to_string(),
            flag: "🇹🇷".to_string(),
        },
        "USD" => CurrencyInfo {
            code: "USD".to_string(),
            name: "US Dollar".to_string(),
            symbol: "$".to_string(),
            flag: "🇺🇸".to_string(),
        },
        "EUR" => CurrencyInfo {
            code: "EUR".to_string(),
            name: "Euro".to_string(),
            symbol: "€".to_string(),
            flag: "🇪🇺".to_string(),
        },
        "GBP" => CurrencyInfo {
            code: "GBP".to_string(),
            name: "British Pound".to_string(),
            symbol: "£".to_string(),
            flag: "🇬🇧".to_string(),
        },
        "CHF" => CurrencyInfo {
            code: "CHF".to_string(),
            name: "Swiss Franc".to_string(),
            symbol: "CHF".to_string(),
            flag: "🇨🇭".to_string(),
        },
        "AUD" => CurrencyInfo {
            code: "AUD".to_string(),
            name: "Australian Dollar".to_string(),
            symbol: "A$".to_string(),
            flag: "🇦🇺".to_string(),
        },
        "CAD" => CurrencyInfo {
            code: "CAD".to_string(),
            name: "Canadian Dollar".to_string(),
            symbol: "C$".to_string(),
            flag: "🇨🇦".to_string(),
        },
        "AZN" => CurrencyInfo {
            code: "AZN".to_string(),
            name: "Azerbaycan Manatı".to_string(),
            symbol: "₼".to_string(),
            flag: "🇦🇿".to_string(),
        },
        "JPY" => CurrencyInfo {
            code: "JPY".to_string(),
            name: "Japanese Yen".to_string(),
            symbol: "¥".to_string(),
            flag: "🇯🇵".to_string(),
        },
        _ => CurrencyInfo {
            code: code.to_string(),
            name: code.to_string(),
            symbol: code.to_string(),
            flag: "🏳️".to_string(),
        },
    }
}

/// GET /api/currencies - Desteklenen para birimlerini listele
///
/// Frontend'de currency dropdown'ı oluşturmak için kullanılır.
/// Session'daki mevcut tercihi ve admin ayarlarındaki desteklenen para birimlerini döner.
pub async fn list_currencies(State(state): State<AppState>, session: Session) -> Response {
    // Settings cache'den desteklenen para birimlerini al
    let (supported_codes, sale_currency) = if let Ok(settings) = state.settings_cache.read() {
        let supported = settings.get_supported_currencies();
        let sale_cur = settings.get_sale_currency();
        (supported, sale_cur)
    } else {
        (vec!["TRY".to_string()], "TRY".to_string())
    };

    // Session'dan mevcut para birimi tercihini al
    let current_currency = session
        .get::<String>("display_currency")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| sale_currency.clone());

    // Desteklenen para birimlerini detaylı bilgileriyle döndür
    let currencies: Vec<CurrencyInfo> = supported_codes
        .iter()
        .map(|code| get_currency_details(code))
        .collect();

    (
        StatusCode::OK,
        Json(CurrencyListResponse {
            success: true,
            currencies,
            current_currency,
            sale_currency,
        }),
    )
        .into_response()
}

/// PUT /api/currencies/current - Kullanıcının para birimi tercihini değiştir
///
/// Session ve cookie'ye seçilen para birimini kaydeder.
/// Sayfa yenilendiğinde veya sonraki API çağrılarında bu tercih kullanılır.
/// B2B kullanıcıları sadece şirketlerine tanımlı para birimini seçebilir.
pub async fn set_currency(
    State(state): State<AppState>,
    session: Session,
    Json(request): Json<SetCurrencyRequest>,
) -> Response {
    let currency_code = request.currency.trim().to_uppercase();

    // Kullanıcı bilgilerini session'dan al
    let user_data = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
        .unwrap_or(None);

    // B2B kullanıcısı mı kontrol et (check_user_has_b2b_access fonksiyonunu kullan)
    let is_b2b_user = if let Some(ref user) = user_data {
        crate::modules::ecommerce::services::cart_service::check_user_has_b2b_access(
            &state.db, user.user_id,
        )
        .await
    } else {
        false
    };

    // B2B kullanıcısı ise, şirket para birimini kontrol et
    if is_b2b_user {
        if let Some(ref user) = user_data {
            // Kullanıcının şirketini bul
            use crate::modules::b2b::services::company_service::CompanyService;

            if let Ok(Some(company)) =
                CompanyService::get_company_by_user_id(&state.db, user.user_id).await
            {
                let company_currency = company.currency.as_deref().unwrap_or("TRY");

                // Seçilen para birimi şirket para birimi değilse engelle
                if currency_code != company_currency {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(SetCurrencyResponse {
                            success: false,
                            currency: currency_code,
                            message: Some(format!(
                                "B2B kullanıcıları sadece '{}' para birimini kullanabilir.",
                                company_currency
                            )),
                        }),
                    )
                        .into_response();
                }
            }
        }
    }

    // Desteklenen para birimleri arasında mı kontrol et
    let is_supported = if let Ok(settings) = state.settings_cache.read() {
        settings.get_supported_currencies().contains(&currency_code)
    } else {
        false
    };

    if !is_supported {
        return (
            StatusCode::BAD_REQUEST,
            Json(SetCurrencyResponse {
                success: false,
                currency: currency_code.clone(),
                message: Some(format!(
                    "'{}' desteklenen para birimleri arasında değil.",
                    currency_code
                )),
            }),
        )
            .into_response();
    }

    // Session'a kaydet
    if let Err(e) = session.insert("display_currency", &currency_code).await {
        eprintln!("Session currency kaydetme hatası: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SetCurrencyResponse {
                success: false,
                currency: currency_code,
                message: Some("Para birimi tercihi kaydedilemedi.".to_string()),
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(SetCurrencyResponse {
            success: true,
            currency: currency_code,
            message: Some("Para birimi tercihi güncellendi.".to_string()),
        }),
    )
        .into_response()
}

/// GET /api/currencies/current - Mevcut para birimi tercihini getir
///
/// Session'dan kullanıcının aktif para birimi tercihini döner.
pub async fn get_current_currency(State(state): State<AppState>, session: Session) -> Response {
    let sale_currency = if let Ok(settings) = state.settings_cache.read() {
        settings.get_sale_currency()
    } else {
        "TRY".to_string()
    };

    let current_currency = session
        .get::<String>("display_currency")
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| sale_currency.clone());

    (
        StatusCode::OK,
        Json(SetCurrencyResponse {
            success: true,
            currency: current_currency,
            message: None,
        }),
    )
        .into_response()
}

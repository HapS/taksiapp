use crate::modules::currency::models::{ExchangeRate, ExchangeRateActiveModel, ExchangeRateModel};
use lazy_static::lazy_static;
use reqwest::Client;
use sea_orm::*;
use std::sync::RwLock;

/// Exchange rate cache - bellekte tutulan son kur bilgisi
#[derive(Clone, Debug)]
pub struct ExchangeRateCache {
    pub rates: Option<ExchangeRateModel>,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for ExchangeRateCache {
    fn default() -> Self {
        Self {
            rates: None,
            last_updated: None,
        }
    }
}

lazy_static! {
    /// Global exchange rate cache
    static ref EXCHANGE_RATE_CACHE: RwLock<ExchangeRateCache> = RwLock::new(ExchangeRateCache::default());
}

/// Cache'den kurları al (cache yoksa veya 1 saatten eskiyse DB'den yükle)
pub async fn get_cached_rates(db: &impl ConnectionTrait) -> Option<ExchangeRateModel> {
    // Önce cache'i kontrol et
    {
        let cache = EXCHANGE_RATE_CACHE.read().ok()?;
        if let Some(ref rates) = cache.rates {
            if let Some(last_updated) = cache.last_updated {
                // 1 saatten yeni ise cache'den döndür
                let now = chrono::Utc::now();
                if now.signed_duration_since(last_updated).num_hours() < 1 {
                    return Some(rates.clone());
                }
            }
        }
    }

    // Cache yoksa veya eskiyse DB'den yükle
    if let Ok(Some(rates)) = get_latest_rates(db).await {
        if let Ok(mut cache) = EXCHANGE_RATE_CACHE.write() {
            cache.rates = Some(rates.clone());
            cache.last_updated = Some(chrono::Utc::now());
        }
        return Some(rates);
    }

    None
}

/// Cache'i manuel olarak yenile
#[allow(dead_code)]
pub fn refresh_cache(rates: ExchangeRateModel) {
    if let Ok(mut cache) = EXCHANGE_RATE_CACHE.write() {
        cache.rates = Some(rates);
        cache.last_updated = Some(chrono::Utc::now());
    }
}

/// Bir para biriminden diğerine çevir
/// Örnek: convert_currency(100.0, "USD", "TRY", &rates) -> USD'yi TRY'ye çevirir
pub fn convert_currency(
    amount: f64,
    from_currency: &str,
    to_currency: &str,
    rates: &ExchangeRateModel,
) -> Option<f64> {
    let from = from_currency.to_uppercase();
    let to = to_currency.to_uppercase();

    // Aynı para birimi ise direkt döndür
    if from == to {
        return Some(amount);
    }

    // Önce TRY'ye çevir, sonra hedef para birimine çevir
    let amount_in_try = convert_to_try(amount, &from, rates)?;
    convert_from_try(amount_in_try, &to, rates)
}

#[derive(Debug)]
pub enum ExchangeRateError {
    FetchError(String),
    ParseError(String),
    DatabaseError(DbErr),
}

impl std::fmt::Display for ExchangeRateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExchangeRateError::FetchError(msg) => write!(f, "Fetch error: {}", msg),
            ExchangeRateError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ExchangeRateError::DatabaseError(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl From<DbErr> for ExchangeRateError {
    fn from(err: DbErr) -> Self {
        ExchangeRateError::DatabaseError(err)
    }
}

/// TCMB XML response'dan parse edilen kur bilgisi
#[derive(Debug, Default)]
struct TcmbRates {
    usd_try: Option<f64>,
    eur_try: Option<f64>,
    gbp_try: Option<f64>,
    chf_try: Option<f64>,
    aud_try: Option<f64>,
    cad_try: Option<f64>,
    azn_try: Option<f64>,
    jpy_try: Option<f64>,
}

/// TCMB'den güncel kurları çek
async fn fetch_tcmb_rates() -> Result<TcmbRates, ExchangeRateError> {
    let client = Client::builder()
        .danger_accept_invalid_certs(true) // TCMB SSL sertifikası bazen sorun çıkarıyor
        .build()
        .map_err(|e| ExchangeRateError::FetchError(e.to_string()))?;

    let url = "https://www.tcmb.gov.tr/kurlar/today.xml";

    let response = client
        .get(url)
        .header("Content-Type", "text/xml")
        .send()
        .await
        .map_err(|e| ExchangeRateError::FetchError(e.to_string()))?;

    let xml_content = response
        .text()
        .await
        .map_err(|e| ExchangeRateError::FetchError(e.to_string()))?;

    // XML parse
    let doc = roxmltree::Document::parse(&xml_content)
        .map_err(|e| ExchangeRateError::ParseError(e.to_string()))?;

    let mut rates = TcmbRates::default();

    // Currency elementlerini bul
    for node in doc.descendants() {
        if node.tag_name().name() == "Currency" {
            if let Some(kod) = node.attribute("Kod") {
                // Kur değerini al (Önce BanknoteSelling, yoksa ForexSelling)
                let banknote_selling = node
                    .children()
                    .find(|n| n.tag_name().name() == "BanknoteSelling")
                    .and_then(|n| n.text())
                    .and_then(|t| t.parse::<f64>().ok());

                let forex_selling = node
                    .children()
                    .find(|n| n.tag_name().name() == "ForexSelling")
                    .and_then(|n| n.text())
                    .and_then(|t| t.parse::<f64>().ok());

                let selling_value = banknote_selling.or(forex_selling);

                match kod {
                    "USD" => rates.usd_try = selling_value,
                    "EUR" => rates.eur_try = selling_value,
                    "GBP" => rates.gbp_try = selling_value,
                    "CHF" => rates.chf_try = selling_value,
                    "AUD" => rates.aud_try = selling_value,
                    "CAD" => rates.cad_try = selling_value,
                    "AZN" => rates.azn_try = selling_value,
                    "JPY" => {
                        // TCMB JPY kurlarını genellikle 100 birim üzerinden verir
                        let unit = node
                            .children()
                            .find(|n| n.tag_name().name() == "Unit")
                            .and_then(|n| n.text())
                            .and_then(|t| t.parse::<f64>().ok())
                            .unwrap_or(1.0);

                        rates.jpy_try = selling_value.map(|v| v / unit);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(rates)
}

/// Tüm kurları çekip veritabanına kaydet
pub async fn fetch_and_save_rates(
    db: &impl ConnectionTrait,
) -> Result<ExchangeRateModel, ExchangeRateError> {
    // TCMB'den kurları çek
    let tcmb_rates = fetch_tcmb_rates().await?;

    // EUR/USD hesapla (TCMB verilerinden)
    let eur_usd = match (tcmb_rates.eur_try, tcmb_rates.usd_try) {
        (Some(eur), Some(usd)) if usd > 0.0 => Some(eur / usd),
        _ => None,
    };

    // Veritabanına kaydet
    let now = chrono::Utc::now();

    let rate = ExchangeRateActiveModel {
        usd_try: Set(tcmb_rates
            .usd_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        eur_try: Set(tcmb_rates
            .eur_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        gbp_try: Set(tcmb_rates
            .gbp_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        chf_try: Set(tcmb_rates
            .chf_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        aud_try: Set(tcmb_rates
            .aud_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        cad_try: Set(tcmb_rates
            .cad_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        azn_try: Set(tcmb_rates
            .azn_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        jpy_try: Set(tcmb_rates
            .jpy_try
            .map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        eur_usd: Set(eur_usd.map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default())),
        source: Set(Some("tcmb".to_string())),
        created_at: Set(Some(now.into())),
        ..Default::default()
    };

    let result = rate.insert(db).await?;

    println!(
        "✅ Exchange rates saved: EUR/TRY={:?}, USD/TRY={:?}, AZN/TRY={:?}, JPY/TRY={:?}",
        result.eur_try, result.usd_try, result.azn_try, result.jpy_try
    );

    Ok(result)
}

/// En son kur bilgisini getir
pub async fn get_latest_rates(db: &impl ConnectionTrait) -> Result<Option<ExchangeRateModel>, DbErr> {
    ExchangeRate::find()
        .order_by_desc(crate::modules::currency::models::exchange_rate::Column::CreatedAt)
        .one(db)
        .await
}

/// Belirli bir tarihteki (veya o tarihe en yakın geçmişteki) kur bilgisini getir
pub async fn get_rates_at_date(
    db: &impl ConnectionTrait,
    date: chrono::DateTime<chrono::Utc>,
) -> Result<Option<ExchangeRateModel>, DbErr> {
    ExchangeRate::find()
        .filter(crate::modules::currency::models::exchange_rate::Column::CreatedAt.lte(date))
        .order_by_desc(crate::modules::currency::models::exchange_rate::Column::CreatedAt)
        .one(db)
        .await
}

/// Belirli bir para birimini TRY'ye çevir
pub fn convert_to_try(amount: f64, currency: &str, rates: &ExchangeRateModel) -> Option<f64> {
    match currency.to_uppercase().as_str() {
        "TRY" => Some(amount),
        "USD" => rates
            .usd_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        "EUR" => rates
            .eur_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        "GBP" => rates
            .gbp_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        "CHF" => rates
            .chf_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        "AUD" => rates
            .aud_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        "CAD" => rates
            .cad_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        "AZN" => rates
            .azn_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        "JPY" => rates
            .jpy_try
            .map(|r| amount * r.to_string().parse::<f64>().unwrap_or(0.0)),
        _ => None,
    }
}

/// TRY'den belirli bir para birimine çevir
pub fn convert_from_try(amount: f64, currency: &str, rates: &ExchangeRateModel) -> Option<f64> {
    match currency.to_uppercase().as_str() {
        "TRY" => Some(amount),
        "USD" => rates.usd_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        "EUR" => rates.eur_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        "GBP" => rates.gbp_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        "CHF" => rates.chf_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        "AUD" => rates.aud_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        "CAD" => rates.cad_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        "AZN" => rates.azn_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        "JPY" => rates.jpy_try.map(|r| {
            let rate = r.to_string().parse::<f64>().unwrap_or(1.0);
            if rate > 0.0 {
                amount / rate
            } else {
                0.0
            }
        }),
        _ => None,
    }
}

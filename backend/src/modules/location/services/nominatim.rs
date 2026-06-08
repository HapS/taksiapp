use serde::{Deserialize, Serialize};
use tracing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeocodeResult {
    pub display_name: String,
    pub lat: f64,
    pub lon: f64,
    pub category: Option<String>,
}

pub async fn search_nominatim(
    query: &str,
    limit: usize,
) -> Result<Vec<GeocodeResult>, String> {
    let encoded = urlencoding::encode(query);
    let url = format!(
        "https://nominatim.openstreetmap.org/search?q={}&format=json&limit={}&accept-language=tr&countrycodes=tr&viewbox=25.5,42.0,45.5,35.5&bounded=0",
        encoded, limit.min(10),
    );

    tracing::info!(query = %query, "Nominatim araması");

    let client = reqwest::Client::builder()
        .user_agent("TaksimApp/1.0")
        .build()
        .map_err(|e| format!("HTTP client hatası: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Nominatim istek hatası: {e}"))?;

    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "Nominatim yanıt başarısız");
        return Ok(vec![]);
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Nominatim body okuma hatası: {e}"))?;

    let items: Vec<NominatimItem> = serde_json::from_str(&body)
        .map_err(|e| format!("Nominatim JSON parse hatası: {e}"))?;

    let results: Vec<GeocodeResult> = items
        .into_iter()
        .filter_map(|item| {
            let lat: f64 = item.lat.parse().ok()?;
            let lon: f64 = item.lon.parse().ok()?;
            if lat == 0.0 && lon == 0.0 {
                return None;
            }
            Some(GeocodeResult {
                display_name: item.display_name,
                lat,
                lon,
                category: item.class.as_ref().and_then(|c| {
                    match c.as_str() {
                        "restaurant" | "cafe" | "fast_food" => Some("yemek"),
                        "hospital" | "clinic" => Some("saglik"),
                        "school" | "university" => Some("egitim"),
                        "bus_station" | "station" => Some("ulasim"),
                        "mall" | "supermarket" | "market" => Some("alisveris"),
                        _ => Some("diger"),
                    }.map(|s| s.to_string())
                }),
            })
        })
        .collect();

    tracing::info!(query = %query, count = results.len(), "Nominatim sonuçları");
    Ok(results)
}

#[derive(Debug, Deserialize)]
struct NominatimItem {
    lat: String,
    lon: String,
    display_name: String,
    class: Option<String>,
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 3);
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push('%');
                    result.push_str(&format!("{byte:02X}"));
                }
            }
        }
        result
    }
}
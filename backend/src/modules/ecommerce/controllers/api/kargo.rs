use crate::app_state::AppState;
use crate::modules::ecommerce::models::{kargo_sirketleri, KargoSirketleriEntity};
use axum::{
    extract::{Extension, State},
    response::Json,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct KargoSirketiFrontendResponse {
    pub id: i32,
    pub title: String,
    pub logo: String,
    pub default: Option<bool>,
}

/// List all shipping providers
use axum::http::StatusCode;

pub async fn get_shipping_providers(
    State(state): State<AppState>,
    Extension(current_language): Extension<crate::middleware::global_context::CurrentLanguage>,
) -> (StatusCode, Json<serde_json::Value>) {
    let lang = current_language;

    // println!("kargo seçenekleri lang {}", lang.0);

    match KargoSirketleriEntity::find()
        .filter(kargo_sirketleri::Column::Publish.eq(true))
        .order_by_asc(kargo_sirketleri::Column::Id)
        .all(&state.db)
        .await
    {
        Ok(entities) => {
            let data: Vec<_> = entities
                .into_iter()
                .map(|kargo| KargoSirketiFrontendResponse {
                    id: kargo.id,
                    title: kargo
                        .data
                        .get("langs")
                        .and_then(|l| l.get(&lang.0))
                        .and_then(|t| t.get("title"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("Admin -> Setting -> Kargo Ayarlarında görünen isimleri değiştirmek gerekiyor")
                        .to_string(),
                    logo: kargo.logo,
                    default: Some(kargo.default),
                })
                .collect();

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "success",
                    "data": data
                })),
            )
        }
        Err(err) => {
            eprintln!("Error: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Veritabanı hatası"
                })),
            )
        }
    }
}

use crate::app_state::AppState;
use crate::modules::search::services::search_service::SearchService;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Json},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct SearchApiParams {
    pub q: String,
    pub type_: Option<String>,
    pub lang: Option<String>,
}

#[derive(Serialize)]
pub struct SearchResultItem {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub image: String,
    pub code: String, // Stock code for products, or slug/id for others
    pub url: String,
    pub content_type: String,
}

pub async fn search_api(
    State(state): State<AppState>,
    Query(params): Query<SearchApiParams>,
) -> impl IntoResponse {
    let lang = params.lang.unwrap_or_else(|| "tr".to_string());
    let type_filter = params.type_.clone();

    // Default to product if only searching without specific intent?
    // No, API should respect the filter. If None, search all or specific default?
    // User interface says default product. Frontend will pass "product".

    let results =
        match SearchService::search_content(&state.db, &params.q, type_filter, &lang).await {
            Ok(items) => items,
            Err(e) => {
                eprintln!("Search error: {}", e);
                return Json(vec![]);
            }
        };

    let mapped_results: Vec<SearchResultItem> = results
        .into_iter()
        .map(|item| {
            // Extract localized data
            let langs = item.data.get("langs");
            let lang_data = langs.and_then(|l| l.get(&lang));

            println!("Lang data: {:#?}", &lang_data);

            let title = lang_data
                .and_then(|d| d.get("title"))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let description = lang_data
                .and_then(|d| d.get("short_description").or(d.get("description")))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            // Image
            // Assuming image is in data->'image' or lang_data->'image'
            let image = lang_data
                .and_then(|d| d.get("media"))
                .and_then(|s| s.get("cover"))
                .and_then(|s| s.get(0))
                .and_then(|s| s.get("url"))
                .and_then(|s| s.as_str())
                .unwrap_or("/static/no_image.png")
                .to_string();

            // Code
            // For products: item.data["stock_code"] ?
            // For others, maybe empty or slug?
            let code = item
                .data
                .get("stock_code")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            // URL
            let url = item.get_absolute_url(&lang).unwrap_or_default();

            SearchResultItem {
                id: item.id,
                title,
                description,
                image,
                code,
                url,
                content_type: item.content_type,
            }
        })
        .collect();

    Json(mapped_results)
}

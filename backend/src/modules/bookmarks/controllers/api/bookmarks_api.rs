use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::bookmarks::services::bookmark_service::BookmarkService;
use crate::modules::content::services::product_service;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Json, Response},
    Extension,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CreateBookmarkRequest {
    pub content_id: i64,
    pub content_type: String,
    pub variant_key: Option<String>,
}

// #[derive(Deserialize)]
// pub struct ResponseBookmarkRequest {
//     pub id: i64,
//     pub user_id: i64,
//     pub module_name: String,
//     pub content_type: String,
//     pub content_id: i64,
//     pub title: String,
//     pub price: Option<f64>,
//     pub link: String,
// }

/// Geçerli kullanıcı için tüm yer imlerini listeler
///
/// Ürünler, sayfalar vb. dahil olmak üzere içerik türleri arasındaki yer imlerini dönen birleşik servisi kullanır.
/// Bu, sayfaların da ürünlerle birlikte listelenmesini sağlar.
pub async fn list_bookmarks(
    State(state): State<AppState>,
    Extension(current_language): Extension<crate::middleware::global_context::CurrentLanguage>,
    auth_user: AuthenticatedUser,
) -> Json<serde_json::Value> {
    // TODO: Gerekirse content_type ile filtrelemek için isteğe bağlı bir sorgu parametresi ekle
    let lang = &current_language.0;
    match BookmarkService::list_bookmarks(&state.db, auth_user.id, lang).await {
        Ok(bookmarks) => Json(serde_json::json!({
            "status": "success",
            "current_language": lang,
            "data": bookmarks
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "current_language": lang,
            "message": e.to_string()
        })),
    }
}

/// Yeni bir yer imi oluştur
pub async fn create_bookmark(
    State(state): State<AppState>,
    Extension(current_language): Extension<crate::middleware::global_context::CurrentLanguage>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<CreateBookmarkRequest>,
) -> Response {
    let user_id = auth_user.id;

    // Varsayılan / yer tutucu değerler (ürün favorileri için üzerine yazılacak)
    let mut title = "favori başlığı".to_string();
    let mut module_name = "varsayılan_modül".to_string();
    // Biçimlendirilmiş görüntüleme fiyatını sakla (örn: "₺3.499,00")
    let mut price: Option<String> = None;

    // Servis çağrısına taşıyacağımız alanları klonla
    let content_type = payload.content_type.clone();
    let variant_key = payload.variant_key.clone();

    // Eğer bu bir ürün favorisi ise, backend'de uygun fiyatı çözümle
    if content_type == "product" {
        match product_service::get_product(
            &state.db,
            &current_language.0,
            None,
            Some(payload.content_id),
            None,
        )
        .await
        {
            Ok(prod) => {
                // Ürün başlığı ve modül adını kullan
                title = prod.title.clone();
                module_name = "content".to_string();

                // Eğer varyant anahtarı sağlandıysa, eşleşen varyantı bul ve biçimlendirilmiş görüntüleme fiyatını kullan
                if let Some(ref vk) = variant_key {
                    if let Some(ref prod_json) = prod.product {
                        if let Some(variants) = prod_json.get("variants").and_then(|v| v.as_array())
                        {
                            for variant in variants.iter() {
                                if let Some(opt_disp) = variant
                                    .get("option_values_display")
                                    .and_then(|v| v.as_str())
                                {
                                    if opt_disp.trim() == vk.trim() {
                                        // Eğer mevcutsa backend hazırlıklı biçimlendirilmiş stringi tercih et,
                                        // aksi halde sayısal görüntüleme fiyatını ürünün görüntüleme para birimine biçimlendir
                                        price = variant
                                            .get("display_price_formatted")
                                            .and_then(|p| p.as_str().map(|s| s.to_string()))
                                            .or_else(|| {
                                                variant
                                                    .get("display_price")
                                                    .and_then(|p| p.as_f64())
                                                    .map(|n| {
                                                        crate::modules::utils::format_price::format_price(
                                                            n,
                                                            prod.display_currency.as_deref().unwrap_or("TRY"),
                                                        )
                                                    })
                                            })
                                            .or_else(|| {
                                                variant
                                                    .get("price")
                                                    .and_then(|p| p.as_f64())
                                                    .map(|n| {
                                                        crate::modules::utils::format_price::format_price(
                                                            n,
                                                            prod.display_currency.as_deref().unwrap_or("TRY"),
                                                        )
                                                    })
                                            });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                // Varyant biçimlendirilmiş fiyatı alamadıysak, ürünün biçimlendirilmiş fiyatına geri dön (varsa)
                if price.is_none() {
                    price = prod.price_formatted.clone();
                }
            }
            Err(e) => {
                // Fiyat sorgulama sorunları nedeniyle favori oluşturmayı başarısız yapma.
                // Kaydet ve varsayılanlarla devam et (fiyat olmadan).
                eprintln!("Favori fiyat çözümlemesi için ürün getirilemedi: {:?}", e);
            }
        }
    }

    match BookmarkService::create_bookmark(
        &state.db,
        user_id,
        title,
        content_type,
        payload.content_id,
        module_name,
        price,
        variant_key,
    )
    .await
    {
        Ok(bookmark) => Json(serde_json::json!({
            "status": "success",
            "data": bookmark,
            "bookmark_product_count": BookmarkService::bookmarks_product_count(&state.db, user_id).await.unwrap_or(0)
        })).into_response(),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })).into_response(),
    }
}

/// Bir yer imini sil
pub async fn delete_bookmark(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    match BookmarkService::delete_bookmark(&state.db, id, auth_user.id).await {
        Ok(_) => Json(serde_json::json!({
            "status": "success",
            "message": "Favori başarıyla silindi",
            "bookmark_product_count": BookmarkService::bookmarks_product_count(&state.db, auth_user.id).await.unwrap_or(0)
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

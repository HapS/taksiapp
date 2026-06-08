use crate::modules::bookmarks::entities::bookmarks_entities::{
    self, Entity as Bookmarks, Model as BookmarkModel,
};
// use crate::modules::content::models::content;
use anyhow::Result;

use sea_orm::*;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::modules::content::helpers::ProductResponse;
use crate::modules::content::services::product_service;

use crate::modules::content::helpers::PageResponse;
use crate::modules::content::models::content::Column as ContentColumn;
use crate::modules::content::models::Content;
use crate::modules::content::services::page_service;

#[derive(Debug, FromQueryResult)]
pub struct BookmarkRow {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub content_type: String,
    pub content_id: i64,
    pub module_name: String,
    pub price: Option<String>,
    pub variant_key: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    // Not: Bu sorguda yerelleştirilmiş içerik alanları ve medya doğrudan SQL ile seçilmez.
    // Aşağıda içerik satırlarını (yayınlanmamış/silinmiş olanlar dahil) açıkça getiriyoruz ve bunları
    // ilgili içerik varlığı eksik olduğunda bunları yedek veri olarak kullanıyoruz.
}

#[derive(Serialize, Clone)]
pub struct BookmarkResponse {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub content_type: String,
    pub content_id: i64,
    pub module_name: String,
    pub price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant_key: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,

    // Önyüz kullanım kolaylığı için türetilmiş alanlar (varsa ürün veya sayfadan doldurulur)
    pub content_title: Option<String>,
    pub content_description: Option<String>,
    pub media: Option<JsonValue>,
    pub link: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<ProductResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<PageResponse>,
}

pub struct BookmarkService;

impl BookmarkService {
    /// Farklı içerik türleri (ürün, sayfa, ...) arasındaki yer imlerini birleştirilmiş şekilde listeler.
    /// Ürünleri ve sayfaları toplu olarak getirerek N+1 sorgusu oluşmasını engeller ve birleşik bir yanıt döner.
    pub async fn list_bookmarks(
        db: &DatabaseConnection,
        user_id: i64,
        lang: &str,
    ) -> Result<Vec<BookmarkResponse>> {
        let rows = Bookmarks::find()
            .filter(bookmarks_entities::Column::UserId.eq(user_id))
            .select_only()
            // Bookmarks tablosundan sütunları seç
            .column(bookmarks_entities::Column::Id)
            .column(bookmarks_entities::Column::UserId)
            .column(bookmarks_entities::Column::Title)
            .column(bookmarks_entities::Column::ContentType)
            .column(bookmarks_entities::Column::ContentId)
            .column(bookmarks_entities::Column::ModuleName)
            .column(bookmarks_entities::Column::Price)
            .column(bookmarks_entities::Column::VariantKey)
            .column(bookmarks_entities::Column::CreatedAt)
            .into_model::<BookmarkRow>()
            .all(db)
            .await?;

        // Türlere göre gruplanmış id'leri topla
        let mut product_ids: Vec<i64> = Vec::new();
        let mut page_ids: Vec<i64> = Vec::new();
        for r in rows.iter() {
            match r.content_type.as_str() {
                "product" => product_ids.push(r.content_id),
                "page" => page_ids.push(r.content_id),
                _ => {}
            }
        }
        product_ids.sort_unstable();
        product_ids.dedup();
        page_ids.sort_unstable();
        page_ids.dedup();

        // Ürünleri ve sayfaları toplu olarak getir
        let products_map = if !product_ids.is_empty() {
            match product_service::get_products_map_by_ids(db, lang, &product_ids, None).await {
                Ok(m) => m,
                Err(_) => std::collections::HashMap::new(),
            }
        } else {
            std::collections::HashMap::new()
        };

        let pages_map = if !page_ids.is_empty() {
            match page_service::get_pages_map_by_ids_including_unpublished(db, lang, &page_ids)
                .await
            {
                Ok(m) => m,
                Err(_) => std::collections::HashMap::new(),
            }
        } else {
            std::collections::HashMap::new()
        };

        // Yedekleme amacıyla ham içerikleri getir (yayınlanmamış/silinmiş dahil)
        let mut raw_contents_map: std::collections::HashMap<
            i64,
            crate::modules::content::models::ContentModel,
        > = std::collections::HashMap::new();
        let all_content_ids: Vec<i64> = rows.iter().map(|r| r.content_id).collect();
        if !all_content_ids.is_empty() {
            let contents = Content::find()
                .filter(ContentColumn::Id.is_in(all_content_ids))
                .all(db)
                .await?;
            for c in contents {
                raw_contents_map.insert(c.id, c);
            }
        }

        // Birleşik yanıtları oluştur
        let mut results: Vec<BookmarkResponse> = Vec::with_capacity(rows.len());
        for row in rows.into_iter() {
            let mut resp = BookmarkResponse {
                id: row.id,
                user_id: row.user_id,
                title: row.title,
                content_type: row.content_type.clone(),
                content_id: row.content_id,
                module_name: row.module_name,
                price: row.price,
                variant_key: row.variant_key,
                created_at: row.created_at,
                content_title: None,
                content_description: None,
                media: None,
                link: None,
                product: None,
                page: None,
            };

            match resp.content_type.as_str() {
                "product" => {
                    if let Some(prod) = products_map.get(&resp.content_id) {
                        resp.product = Some(prod.clone());
                        // Görüntüleme alanlarını üründen doldur
                        resp.content_title = Some(prod.title.clone());
                        resp.content_description = prod.description.clone();
                        resp.link = prod.get_absolute_url.clone().or_else(|| {
                            Some(format!("/{}/product/{}-{}", lang, prod.slug, prod.id))
                        });
                        // Öncelikle product.data.langs.$lang.media içindeki yerelleştirilmiş medyayı kullan
                        if let Some(media_val) = prod
                            .data
                            .get("langs")
                            .and_then(|langs| langs.get(lang))
                            .and_then(|ld| ld.get("media"))
                        {
                            resp.media = Some(media_val.clone());
                        } else if let Some(prod_value) = prod.product.as_ref() {
                            if let Some(media_val) = prod_value.get("media") {
                                resp.media = Some(media_val.clone());
                            } else if let Some(raw) = raw_contents_map.get(&resp.content_id) {
                                // Yedek olarak ham içerik alanlarını kullan
                                if resp.content_title.is_none() {
                                    resp.content_title = raw
                                        .data
                                        .get("langs")
                                        .and_then(|langs| langs.get(lang))
                                        .and_then(|ld| ld.get("title"))
                                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                                        .or_else(|| {
                                            raw.data
                                                .get("title")
                                                .and_then(|v| v.as_str().map(|s| s.to_string()))
                                        });
                                }
                                if resp.content_description.is_none() {
                                    resp.content_description = raw
                                        .data
                                        .get("langs")
                                        .and_then(|langs| langs.get(lang))
                                        .and_then(|ld| ld.get("description"))
                                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                                        .or_else(|| {
                                            raw.data
                                                .get("description")
                                                .and_then(|v| v.as_str().map(|s| s.to_string()))
                                        });
                                }
                                if let Some(media_val) = raw
                                    .data
                                    .get("langs")
                                    .and_then(|langs| langs.get(lang))
                                    .and_then(|ld| ld.get("media"))
                                {
                                    resp.media = Some(media_val.clone());
                                } else if let Some(media_val) = raw.data.get("media") {
                                    resp.media = Some(media_val.clone());
                                }
                            }
                        } else if let Some(raw) = raw_contents_map.get(&resp.content_id) {
                            // Yedek olarak ham içerik alanlarını kullan
                            if resp.content_title.is_none() {
                                resp.content_title = raw
                                    .data
                                    .get("langs")
                                    .and_then(|langs| langs.get(lang))
                                    .and_then(|ld| ld.get("title"))
                                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    .or_else(|| {
                                        raw.data
                                            .get("title")
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    });
                            }
                            if resp.content_description.is_none() {
                                resp.content_description = raw
                                    .data
                                    .get("langs")
                                    .and_then(|langs| langs.get(lang))
                                    .and_then(|ld| ld.get("description"))
                                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    .or_else(|| {
                                        raw.data
                                            .get("description")
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    });
                            }
                            if let Some(media_val) = raw
                                .data
                                .get("langs")
                                .and_then(|langs| langs.get(lang))
                                .and_then(|ld| ld.get("media"))
                            {
                                resp.media = Some(media_val.clone());
                            } else if let Some(media_val) = raw.data.get("media") {
                                resp.media = Some(media_val.clone());
                            }
                        }

                        // Eğer bookmark belirli bir varyanta işaret ediyorsa, gösterilen fiyatı varyant fiyatına göre override et
                        if let Some(ref vk) = resp.variant_key {
                            // Clone the product response so we can mutate the displayed price fields safely
                            let mut prod_clone = prod.clone();

                            if let Some(prod_json) = prod_clone.product.as_ref() {
                                if let Some(variants) =
                                    prod_json.get("variants").and_then(|v| v.as_array())
                                {
                                    if let Some(variant) = variants.iter().find(|v| {
                                        v.get("option_values_display")
                                            .and_then(|x| x.as_str())
                                            .map(|s| s.trim() == vk.trim())
                                            .unwrap_or(false)
                                    }) {
                                        // Prefer display_price (converted), fall back to raw price (float/int)
                                        let variant_price = variant
                                            .get("display_price")
                                            .and_then(|p| p.as_f64())
                                            .or_else(|| {
                                                variant.get("price").and_then(|p| {
                                                    p.as_f64()
                                                        .or_else(|| p.as_i64().map(|i| i as f64))
                                                })
                                            });

                                        // Build a formatted price string (prefer variant-provided formatted string)
                                        let formatted = variant
                                            .get("display_price_formatted")
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                            .or(prod_clone.price_formatted.clone())
                                            .or_else(|| {
                                                variant_price.map(|n| {
                                                    crate::modules::utils::format_price::format_price(
                                                        n,
                                                        prod_clone
                                                            .display_currency
                                                            .as_deref()
                                                            .unwrap_or("TRY"),
                                                    )
                                                })
                                            });

                                        if let Some(vp) = variant_price {
                                            // Keep numeric display price on product clone
                                            prod_clone.display_price = Some(vp);
                                        }

                                        if let Some(ref fstr) = formatted {
                                            // Keep the bookmark's stored 'added' price (do not overwrite resp.price)
                                            // Set product display formatted price to current variant price for UI ('Şimdiki Fiyat')
                                            prod_clone.price_formatted = Some(fstr.clone());
                                        }

                                        // Also override old price fields if present
                                        prod_clone.display_old_price = variant
                                            .get("display_old_price")
                                            .and_then(|p| p.as_f64())
                                            .or_else(|| {
                                                variant.get("old_price").and_then(|p| p.as_f64())
                                            })
                                            .or(prod_clone.display_old_price);

                                        prod_clone.old_price_formatted = variant
                                            .get("display_old_price_formatted")
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                            .or(prod_clone.old_price_formatted.clone());

                                        // Replace the product info on response with the overridden clone
                                        resp.product = Some(prod_clone);
                                    }
                                }
                            }
                        }
                    }
                }
                "page" => {
                    if let Some(pg) = pages_map.get(&resp.content_id) {
                        resp.page = Some(pg.clone());
                        // Görüntüleme alanlarını sayfadan doldur
                        resp.content_title = Some(pg.title.clone());
                        resp.content_description = pg.description.clone();
                        resp.link = Some(format!("/{}/page/{}-{}", lang, pg.slug, pg.id));
                        // Öncelikle page.data.langs.$lang.media içindeki yerelleştirilmiş medyayı kullan
                        if let Some(media_val) = pg
                            .data
                            .get("langs")
                            .and_then(|langs| langs.get(lang))
                            .and_then(|ld| ld.get("media"))
                        {
                            resp.media = Some(media_val.clone());
                        } else if let Some(media_val) = pg.data.get("media") {
                            resp.media = Some(media_val.clone());
                        } else if let Some(raw) = raw_contents_map.get(&resp.content_id) {
                            // Yedek olarak ham içerik alanlarını kullan
                            if resp.content_title.is_none() {
                                resp.content_title = raw
                                    .data
                                    .get("langs")
                                    .and_then(|langs| langs.get(lang))
                                    .and_then(|ld| ld.get("title"))
                                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    .or_else(|| {
                                        raw.data
                                            .get("title")
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    });
                            }
                            if resp.content_description.is_none() {
                                resp.content_description = raw
                                    .data
                                    .get("langs")
                                    .and_then(|langs| langs.get(lang))
                                    .and_then(|ld| ld.get("description"))
                                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    .or_else(|| {
                                        raw.data
                                            .get("description")
                                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                                    });
                            }
                            if let Some(media_val) = raw
                                .data
                                .get("langs")
                                .and_then(|langs| langs.get(lang))
                                .and_then(|ld| ld.get("media"))
                            {
                                resp.media = Some(media_val.clone());
                            } else if let Some(media_val) = raw.data.get("media") {
                                resp.media = Some(media_val.clone());
                            }
                        }
                    }
                }
                _ => {
                    // Bilinmeyen içerik türleri için yedek: mevcutsa ham içerik satırlarını kullan
                    if let Some(raw) = raw_contents_map.get(&resp.content_id) {
                        if resp.content_title.is_none() {
                            resp.content_title = raw
                                .data
                                .get("langs")
                                .and_then(|langs| langs.get(lang))
                                .and_then(|ld| ld.get("title"))
                                .and_then(|v| v.as_str().map(|s| s.to_string()))
                                .or_else(|| {
                                    raw.data
                                        .get("title")
                                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                                });
                        }
                        if resp.content_description.is_none() {
                            resp.content_description = raw
                                .data
                                .get("langs")
                                .and_then(|langs| langs.get(lang))
                                .and_then(|ld| ld.get("description"))
                                .and_then(|v| v.as_str().map(|s| s.to_string()))
                                .or_else(|| {
                                    raw.data
                                        .get("description")
                                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                                });
                        }
                        if let Some(media_val) = raw
                            .data
                            .get("langs")
                            .and_then(|langs| langs.get(lang))
                            .and_then(|ld| ld.get("media"))
                        {
                            resp.media = Some(media_val.clone());
                        } else if let Some(media_val) = raw.data.get("media") {
                            resp.media = Some(media_val.clone());
                        }
                    }
                }
            }

            results.push(resp);
        }

        Ok(results)
    }

    /// Yeni bir yer imi oluştur
    /// Aynı kullanıcı, içerik ve varyant anahtarına sahip bir yer imi zaten varsa,
    /// yeni bir kayıt oluşturmak yerine varolan kaydı döndürür (çoğaltmayı önler).
    pub async fn create_bookmark(
        db: &DatabaseConnection,
        user_id: i64,
        title: String,
        content_type: String,
        content_id: i64,
        module_name: String,
        price: Option<String>,
        variant_key: Option<String>,
    ) -> Result<BookmarkModel> {
        // Çoğaltmayı önlemek için önce aynı user/content/variant_key ve price kombinasyonu var mı kontrol et
        let existing = if let Some(ref vk) = variant_key {
            // Varyant belirtilmişse variant_key ile eşleşen kayıtları sorgula ve price ile de eşleşiyorsa var olanı döndür
            let mut q = Bookmarks::find()
                .filter(bookmarks_entities::Column::UserId.eq(user_id))
                .filter(bookmarks_entities::Column::ContentType.eq(content_type.clone()))
                .filter(bookmarks_entities::Column::ContentId.eq(content_id))
                .filter(bookmarks_entities::Column::VariantKey.eq(vk.clone()));
            if let Some(ref p) = price {
                q = q.filter(bookmarks_entities::Column::Price.eq(p.clone()));
            } else {
                q = q.filter(bookmarks_entities::Column::Price.is_null());
            }
            q.one(db).await?
        } else {
            // Varyant yoksa variant_key IS NULL ile sorgula ve price ile de eşleşiyorsa var olanı döndür
            let mut q = Bookmarks::find()
                .filter(bookmarks_entities::Column::UserId.eq(user_id))
                .filter(bookmarks_entities::Column::ContentType.eq(content_type.clone()))
                .filter(bookmarks_entities::Column::ContentId.eq(content_id))
                .filter(bookmarks_entities::Column::VariantKey.is_null());
            if let Some(ref p) = price {
                q = q.filter(bookmarks_entities::Column::Price.eq(p.clone()));
            } else {
                q = q.filter(bookmarks_entities::Column::Price.is_null());
            }
            q.one(db).await?
        };

        // Eğer varsa, varolan kaydı döndür (user/content/variant_key/price hepsi eşleşiyorsa)
        if let Some(existing_model) = existing {
            return Ok(existing_model);
        }

        let new_bookmark = bookmarks_entities::ActiveModel {
            user_id: Set(user_id),
            title: Set(title),
            content_type: Set(content_type),
            content_id: Set(content_id),
            module_name: Set(module_name),
            price: Set(price),
            variant_key: Set(variant_key),
            ..Default::default()
        };

        let result = new_bookmark.insert(db).await?;
        Ok(result)
    }

    /// Yer imini sil
    pub async fn delete_bookmark(db: &DatabaseConnection, id: i64, user_id: i64) -> Result<()> {
        let result = Bookmarks::delete_many()
            .filter(bookmarks_entities::Column::Id.eq(id))
            .filter(bookmarks_entities::Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        if result.rows_affected == 0 {
            return Err(anyhow::anyhow!("Address not found"));
        }
        Ok(())
    }

    pub async fn bookmarks_product_count(db: &DatabaseConnection, user_id: i64) -> Result<u64> {
        let count = Bookmarks::find()
            .filter(bookmarks_entities::Column::UserId.eq(user_id))
            .count(db)
            .await?;
        Ok(count)
    }
}

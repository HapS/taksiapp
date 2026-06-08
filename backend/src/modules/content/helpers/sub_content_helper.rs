// Sub Content Helper - Alt içerikleri yönetir
use crate::modules::content::models::{content::Column as ContentColumn, Content};
use sea_orm::*;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Belirli bir içeriğin alt içeriklerini yükler
pub async fn load_sub_contents(
    db: &DatabaseConnection,
    parent_content_id: i64,
    lang: Option<&str>,
) -> Result<HashMap<String, JsonValue>, DbErr> {
    // Önce parent content'i al
    let parent_content = Content::find_by_id(parent_content_id)
        .filter(ContentColumn::DeletedAt.is_null())
        .one(db)
        .await?;

    let mut sub_contents = HashMap::new();

    if let Some(parent) = parent_content {
        // Parent'ın data'sından sub_contents'i al
        if let Some(sub_content_configs) = parent.data.get("sub_contents") {
            if let Some(configs) = sub_content_configs.as_array() {
                // Her sub content config için içeriği yükle
                for config in configs {
                    if let (Some(name), Some(content_id)) = (
                        config.get("name").and_then(|n| n.as_str()),
                        config.get("content_id").and_then(|id| id.as_i64()),
                    ) {
                        // Sub content'i veritabanından yükle
                        match Content::find_by_id(content_id)
                            .filter(ContentColumn::Publish.eq(true))
                            .filter(ContentColumn::DeletedAt.is_null())
                            .one(db)
                            .await
                        {
                            Ok(Some(sub_content)) => {
                                let mut sub_content_data = serde_json::Map::new();
                                
                                // Temel content alanları
                                sub_content_data.insert("id".to_string(), JsonValue::Number(sub_content.id.into()));
                                sub_content_data.insert("content_type".to_string(), JsonValue::String(sub_content.content_type.clone()));
                                sub_content_data.insert("publish".to_string(), JsonValue::Bool(sub_content.publish));
                                sub_content_data.insert("gcx".to_string(), JsonValue::Bool(sub_content.gcx));
                                
                                if let Some(parent_id) = sub_content.parent_id {
                                    sub_content_data.insert("parent_id".to_string(), JsonValue::Number(parent_id.into()));
                                }
                                
                                if let Some(order_id) = sub_content.order_id {
                                    sub_content_data.insert("order_id".to_string(), JsonValue::Number(order_id.into()));
                                }
                                
                                if let Some(user_id) = sub_content.user_id {
                                    sub_content_data.insert("user_id".to_string(), JsonValue::Number(user_id.into()));
                                }
                                
                                // Timestamp'ler
                                if let Some(created_at) = sub_content.created_at {
                                    sub_content_data.insert("created_at".to_string(), JsonValue::String(created_at.to_string()));
                                }
                                if let Some(updated_at) = sub_content.updated_at {
                                    sub_content_data.insert("updated_at".to_string(), JsonValue::String(updated_at.to_string()));
                                }
                                
                                // Tüm JSON data'sını ekle - template'de current_language ile erişilecek
                                let mut data = sub_content.data.clone();
                                
                                // Apply media fallbacks if language is provided
                                if let Some(l) = lang {
                                    crate::modules::media::helpers::media_helper::resolve_media_fallbacks(&mut data, l);
                                }
                                
                                sub_content_data.insert("data".to_string(), data);
                                
                                // Tag'ları yükle
                                let tags = crate::modules::content::helpers::page_helper::fetch_tags_for_contents(
                                    db, 
                                    &[sub_content.id], 
                                    "tr" // Default language, template'de current_language ile override edilecek
                                ).await.unwrap_or_default();
                                
                                if let Some(content_tags) = tags.get(&sub_content.id) {
                                    sub_content_data.insert("tags".to_string(), serde_json::to_value(content_tags).unwrap_or(serde_json::Value::Array(vec![])));
                                } else {
                                    sub_content_data.insert("tags".to_string(), serde_json::Value::Array(vec![]));
                                }
                                
                                // Config'den gelen description'ı ekle
                                if let Some(description) = config.get("description").and_then(|d| d.as_str()) {
                                    if !description.is_empty() {
                                        sub_content_data.insert("sub_content_description".to_string(), JsonValue::String(description.to_string()));
                                    }
                                }

                                sub_contents.insert(name.to_string(), JsonValue::Object(sub_content_data));
                            }
                            Ok(None) => {
                                // İçerik bulunamadı veya yayında değil - sessizce atla
                            }
                            Err(_) => {
                                // Veritabanı hatası - sessizce atla
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(sub_contents)
}

/// Belirli bir content type'ın sub content'lerini yükler
#[allow(dead_code)] // Gelecekte kullanılabilir
pub async fn load_sub_contents_by_type(
    db: &DatabaseConnection,
    parent_content_id: i64,
    content_type: &str,
    lang: Option<&str>,
) -> Result<Vec<JsonValue>, DbErr> {
    let sub_contents_map = load_sub_contents(db, parent_content_id, lang).await?;
    
    let mut result = Vec::new();
    
    for (_name, sub_content) in sub_contents_map {
        if let Some(sub_content_type) = sub_content.get("content_type").and_then(|ct| ct.as_str()) {
            if sub_content_type == content_type {
                result.push(sub_content);
            }
        }
    }
    
    Ok(result)
}
// Global Context Helper - Global context içeriklerini yönetir
use crate::modules::content::models::{content::Column as ContentColumn, Content};
use crate::modules::taxonomy::models::{vocabulary::Column as VocabularyColumn, Term, Vocabulary};
use sea_orm::*;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::collections::HashSet;
/// Global context içeriklerini yükler
pub async fn load_global_context(
    db: &DatabaseConnection,
) -> Result<BTreeMap<String, JsonValue>, DbErr> {
    // 1. Content'leri yükle (gcx=true)
    let contents = Content::find()
        .filter(ContentColumn::Gcx.eq(true))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null())
        .order_by_asc(ContentColumn::OrderId)
        .all(db)
        .await?;

    // 1b. Subpages için gcx=false olan alt içerikleri de yükle
    // parent_id'si yukarıdaki content'lerin id'sine eşit olan TÜM content'leri al
    let gcx_content_ids: Vec<i64> = contents.iter().map(|c| c.id).collect();

    let subpages = Content::find()
        .filter(ContentColumn::ParentId.is_in(gcx_content_ids.clone()))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null())
        .order_by_asc(ContentColumn::OrderId)
        .all(db)
        .await?;

    // println!("{:?}", subpages);

    // Tüm içerikleri birleştir
    let mut all_contents = contents;
    all_contents.extend(subpages);

    let mut contents_map: BTreeMap<String, JsonValue> = BTreeMap::new();

    for content in all_contents {
        // Content'i ID ile contents altına ekle
        let key = content.id.to_string();

        let mut context_data = serde_json::Map::new();

        // Temel content alanları
        context_data.insert("id".to_string(), JsonValue::Number(content.id.into()));
        context_data.insert(
            "content_type".to_string(),
            JsonValue::String(content.content_type.clone()),
        );
        context_data.insert("publish".to_string(), JsonValue::Bool(content.publish));
        context_data.insert("gcx".to_string(), JsonValue::Bool(content.gcx));

        if let Some(parent_id) = content.parent_id {
            context_data.insert("parent_id".to_string(), JsonValue::Number(parent_id.into()));
        }

        if let Some(order_id) = content.order_id {
            context_data.insert("order_id".to_string(), JsonValue::Number(order_id.into()));
        }

        if let Some(user_id) = content.user_id {
            context_data.insert("user_id".to_string(), JsonValue::Number(user_id.into()));
        }

        // Timestamp'ler
        if let Some(created_at) = content.created_at {
            context_data.insert(
                "created_at".to_string(),
                JsonValue::String(created_at.to_string()),
            );
        }
        if let Some(updated_at) = content.updated_at {
            context_data.insert(
                "updated_at".to_string(),
                JsonValue::String(updated_at.to_string()),
            );
        }

        // JSON data'sını ekle
        context_data.insert("data".to_string(), content.data.clone());

        // Tag'ları yükle
        let tags = crate::modules::content::helpers::page_helper::fetch_tags_for_contents(
            db,
            &[content.id],
            "tr", // Default language, template'de current_language ile override edilecek
        )
        .await
        .unwrap_or_default();

        if let Some(content_tags) = tags.get(&content.id) {
            context_data.insert(
                "tags".to_string(),
                serde_json::to_value(content_tags).unwrap_or(serde_json::Value::Array(vec![])),
            );
        } else {
            context_data.insert("tags".to_string(), serde_json::Value::Array(vec![]));
        }

        // Sub contents'i yükle ve direkt content altına ekle
        if let Some(sub_contents_array) = content
            .data
            .get("sub_contents")
            .and_then(|sc| sc.as_array())
        {
            let mut expanded_sub_contents = serde_json::Map::new();

            for sub_content_config in sub_contents_array {
                if let (Some(name), Some(content_id)) = (
                    sub_content_config.get("name").and_then(|n| n.as_str()),
                    sub_content_config
                        .get("content_id")
                        .and_then(|id| id.as_i64()),
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

                            // Temel sub content alanları
                            sub_content_data
                                .insert("id".to_string(), JsonValue::Number(sub_content.id.into()));
                            sub_content_data.insert(
                                "content_type".to_string(),
                                JsonValue::String(sub_content.content_type.clone()),
                            );
                            sub_content_data.insert(
                                "publish".to_string(),
                                JsonValue::Bool(sub_content.publish),
                            );
                            sub_content_data
                                .insert("gcx".to_string(), JsonValue::Bool(sub_content.gcx));

                            if let Some(parent_id) = sub_content.parent_id {
                                sub_content_data.insert(
                                    "parent_id".to_string(),
                                    JsonValue::Number(parent_id.into()),
                                );
                            }

                            if let Some(order_id) = sub_content.order_id {
                                sub_content_data.insert(
                                    "order_id".to_string(),
                                    JsonValue::Number(order_id.into()),
                                );
                            }

                            if let Some(user_id) = sub_content.user_id {
                                sub_content_data.insert(
                                    "user_id".to_string(),
                                    JsonValue::Number(user_id.into()),
                                );
                            }

                            // Timestamp'ler
                            if let Some(created_at) = sub_content.created_at {
                                sub_content_data.insert(
                                    "created_at".to_string(),
                                    JsonValue::String(created_at.to_string()),
                                );
                            }
                            if let Some(updated_at) = sub_content.updated_at {
                                sub_content_data.insert(
                                    "updated_at".to_string(),
                                    JsonValue::String(updated_at.to_string()),
                                );
                            }

                            // Sub content'in tüm JSON data'sını ekle
                            sub_content_data.insert("data".to_string(), sub_content.data.clone());

                            // Sub content için tag'ları yükle
                            let sub_tags = crate::modules::content::helpers::page_helper::fetch_tags_for_contents(
                                db,
                                &[sub_content.id],
                                "tr" // Default language, template'de current_language ile override edilecek
                            ).await.unwrap_or_default();

                            if let Some(sub_content_tags) = sub_tags.get(&sub_content.id) {
                                sub_content_data.insert(
                                    "tags".to_string(),
                                    serde_json::to_value(sub_content_tags)
                                        .unwrap_or(serde_json::Value::Array(vec![])),
                                );
                            } else {
                                sub_content_data
                                    .insert("tags".to_string(), serde_json::Value::Array(vec![]));
                            }

                            // Config'den gelen description'ı ekle
                            if let Some(description) = sub_content_config
                                .get("description")
                                .and_then(|d| d.as_str())
                            {
                                if !description.is_empty() {
                                    sub_content_data.insert(
                                        "sub_content_description".to_string(),
                                        JsonValue::String(description.to_string()),
                                    );
                                }
                            }

                            expanded_sub_contents
                                .insert(name.to_string(), JsonValue::Object(sub_content_data));
                        }
                        Ok(None) => {
                            // İçerik bulunamadı veya yayında değil - sessizce atla
                            tracing::warn!(
                                "Sub content not found or not published: content_id={}",
                                content_id
                            );
                        }
                        Err(e) => {
                            // Veritabanı hatası - sessizce atla
                            tracing::error!("Error loading sub content {}: {}", content_id, e);
                        }
                    }
                }
            }

            // Sub contents'i direkt content altına ekle
            if !expanded_sub_contents.is_empty() {
                context_data.insert(
                    "sub_contents".to_string(),
                    JsonValue::Object(expanded_sub_contents),
                );
            }
        }

        contents_map.insert(key, JsonValue::Object(context_data));
    }

    // Subpages ekle: parent_id'si bu content'in id'sine eşit olan page/news/blog'ları bul
    // let content_types_for_subpages = vec!["page", "news", "blog"];
    let content_types_for_subpages: HashSet<&str> = ["page", "news", "blog"].into_iter().collect();

    // parent_id'ye göre grupla
    let mut parent_to_children: std::collections::BTreeMap<i64, Vec<JsonValue>> = BTreeMap::new();

    for (_content_id, content_value) in &contents_map {
        if let Some(parent_id) = content_value.get("parent_id").and_then(|p| p.as_i64()) {
            parent_to_children
                .entry(parent_id)
                .or_default()
                .push(content_value.clone());
        }
    }

    // Her content için subpages ekle (order_id'ye göre sıralı)
    for (content_id, content_value) in &mut contents_map {
        let content_id_i64: i64 = content_id.parse().unwrap_or(0);
        if content_id_i64 == 0 {
            continue;
        }

        if let Some(obj) = content_value.as_object_mut() {
            if let Some(content_type) = obj.get("content_type").and_then(|v| v.as_str()) {
                if content_types_for_subpages.contains(content_type) {
                    let mut subpages = parent_to_children
                        .get(&content_id_i64)
                        .cloned()
                        .unwrap_or_default();
                    subpages.sort_by(|a, b| {
                        let o1 = a
                            .get("order_id")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(i64::MAX);
                        let o2 = b
                            .get("order_id")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(i64::MAX);
                        o1.cmp(&o2)
                    });
                    obj.insert("subpages".to_string(), JsonValue::Array(subpages));
                }
            }
        }
    }

    // Contents'i global altına ekle - bu satırları kaldır
    // if !contents_map.is_empty() {
    //     global_data.insert("contents".to_string(), JsonValue::Object(contents_map));
    // }

    // 2. Vocabulary'leri yükle (gcx=true)
    let vocabularies = Vocabulary::find()
        .filter(VocabularyColumn::Gcx.eq(true))
        .order_by_asc(VocabularyColumn::OrderId)
        .all(db)
        .await?;

    let mut vocabularies_map: BTreeMap<String, JsonValue> = BTreeMap::new();

    for vocabulary in vocabularies {
        // Vocabulary'i ID ile vocabularies altına ekle
        let key = vocabulary.id.to_string();

        let mut vocab_data = serde_json::Map::new();

        // Temel vocabulary alanları
        vocab_data.insert("id".to_string(), JsonValue::Number(vocabulary.id.into()));
        vocab_data.insert(
            "vocabulary_type".to_string(),
            JsonValue::String(vocabulary.vocabulary_type.clone()),
        );
        vocab_data.insert("gcx".to_string(), JsonValue::Bool(vocabulary.gcx));

        if let Some(order_id) = vocabulary.order_id {
            vocab_data.insert("order_id".to_string(), JsonValue::Number(order_id.into()));
        }

        // Timestamp'ler
        if let Some(created_at) = vocabulary.created_at {
            vocab_data.insert(
                "created_at".to_string(),
                JsonValue::String(created_at.to_string()),
            );
        }
        if let Some(updated_at) = vocabulary.updated_at {
            vocab_data.insert(
                "updated_at".to_string(),
                JsonValue::String(updated_at.to_string()),
            );
        }

        // Vocabulary data'sını ekle
        vocab_data.insert("data".to_string(), vocabulary.data.clone());

        // Bu vocabulary'nin term'lerini yükle
        let terms = Term::find()
            .filter(crate::modules::taxonomy::models::term::Column::VocabularyId.eq(vocabulary.id))
            .filter(crate::modules::taxonomy::models::term::Column::Publish.eq(true))
            .order_by_asc(crate::modules::taxonomy::models::term::Column::OrderId)
            .all(db)
            .await?;

        let mut terms_array = Vec::new();

        for term in &terms {
            let mut term_data = serde_json::Map::new();
            term_data.insert("id".to_string(), JsonValue::Number(term.id.into()));
            term_data.insert(
                "vocabulary_id".to_string(),
                JsonValue::Number(term.vocabulary_id.into()),
            );
            term_data.insert("publish".to_string(), JsonValue::Bool(term.publish));

            if let Some(parent_id) = term.parent_id {
                term_data.insert("parent_id".to_string(), JsonValue::Number(parent_id.into()));
            }

            if let Some(order_id) = term.order_id {
                term_data.insert("order_id".to_string(), JsonValue::Number(order_id.into()));
            }

            // Timestamp'ler
            if let Some(created_at) = term.created_at {
                term_data.insert(
                    "created_at".to_string(),
                    JsonValue::String(created_at.to_string()),
                );
            }
            if let Some(updated_at) = term.updated_at {
                term_data.insert(
                    "updated_at".to_string(),
                    JsonValue::String(updated_at.to_string()),
                );
            }

            // Term data'sını ekle
            term_data.insert("data".to_string(), term.data.clone());

            terms_array.push(JsonValue::Object(term_data));
        }

        // Terms'leri vocabulary'ye array olarak ekle (flat)
        vocab_data.insert("terms".to_string(), JsonValue::Array(terms_array));

        // Çok dilli hiyerarşik terms'leri de ekle (mega menü için)
        let config = crate::config::get_config();
        let supported_languages: Vec<String> = config.supported_languages.keys().cloned().collect();
        let multilingual_hierarchical_terms =
            crate::modules::utils::terms_utils::build_multilingual_term_hierarchy(
                &terms,
                &supported_languages,
                None,
            );
        vocab_data.insert(
            "hierarchical_terms".to_string(),
            serde_json::to_value(multilingual_hierarchical_terms)
                .unwrap_or(JsonValue::Array(vec![])),
        );

        vocabularies_map.insert(key, JsonValue::Object(vocab_data));
    }

    // Vocabularies'i global altına ekle - bu satırları kaldır
    // if !vocabularies_map.is_empty() {
    //     global_data.insert("vocabularies".to_string(), JsonValue::Object(vocabularies_map));
    // }

    // Global data'yı direkt döndür
    let mut global_context: BTreeMap<String, JsonValue> = BTreeMap::new();
    global_context.insert(
        "contents".to_string(),
        JsonValue::Object(serde_json::Map::from_iter(
            contents_map.into_iter().map(|(k, v)| (k, v)),
        )),
    );
    global_context.insert(
        "vocabularies".to_string(),
        JsonValue::Object(serde_json::Map::from_iter(
            vocabularies_map.into_iter().map(|(k, v)| (k, v)),
        )),
    );

    Ok(global_context)
}

/// Global context cache'ini yenile
pub async fn refresh_global_context_cache(
    db: &DatabaseConnection,
    cache: &std::sync::Arc<std::sync::RwLock<BTreeMap<String, JsonValue>>>,
) -> Result<(), DbErr> {
    let new_context = load_global_context(db).await?;

    if let Ok(mut cache_guard) = cache.write() {
        *cache_guard = new_context;
        tracing::debug!(
            "Global context cache refreshed with {} items",
            cache_guard.len()
        );
    }

    Ok(())
}

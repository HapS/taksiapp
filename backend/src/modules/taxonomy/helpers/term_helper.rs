// Term Helpers - JSON parsing and DTO transformations
use crate::modules::taxonomy::models::term::Model as TermModel;
use serde::{Deserialize, Serialize, de::Deserializer};
use serde_json::Value as JsonValue;

// Term Response DTO
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TermResponse {
    pub id: i64,
    pub vocabulary_id: i64,
    pub title: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub publish: bool,
    pub lock: bool,
    pub hide: bool,
    pub order_id: Option<i32>, //neden order_id de order değil??? order rezerve bir kelime o yüzden order_id yaptık order_morder da olaiblirdi
    pub data: serde_json::Value,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term_icon: Option<String>,
}

// Breadcrumb Item for hierarchical navigation
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BreadcrumbItem {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub has_children: bool,
    pub children_count: i64,
}

// Hierarchical List Response with meta info
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct TermListResponse {
    pub data: Vec<TermResponse>,
    pub meta: TermListMeta,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct TermListMeta {
    pub total: u64,
    pub page: u64,
    pub limit: u64,
    pub total_pages: u64,
    pub parent_id: Option<String>,
    pub breadcrumbs: Vec<BreadcrumbItem>,
}

// Create/Update Request DTOs
#[derive(Debug, Deserialize)]
pub struct CreateTermRequest {
    pub vocabulary_id: i64,
    pub data: serde_json::Map<String, JsonValue>,
    pub parent_id: Option<i64>,
    pub publish: bool,
    pub lock: Option<bool>,
    pub hide: Option<bool>,
}

// Helper to deserialize nested Option types (double option pattern)
fn double_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(de).map(Some)
}

#[derive(Debug, Deserialize)]
pub struct UpdateTermRequest {
    pub data: Option<serde_json::Map<String, JsonValue>>,
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<i64>>,
    pub publish: Option<bool>,
    pub lock: Option<bool>,
    pub hide: Option<bool>,
}

// Trait for term extensions
pub trait TermExtensions {
    fn get_title(&self, lang: &str) -> String;
    fn get_slug(&self, lang: &str) -> Option<String>;
    fn get_description(&self, lang: &str) -> Option<String>;
    fn get_term_icon(&self) -> Option<String>;
    fn to_response(&self, lang: &str) -> TermResponse;
    fn to_response_with_meta(
        &self,
        lang: &str,
        children_count: i64,
        parent_title: Option<String>,
    ) -> TermResponse;
}

impl TermExtensions for TermModel {
    fn get_title(&self, lang: &str) -> String {
        get_string_from_json(&self.data, "title", lang).unwrap_or_else(|| "Untitled".to_string())
    }

    fn get_slug(&self, lang: &str) -> Option<String> {
        self.generate_slug(self.get_title(lang).as_str())
    }

    fn get_description(&self, lang: &str) -> Option<String> {
        get_string_from_json(&self.data, "description", lang)
    }

    fn get_term_icon(&self) -> Option<String> {
        // term_icon alanını data'dan al
        // Format: data.term_icon
        if let Some(icon) = self.data.get("term_icon") {
            if let Some(icon_str) = icon.as_str() {
                if !icon_str.is_empty() {
                    return Some(icon_str.to_string());
                }
            }
        }
        None
    }

    fn to_response(&self, lang: &str) -> TermResponse {
        TermResponse {
            id: self.id,
            vocabulary_id: self.vocabulary_id,
            title: self.get_title(lang),
            slug: self.get_slug(lang),
            description: self.get_description(lang),
            parent_id: self.parent_id,
            publish: self.publish,
            lock: self.lock,
            hide: self.hide,
            order_id: self.order_id,
            data: self.data.clone(),
            created_at: self.created_at.map(|dt| dt.naive_utc().and_utc()),
            children_count: None,
            parent_title: None,
            term_icon: self.get_term_icon(),
        }
    }

    fn to_response_with_meta(
        &self,
        lang: &str,
        children_count: i64,
        parent_title: Option<String>,
    ) -> TermResponse {
        TermResponse {
            id: self.id,
            vocabulary_id: self.vocabulary_id,
            title: self.get_title(lang),
            slug: self.get_slug(lang),
            description: self.get_description(lang),
            parent_id: self.parent_id,
            publish: self.publish,
            lock: self.lock,
            hide: self.hide,
            order_id: self.order_id,
            data: self.data.clone(),
            created_at: self.created_at.map(|dt| dt.naive_utc().and_utc()),
            children_count: Some(children_count),
            parent_title,
            term_icon: self.get_term_icon(),
        }
    }
}

// Helper function to get string from JSON
pub fn get_string_from_json(data: &JsonValue, field: &str, lang: &str) -> Option<String> {
    // Try langs.lang.field structure: data["langs"][lang][field]
    if let Some(langs_obj) = data.get("langs") {
        if let Some(lang_obj) = langs_obj.get(lang) {
            if let Some(value) = lang_obj.get(field) {
                if let Some(s) = value.as_str() {
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }

    // Try lang.field structure: data[lang][field]
    if let Some(lang_obj) = data.get(lang) {
        if let Some(value) = lang_obj.get(field) {
            if let Some(s) = value.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }

    // Try field.lang structure: data[field][lang]
    if let Some(field_obj) = data.get(field) {
        if let Some(value) = field_obj.get(lang) {
            if let Some(s) = value.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }

    None
}

// Vocabulary Helpers - JSON parsing and DTO transformations
use crate::modules::taxonomy::models::vocabulary::Model as VocabularyModel;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VocabularyType {
    Tag,
    Category,
    ProductAttributes,
    Menu,
    Options,
}

impl VocabularyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            VocabularyType::Tag => "tag",
            VocabularyType::Category => "category",
            VocabularyType::ProductAttributes => "product_attributes",
            VocabularyType::Menu => "menu",
            VocabularyType::Options => "options",
        }
    }
}

impl ToString for VocabularyType {
    fn to_string(&self) -> String {
        self.as_str().to_string()
    }
}

// Vocabulary Response DTO
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VocabularyResponse {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub vocabulary_type: String,
    pub data: serde_json::Value,
    pub gcx: Option<bool>,
    pub lock: Option<bool>,
    pub hide: Option<bool>,
    pub order_id: Option<i32>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

// Create/Update Request DTOs
#[derive(Debug, Deserialize)]
pub struct CreateVocabularyRequest {
    pub data: serde_json::Map<String, JsonValue>,
    pub vocabulary_type: VocabularyType,
    pub gcx: Option<bool>,
    pub lock: Option<bool>,
    pub hide: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateVocabularyRequest {
    pub data: Option<serde_json::Map<String, JsonValue>>,
    pub vocabulary_type: Option<VocabularyType>,
    pub gcx: Option<bool>,
    pub lock: Option<bool>,
    pub hide: Option<bool>,
}

// Trait for vocabulary extensions
pub trait VocabularyExtensions {
    fn get_title(&self, lang: &str) -> String;
    fn get_description(&self, lang: &str) -> Option<String>;
    fn to_response(&self, lang: &str) -> VocabularyResponse;
}

impl VocabularyExtensions for VocabularyModel {
    fn get_title(&self, lang: &str) -> String {
        get_string_from_json(&self.data, "title", lang).unwrap_or_else(|| "Untitled".to_string())
    }

    fn get_description(&self, lang: &str) -> Option<String> {
        get_string_from_json(&self.data, "description", lang)
    }

    fn to_response(&self, lang: &str) -> VocabularyResponse {
        VocabularyResponse {
            id: self.id,
            title: self.get_title(lang),
            description: self.get_description(lang),
            vocabulary_type: self.vocabulary_type.clone(),
            gcx: Some(self.gcx),
            lock: Some(self.lock),
            hide: Some(self.hide),
            order_id: self.order_id,
            data: self.data.clone(),
            created_at: self.created_at.map(|dt| dt.naive_utc().and_utc()),
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

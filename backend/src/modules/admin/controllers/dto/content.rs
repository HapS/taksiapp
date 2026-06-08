// Shared Page Models and Helpers
// Used by both web and API controllers
use crate::modules::content::models::ContentModel;
use serde::{Deserialize, Serialize};

// ============ DATA MODELS ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangContent {
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub body: String,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentData {
    // Çoklu dil desteği - Go projesi formatı
    pub langs: std::collections::HashMap<String, LangContent>,

    // Dil bağımsız alanlar
    pub template: Option<String>,
    pub settings: Option<std::collections::HashMap<String, bool>>, // Yeni eklenen ayarlar
}

impl Default for ContentData {
    fn default() -> Self {
        let config = crate::config::get_config();
        let mut langs = std::collections::HashMap::new();

        // Desteklenen her dil için boş içerik
        for lang_code in config.supported_languages.keys() {
            langs.insert(
                lang_code.clone(),
                LangContent {
                    title: "".to_string(),
                    slug: "".to_string(),
                    description: None,
                    body: "".to_string(),
                    meta_title: None,
                    meta_description: None,
                },
            );
        }

        Self {
            langs,
            template: None,
            settings: None,
        }
    }
}

// Breadcrumb item for navigation
#[derive(Serialize, Clone)]
pub struct BreadcrumbItem {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub has_children: bool,
    pub children_count: i64,
}

// Admin API için raw JSON response
#[derive(Serialize)]
pub struct AdminContentResponse {
    pub id: i64,
    pub content_type: Option<String>,
    pub data: serde_json::Value, // Raw JSON - esnek yapı
    pub publish: bool,
    pub gcx: Option<bool>,
    pub parent_id: Option<i64>,
    pub term_ids: Vec<i64>,
    pub tag_ids: Vec<i64>,
    pub breadcrumbs: Vec<BreadcrumbItem>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
pub struct CreateContentRequest {
    pub content_type: Option<String>,
    pub publish: Option<bool>,
    pub gcx: Option<bool>,
    pub parent_id: Option<i64>,
    pub data: serde_json::Value, // Raw JSON - esnek yapı
    pub term_ids: Option<Vec<i64>>,
    pub tag_ids: Option<Vec<i64>>,
}

#[derive(Deserialize)]
pub struct UpdateContentRequest {
    pub content_type: Option<String>,
    pub publish: Option<bool>,
    pub gcx: Option<bool>,
    pub parent_id: Option<i64>,
    pub data: Option<serde_json::Value>, // Raw JSON - esnek yapı
    pub term_ids: Option<Vec<i64>>,
    pub tag_ids: Option<Vec<i64>>,
}

// Helper trait
pub trait ContentExtensions {
    fn as_content_data(&self) -> Result<ContentData, serde_json::Error>;
    fn get_admin_content_response(&self) -> AdminContentResponse;
}

impl ContentExtensions for ContentModel {
    fn as_content_data(&self) -> Result<ContentData, serde_json::Error> {
        serde_json::from_value(self.data.clone())
    }

    fn get_admin_content_response(&self) -> AdminContentResponse {
        AdminContentResponse {
            id: self.id,
            content_type: Some(self.content_type.clone()),
            data: self.data.clone(), // Raw JSON - olduğu gibi
            publish: self.publish,
            gcx: Some(self.gcx),
            parent_id: self.parent_id,
            term_ids: Vec::new(), // Boş array, gerçek değer API'de set edilecek
            tag_ids: Vec::new(),  // Boş array, gerçek değer API'de set edilecek
            breadcrumbs: Vec::new(), // Boş array, gerçek değer API'de set edilecek
            created_at: self.created_at.map(|dt| dt.naive_utc().and_utc()),
            updated_at: self.updated_at.map(|dt| dt.naive_utc().and_utc()),
        }
    }
}

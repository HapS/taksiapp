use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct FormResponse {
    pub id: i64,
    pub form_id: i64,
    pub data: serde_json::Value,
    pub ip: Option<String>,
    pub user_id: Option<i64>,
    pub created_at: Option<String>,
    pub content: Option<FormContentData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FormContentData {
    pub id: i64,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub content_type: String,
    pub publish: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FormListResponse {
    pub id: i64,
    pub form_id: i64,
    pub created_at: Option<String>,
    pub content: Option<FormContentData>,
}

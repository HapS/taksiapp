use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::scenario::ScenarioType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCampaignRequest {
    pub name: String,
    pub description: Option<String>,
    pub scenario_type: ScenarioType,
    pub params: serde_json::Value,
    #[serde(default = "default_campaign_type")]
    pub campaign_type: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub stackable: Option<bool>,
    pub max_uses: Option<i32>,
    pub max_uses_per_user: Option<i32>,
    #[serde(default = "default_target_cart_type")]
    pub target_cart_type: String,
}

use crate::modules::ecommerce::models::campaign;

fn default_campaign_type() -> String {
    campaign::campaign_type::AUTOMATIC.to_string()
}

fn default_target_cart_type() -> String {
    "both".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCampaignRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub params: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub priority: Option<i32>,
    pub stackable: Option<bool>,
    pub max_uses: Option<i32>,
    pub max_uses_per_user: Option<i32>,
    pub target_cart_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCouponRequest {
    pub codes: Vec<String>,
    pub user_id: Option<i64>,
    pub max_usage: Option<i32>,
    pub valid_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateCouponsRequest {
    pub prefix: Option<String>,
    pub length: Option<usize>,
    pub count: Option<usize>,
    pub user_id: Option<i64>,
    pub max_usage: Option<i32>,
    pub valid_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCouponRequest {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignListQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub is_active: Option<bool>,
    pub scenario_type: Option<String>,
    pub campaign_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignStatsResponse {
    pub usage_count: u64,
    pub total_discount_given: String,
    pub top_products: Vec<TopProductInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopProductInfo {
    pub product_id: i64,
    pub product_title: Option<String>,
    pub usage_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignTestRequest {
    pub cart_id: i64,
}
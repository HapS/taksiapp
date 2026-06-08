use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Settings - Site ayarları tablosu
/// Tek satır olacak, ID=1 sabit
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "settings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Tüm ayarlar JSONB formatında
    pub data: Option<Json>,

    /// Timestamps
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

/// Settings data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SettingsData {
    // Site Bilgileri (Çok Dilli)
    pub site_name_langs: Option<serde_json::Value>, // {"langs": {"tr": {"title": "..."}, "en": {"title": "..."}}}
    pub site_description_langs: Option<serde_json::Value>,
    pub site_keywords_langs: Option<serde_json::Value>,
    pub site_logo: Option<String>,
    pub site_logo_dark: Option<String>,
    pub site_favicon: Option<String>,

    // SEO Ayarları (Çok Dilli)
    pub seo_title_langs: Option<serde_json::Value>,
    pub seo_description_langs: Option<serde_json::Value>,
    pub seo_image_langs: Option<serde_json::Value>,

    // SMTP Mail Ayarları
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from_email: Option<String>,
    pub smtp_from_name: Option<String>,
    pub smtp_encryption: Option<String>, // "tls", "ssl", "none"

    // Sosyal Medya
    pub social_facebook: Option<String>,
    pub social_twitter: Option<String>,
    pub social_instagram: Option<String>,
    pub social_linkedin: Option<String>,
    pub social_youtube: Option<String>,

    // İletişim Bilgileri
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_address: Option<String>,
    pub contact_map_embed: Option<String>,

    // Banka Bilgileri
    pub bank1_name: Option<String>,
    pub bank1_account_holder: Option<String>,
    pub bank1_iban: Option<String>,
    pub bank1_branch_code: Option<String>,
    pub bank2_name: Option<String>,
    pub bank2_account_holder: Option<String>,
    pub bank2_iban: Option<String>,
    pub bank2_branch_code: Option<String>,

    // Diğer Ayarlar
    pub maintenance_mode: Option<bool>,
    pub analytics_code: Option<String>,
    pub custom_css: Option<String>,
    pub custom_js: Option<String>,

    // Vocabulary Ayarları
    pub vocab_navbar_menu: Option<i64>,
    pub vocab_footer_menu: Option<i64>,
    pub vocab_product_categories: Option<i64>,

    // Payment Provider Ayarları
    pub default_payment_provider: Option<String>, // "iyzico", "garanti", etc.
    pub payment_providers: Option<serde_json::Value>, // JSON object with provider configs
    pub payment_methods: Option<serde_json::Value>,
    pub vocab_blog_categories: Option<i64>,
    pub vocab_news_categories: Option<i64>,
    pub vocab_page_categories: Option<i64>,
    pub vocab_tags_categories: Option<i64>,
    pub vocab_payment_methods: Option<i64>,
    pub default_contact_form: Option<i64>,

    // Frontend Theme Ayarı
    pub frontend_theme: Option<String>, // "base", "supertema", etc.
    pub debug_logs: Option<bool>,

    // Default Content Ayarları
    pub default_home_content_id: Option<i64>, // Ana sayfa için varsayılan content ID

    // Robots.txt
    pub robots: Option<String>,

    // Bildirim Ayarları (Mail & Telefon)
    pub admin_notification_mail: Option<String>,
    pub admin_notification_phone: Option<String>,
    pub accounting_notification_mail: Option<String>,
    pub accounting_notification_phone: Option<String>,
    pub warehouse_notification_mail: Option<String>,
    pub warehouse_notification_phone: Option<String>,
    pub purchasing_notification_mail: Option<String>,
    pub purchasing_notification_phone: Option<String>,
    pub return_notification_mail: Option<String>,
    pub return_notification_phone: Option<String>,
    pub service_notification_mail: Option<String>,
    pub service_notification_phone: Option<String>,
}

impl Default for SettingsData {
    fn default() -> Self {
        use serde_json::json;

        Self {
            site_name_langs: Some(json!({
                "langs": {
                    "tr": {"title": "Backend-RS"},
                    "en": {"title": "Backend-RS"}
                }
            })),
            site_description_langs: Some(json!({
                "langs": {
                    "tr": {"title": "Modern Rust Web Uygulaması"},
                    "en": {"title": "Modern Rust Web Application"}
                }
            })),
            site_keywords_langs: None,
            site_logo: None,
            site_logo_dark: None,
            site_favicon: None,
            seo_title_langs: None,
            seo_description_langs: None,
            seo_image_langs: None,
            smtp_host: None,
            smtp_port: Some(587),
            smtp_username: None,
            smtp_password: None,
            smtp_from_email: None,
            smtp_from_name: None,
            smtp_encryption: Some("tls".to_string()),
            social_facebook: None,
            social_twitter: None,
            social_instagram: None,
            social_linkedin: None,
            social_youtube: None,
            contact_email: None,
            contact_phone: None,
            contact_address: None,
            contact_map_embed: None,
            bank1_name: None,
            bank1_account_holder: None,
            bank1_iban: None,
            bank1_branch_code: None,
            bank2_name: None,
            bank2_account_holder: None,
            bank2_iban: None,
            bank2_branch_code: None,
            maintenance_mode: Some(false),
            analytics_code: None,
            custom_css: None,
            custom_js: None,
            vocab_navbar_menu: Some(1),
            vocab_footer_menu: Some(2),
            vocab_product_categories: Some(3),
            default_contact_form: Some(90),
            default_payment_provider: Some("iyzico".to_string()),
            payment_providers: Some(json!({
                "iyzico": {
                    "provider_type": "iyzico",
                    "enabled": false,
                    "test_mode": true,
                    "config": {
                        "api_key": "",
                        "secret_key": "",
                        "base_url": "https://sandbox-api.iyzipay.com"
                    }
                },
                "garanti": {
                    "provider_type": "garanti",
                    "enabled": false,
                    "test_mode": true,
                    "config": {
                        "terminal_id": "30691297",
                        "merchant_id": "7000679",
                        "user_id": "PROVAUT",
                        "password": "123qweASD/",
                        "store_key": "12345678",
                        "base_url": "https://sanalposprovtest.garantibbva.com.tr"
                    }
                }
            })),
            payment_methods: Some(json!({
                "credit_card": {
                    "enabled": true,
                    "icon": "credit-card-2-front",
                    "order_id": 1,
                    "b2b_available": true,
                    "b2c_available": true,
                    "langs": {
                        "tr": {"title": "Kredi Kartı", "description": "Kredi kartınızla güvenli ödeme yapın"},
                        "en": {"title": "Credit Card", "description": "Pay securely with your credit card"}
                    }
                },
                "bank_transfer": {
                    "enabled": true,
                    "icon": "bank",
                    "order_id": 2,
                    "b2b_available": true,
                    "b2c_available": true,
                    "langs": {
                        "tr": {"title": "Banka Transferi", "description": "Banka hesabımıza transfer yaparak ödeme yapın"},
                        "en": {"title": "Bank Transfer", "description": "Pay by transferring to our bank account"}
                    }
                },
                "cash_on_delivery": {
                    "enabled": false,
                    "icon": "cash-coin",
                    "order_id": 3,
                    "b2b_available": false,
                    "b2c_available": true,
                    "langs": {
                        "tr": {"title": "Kapıda Ödeme", "description": "Sipariş teslimatında nakit ödeme yapın"},
                        "en": {"title": "Cash on Delivery", "description": "Pay with cash upon delivery"}
                    }
                },
                "b2b_credit": {
                    "enabled": true,
                    "icon": "person-badge",
                    "order_id": 4,
                    "b2b_available": true,
                    "b2c_available": false,
                    "langs": {
                        "tr": {"title": "B2B Kredisi", "description": "B2B kredisi ile ödeme yapın"},
                        "en": {"title": "B2B Credit", "description": "Pay with B2B credit"}
                    }
                }
            })),
            vocab_blog_categories: Some(4),
            vocab_news_categories: Some(5),
            vocab_page_categories: Some(6),
            vocab_tags_categories: Some(7),
            vocab_payment_methods: Some(12),
            frontend_theme: Some("base".to_string()),
            debug_logs: Some(false),
            default_home_content_id: Some(70), // Varsayılan olarak 70
            robots: Some("User-agent: *\nAllow: /".to_string()),
            // Bildirim Ayarları Varsayılan Değerler
            admin_notification_mail: None,
            admin_notification_phone: None,
            accounting_notification_mail: None,
            accounting_notification_phone: None,
            warehouse_notification_mail: None,
            warehouse_notification_phone: None,
            purchasing_notification_mail: None,
            purchasing_notification_phone: None,
            return_notification_mail: None,
            return_notification_phone: None,
            service_notification_mail: None,
            service_notification_phone: None,
        }
    }
}

impl SettingsData {
    /// Çok dilli alanlardan belirli bir dildeki değeri al
    pub fn get_lang_value(&self, field: &str, lang: &str) -> Option<String> {
        let json_value = match field {
            "site_name" => &self.site_name_langs,
            "site_description" => &self.site_description_langs,
            "site_keywords" => &self.site_keywords_langs,
            "seo_title" => &self.seo_title_langs,
            "seo_description" => &self.seo_description_langs,
            "seo_image" => &self.seo_image_langs,
            _ => return None,
        };

        if let Some(json) = json_value {
            json.get("langs")
                .and_then(|langs| langs.get(lang))
                .and_then(|lang_data| lang_data.get("title"))
                .and_then(|title| title.as_str())
                .map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Çok dilli alan için değer set et
    pub fn set_lang_value(&mut self, field: &str, lang: &str, value: &str) {
        use serde_json::json;

        let json_field = match field {
            "site_name" => &mut self.site_name_langs,
            "site_description" => &mut self.site_description_langs,
            "site_keywords" => &mut self.site_keywords_langs,
            "seo_title" => &mut self.seo_title_langs,
            "seo_description" => &mut self.seo_description_langs,
            "seo_image" => &mut self.seo_image_langs,
            _ => return,
        };

        // Mevcut JSON'u al veya yeni oluştur
        let mut current_json = json_field.clone().unwrap_or_else(|| json!({"langs": {}}));

        // Dil değerini set et
        if let Some(langs) = current_json.get_mut("langs") {
            if let Some(lang_obj) = langs.get_mut(lang) {
                lang_obj["title"] = json!(value);
            } else {
                langs[lang] = json!({"title": value});
            }
        }

        *json_field = Some(current_json);
    }

    /// Vocabulary ID'sini al
    pub fn get_vocab_id(&self, vocab_type: &str) -> Option<i64> {
        match vocab_type {
            "navbar_menu" => self.vocab_navbar_menu,
            "footer_menu" => self.vocab_footer_menu,
            "product_categories" => self.vocab_product_categories,
            "blog_categories" => self.vocab_blog_categories,
            "news_categories" => self.vocab_news_categories,
            "page_categories" => self.vocab_page_categories,
            "tags_categories" => self.vocab_tags_categories,
            "vocab_payment_methods" => self.vocab_payment_methods,
            _ => None,
        }
    }
}

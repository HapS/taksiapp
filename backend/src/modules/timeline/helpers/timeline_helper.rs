use crate::modules::timeline::models::timeline_event::TimelineEventType;
use crate::modules::timeline::services::{CreateTimelineEventRequest, TimelineService};
use sea_orm::DatabaseConnection;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub struct TimelineHelper;

impl TimelineHelper {
    /// JSON'dan belirli dil ve alan için string çıkar
    /// Desteklenen format: data.langs.tr.field
    
    pub fn get_string_from_json(data: &JsonValue, lang: &str, field: &str) -> Option<String> {
        if let Some(langs) = data.get("langs") {
            if let Some(lang_data) = langs.get(lang) {
                if let Some(value) = lang_data.get(field) {
                    return value.as_str().map(|s| s.to_string());
                }
            }
        }
        None
    }

    /// JSON'dan mevcut dilleri çıkar //kullanılmıyor gerçekten
    #[allow(dead_code)]
    pub fn get_available_languages(data: &JsonValue) -> Vec<String> {
        if let Some(langs) = data.get("langs") {
            if let Some(obj) = langs.as_object() {
                return obj.keys().cloned().collect();
            }
        }
        vec![]
    }

    /// Belirli dilde içerik var mı kontrol et
    #[allow(dead_code)]
    pub fn has_content_in_language(data: &JsonValue, language: &str) -> bool {
        let title =
            Self::get_string_from_json(data, language, "title").unwrap_or_else(|| "".to_string());
        !title.is_empty()
    }

    /// Mevcut dil için title çıkar, yoksa fallback dil kullan
    #[allow(dead_code)]
    pub fn get_title_for_language(data: &JsonValue, language: &str, fallback: &str) -> String {
        Self::get_string_from_json(data, language, "title")
            .or_else(|| Self::get_string_from_json(data, fallback, "title"))
            .unwrap_or_else(|| "Başlıksız".to_string())
    }

    /// Mevcut dil için description çıkar, yoksa fallback dil kullan
    #[allow(dead_code)]
    pub fn get_description_for_language(
        data: &JsonValue,
        language: &str,
        fallback: &str,
    ) -> Option<String> {
        Self::get_string_from_json(data, language, "description")
            .or_else(|| Self::get_string_from_json(data, fallback, "description"))
    }

    /// Genel content eventi oluştur - content_type'a göre dinamik
    pub async fn create_content_event(
        db: &DatabaseConnection,
        content_id: i64,
        content_type: &str, // "product", "blog", "news", "page"
        action: &str,       // "created", "updated", "published", "unpublished"
        admin_user_id: Option<i64>,
        metadata: Option<JsonValue>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Content type'a göre Türkçe isimler
        let content_name_tr = match content_type {
            "product" => "Ürün",
            "blog" => "Blog yazısı",
            "news" => "Haber",
            "page" => "Sayfa",
            _ => "İçerik",
        };

        let content_name_en = match content_type {
            "product" => "Product",
            "blog" => "Blog post",
            "news" => "News",
            "page" => "Page",
            _ => "Content",
        };

        // Action'a göre mesajlar
        let (action_tr, action_en) = match action {
            "created" => ("oluşturuldu", "created"),
            "updated" => ("güncellendi", "updated"),
            "published" => ("yayınlandı", "published"),
            "unpublished" => ("yayından kaldırıldı", "unpublished"),
            "deleted" => ("silindi", "deleted"),
            _ => ("işlendi", "processed"),
        };

        let mut title = HashMap::new();
        title.insert(
            "tr".to_string(),
            format!("{} {}", content_name_tr, action_tr),
        );
        title.insert(
            "en".to_string(),
            format!("{} {}", content_name_en, action_en),
        );

        // Event type'ı belirle
        let event_type = match (content_type, action) {
            ("product", "created") => TimelineEventType::ProductCreated,
            ("product", "updated") => TimelineEventType::ProductUpdated,
            ("product", "published") => TimelineEventType::ProductPublished,
            ("product", "unpublished") => TimelineEventType::ProductUnpublished,
            ("product", "deleted") => TimelineEventType::ProductDeleted,
            _ => TimelineEventType::Custom(format!("{}_{}", content_type, action)),
        };

        // Public/admin visibility - product'lar genelde admin-only, diğerleri public olabilir
        let (is_public, is_admin_only) = match content_type {
            "product" => (false, true),
            "blog" | "news" => (true, false),
            "page" => (false, true),
            _ => (false, true),
        };

        let request = CreateTimelineEventRequest {
            module_type: "content".to_string(),
            content_type: content_type.to_string(),
            content_id,
            event_type,
            title,
            description: None,
            icon: None,
            color: None,
            user_id: None,
            admin_user_id,
            metadata,
            is_public: Some(is_public),
            is_admin_only: Some(is_admin_only),
        };

        TimelineService::create_event(db, request).await?;
        Ok(())
    }

    /// Kullanıcı eventi oluştur
    #[allow(dead_code)]
    pub async fn create_user_event(
        db: &DatabaseConnection,
        user_id: i64,
        event_type: TimelineEventType,
        metadata: Option<JsonValue>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (title_tr, title_en) = match event_type {
            TimelineEventType::UserRegistered => ("Hesap oluşturuldu", "Account created"),
            TimelineEventType::UserLogin => ("Giriş yapıldı", "Logged in"),
            TimelineEventType::UserProfileUpdated => ("Profil güncellendi", "Profile updated"),
            TimelineEventType::PasswordChanged => ("Şifre değiştirildi", "Password changed"),
            _ => return Err("Geçersiz kullanıcı event tipi".into()),
        };

        let mut title = HashMap::new();
        title.insert("tr".to_string(), title_tr.to_string());
        title.insert("en".to_string(), title_en.to_string());

        let request = CreateTimelineEventRequest {
            module_type: "auth".to_string(),
            content_type: "user".to_string(),
            content_id: user_id,
            event_type,
            title,
            description: None,
            icon: None,
            color: None,
            user_id: Some(user_id),
            admin_user_id: None,
            metadata,
            is_public: Some(true),
            is_admin_only: Some(false),
        };

        TimelineService::create_event(db, request).await?;
        Ok(())
    }

    /// Özel event oluştur - dinamik dil desteği ile
    #[allow(dead_code)]
    pub async fn create_custom_event(
        db: &DatabaseConnection,
        module_type: &str,
        content_type: &str,
        content_id: i64,
        event_name: &str,
        title: HashMap<String, String>, // Dinamik dil desteği
        description: Option<HashMap<String, String>>, // Dinamik dil desteği
        icon: Option<String>,
        color: Option<String>,
        user_id: Option<i64>,
        admin_user_id: Option<i64>,
        metadata: Option<JsonValue>,
        is_public: bool,
        is_admin_only: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = CreateTimelineEventRequest {
            module_type: module_type.to_string(),
            content_type: content_type.to_string(),
            content_id,
            event_type: TimelineEventType::Custom(event_name.to_string()),
            title,
            description,
            icon,
            color,
            user_id,
            admin_user_id,
            metadata,
            is_public: Some(is_public),
            is_admin_only: Some(is_admin_only),
        };

        TimelineService::create_event(db, request).await?;
        Ok(())
    }
}

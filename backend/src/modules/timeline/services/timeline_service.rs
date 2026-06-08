use crate::modules::timeline::models::timeline_event::{
    ActiveModel as TimelineEventActiveModel, Entity as TimelineEvent, Model as TimelineEventModel,
    TimelineEventType,
};
use sea_orm::{entity::prelude::*, QueryOrder, QuerySelect, Set};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;

pub struct TimelineService;

#[derive(Debug, Clone)]
pub struct CreateTimelineEventRequest {
    pub module_type: String,
    pub content_type: String,
    pub content_id: i64,
    pub event_type: TimelineEventType,
    pub title: HashMap<String, String>, // {"tr": "Sipariş oluşturuldu", "en": "Order created"}
    pub description: Option<HashMap<String, String>>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub user_id: Option<i64>,
    pub admin_user_id: Option<i64>,
    pub metadata: Option<JsonValue>,
    pub is_public: Option<bool>,
    pub is_admin_only: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct TimelineEventFilter {
    pub module_type: Option<String>,
    pub content_type: Option<String>,
    pub content_id: Option<i64>,
    pub user_id: Option<i64>,
    pub is_public: Option<bool>,
    pub is_admin_only: Option<bool>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

impl TimelineService {
    /// Yeni timeline event oluştur
    pub async fn create_event(
        db: &DatabaseConnection,
        request: CreateTimelineEventRequest,
    ) -> Result<TimelineEventModel, DbErr> {
        let now = chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(0).unwrap());

        // Title'ı langs formatına çevir
        let title_langs = json!({
            "langs": request.title.into_iter().map(|(lang, title)| {
                (lang, json!({"title": title}))
            }).collect::<serde_json::Map<String, JsonValue>>()
        });

        // Description'ı langs formatına çevir (eğer varsa)
        let description_langs = request.description.map(|desc| {
            json!({
                "langs": desc.into_iter().map(|(lang, description)| {
                    (lang, json!({"description": description}))
                }).collect::<serde_json::Map<String, JsonValue>>()
            })
        });

        let timeline_event = TimelineEventActiveModel {
            module_type: Set(request.module_type),
            content_type: Set(request.content_type),
            content_id: Set(request.content_id),
            event_type: Set(request.event_type.as_str().to_string()),
            title: Set(title_langs),
            description: Set(description_langs),
            icon: Set(request
                .icon
                .or_else(|| Some(request.event_type.default_icon().to_string()))),
            color: Set(request
                .color
                .or_else(|| Some(request.event_type.default_color().to_string()))),
            user_id: Set(request.user_id),
            admin_user_id: Set(request.admin_user_id),
            metadata: Set(request.metadata),
            is_public: Set(request.is_public.unwrap_or(true)),
            is_admin_only: Set(request.is_admin_only.unwrap_or(false)),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        };

        timeline_event.insert(db).await
    }

    /// Timeline eventlerini filtrele ve getir
    pub async fn get_events(
        db: &DatabaseConnection,
        filter: TimelineEventFilter,
    ) -> Result<Vec<TimelineEventModel>, DbErr> {
        let mut query = TimelineEvent::find();

        // Filtreleri uygula
        if let Some(module_type) = filter.module_type {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::ModuleType
                    .eq(module_type),
            );
        }

        if let Some(content_type) = filter.content_type {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::ContentType
                    .eq(content_type),
            );
        }

        if let Some(content_id) = filter.content_id {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::ContentId.eq(content_id),
            );
        }

        if let Some(user_id) = filter.user_id {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::UserId.eq(user_id),
            );
        }

        if let Some(is_public) = filter.is_public {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::IsPublic.eq(is_public),
            );
        }

        if let Some(is_admin_only) = filter.is_admin_only {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::IsAdminOnly
                    .eq(is_admin_only),
            );
        }

        // Sıralama ve sayfalama
        query = query
            .order_by_desc(crate::modules::timeline::models::timeline_event::Column::CreatedAt);

        if let Some(limit) = filter.limit {
            query = query.limit(limit);
        }

        if let Some(offset) = filter.offset {
            query = query.offset(offset);
        }

        query.all(db).await
    }

    /// Belirli bir içerik için timeline getir
    pub async fn get_content_timeline(
        db: &DatabaseConnection,
        module_type: &str,
        content_type: &str,
        content_id: i64,
        user_id: Option<i64>,
        is_admin: bool,
    ) -> Result<Vec<TimelineEventModel>, DbErr> {
        let mut query = TimelineEvent::find()
            .filter(
                crate::modules::timeline::models::timeline_event::Column::ModuleType
                    .eq(module_type),
            )
            .filter(
                crate::modules::timeline::models::timeline_event::Column::ContentType
                    .eq(content_type),
            )
            .filter(
                crate::modules::timeline::models::timeline_event::Column::ContentId.eq(content_id),
            );

        // Yetki kontrolü
        if !is_admin {
            // Admin-only eventleri (ör: "Stok geri yüklendi") kullanıcıya gösterme
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::IsAdminOnly.eq(false),
            );

            if let Some(uid) = user_id {
                // Kullanıcı kendi eventlerini görebilir veya public olanları
                query = query.filter(
                    crate::modules::timeline::models::timeline_event::Column::UserId
                        .eq(uid)
                        .or(
                            crate::modules::timeline::models::timeline_event::Column::IsPublic
                                .eq(true),
                        ),
                );
            } else {
                // Login olmamış kullanıcı sadece public eventleri görebilir
                query = query.filter(
                    crate::modules::timeline::models::timeline_event::Column::IsPublic.eq(true),
                );
            }
        }

        query
            .order_by_desc(crate::modules::timeline::models::timeline_event::Column::CreatedAt)
            .all(db)
            .await
    }

    /// Kullanıcının timeline'ını getir
    pub async fn get_user_timeline(
        db: &DatabaseConnection,
        user_id: i64,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<Vec<TimelineEventModel>, DbErr> {
        let filter = TimelineEventFilter {
            user_id: Some(user_id),
            is_public: Some(true),
            limit,
            offset,
            ..Default::default()
        };

        Self::get_events(db, filter).await
    }

    /// Event sayısını getir
    pub async fn count_events(
        db: &DatabaseConnection,
        filter: TimelineEventFilter,
    ) -> Result<u64, DbErr> {
        let mut query = TimelineEvent::find();

        // Aynı filtreleri uygula
        if let Some(module_type) = filter.module_type {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::ModuleType
                    .eq(module_type),
            );
        }

        if let Some(content_type) = filter.content_type {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::ContentType
                    .eq(content_type),
            );
        }

        if let Some(content_id) = filter.content_id {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::ContentId.eq(content_id),
            );
        }

        if let Some(user_id) = filter.user_id {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::UserId.eq(user_id),
            );
        }

        if let Some(is_public) = filter.is_public {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::IsPublic.eq(is_public),
            );
        }

        if let Some(is_admin_only) = filter.is_admin_only {
            query = query.filter(
                crate::modules::timeline::models::timeline_event::Column::IsAdminOnly
                    .eq(is_admin_only),
            );
        }

        query.count(db).await
    }
}

impl Default for TimelineEventFilter {
    fn default() -> Self {
        Self {
            module_type: None,
            content_type: None,
            content_id: None,
            user_id: None,
            is_public: None,
            is_admin_only: None,
            limit: Some(50),
            offset: None,
        }
    }
}

// Helper makro - kolay event oluşturma için
#[macro_export]
macro_rules! create_timeline_event {
    ($db:expr, $module:expr, $content_type:expr, $content_id:expr, $event_type:expr, $title:expr) => {
        $crate::modules::timeline::services::timeline_service::TimelineService::create_event(
            $db,
            $crate::modules::timeline::services::timeline_service::CreateTimelineEventRequest {
                module_type: $module.to_string(),
                content_type: $content_type.to_string(),
                content_id: $content_id,
                event_type: $event_type,
                title: $title,
                description: None,
                icon: None,
                color: None,
                user_id: None,
                admin_user_id: None,
                metadata: None,
                is_public: None,
                is_admin_only: None,
            },
        )
    };

    ($db:expr, $module:expr, $content_type:expr, $content_id:expr, $event_type:expr, $title:expr, $user_id:expr) => {
        $crate::modules::timeline::services::timeline_service::TimelineService::create_event(
            $db,
            $crate::modules::timeline::services::timeline_service::CreateTimelineEventRequest {
                module_type: $module.to_string(),
                content_type: $content_type.to_string(),
                content_id: $content_id,
                event_type: $event_type,
                title: $title,
                description: None,
                icon: None,
                color: None,
                user_id: Some($user_id),
                admin_user_id: None,
                metadata: None,
                is_public: None,
                is_admin_only: None,
            },
        )
    };
}

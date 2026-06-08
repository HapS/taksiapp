use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "timeline_events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub module_type: String,
    pub content_type: String,
    pub content_id: i64,
    pub event_type: String,
    pub title: Json, // {"langs": {"tr": {"title": "..."}, "en": {"title": "..."}}}
    pub description: Option<Json>, // {"langs": {"tr": {"description": "..."}, "en": {"description": "..."}}}
    pub icon: Option<String>,
    pub color: Option<String>,
    pub user_id: Option<i64>,
    pub admin_user_id: Option<i64>,
    pub metadata: Option<Json>,
    pub is_public: bool,
    pub is_admin_only: bool,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::AdminUserId",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    AdminUser,
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Timeline event types enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimelineEventType {
    // Ecommerce events
    OrderCreated,
    OrderStatusChanged,
    OrderShipped,
    OrderDelivered,
    OrderCancelled,
    PaymentReceived,
    PaymentFailed,
    
    // Content events
    ProductCreated,
    ProductUpdated,
    ProductPublished,
    ProductUnpublished,
    ProductDeleted,
    
    // Auth events
    UserRegistered,
    UserLogin,
    UserProfileUpdated,
    PasswordChanged,
    
    // Admin events
    AdminAction,
    SystemUpdate,
    
    // Custom events
    Custom(String),
}

impl TimelineEventType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::OrderCreated => "order_created",
            Self::OrderStatusChanged => "order_status_changed",
            Self::OrderShipped => "order_shipped",
            Self::OrderDelivered => "order_delivered",
            Self::OrderCancelled => "order_cancelled",
            Self::PaymentReceived => "payment_received",
            Self::PaymentFailed => "payment_failed",
            Self::ProductCreated => "product_created",
            Self::ProductUpdated => "product_updated",
            Self::ProductPublished => "product_published",
            Self::ProductUnpublished => "product_unpublished",
            Self::ProductDeleted => "product_deleted",
            Self::UserRegistered => "user_registered",
            Self::UserLogin => "user_login",
            Self::UserProfileUpdated => "user_profile_updated",
            Self::PasswordChanged => "password_changed",
            Self::AdminAction => "admin_action",
            Self::SystemUpdate => "system_update",
            Self::Custom(s) => s,
        }
    }
    
    pub fn default_icon(&self) -> &str {
        match self {
            Self::OrderCreated => "bi-cart-plus",
            Self::OrderStatusChanged => "bi-arrow-repeat",
            Self::OrderShipped => "bi-truck",
            Self::OrderDelivered => "bi-check-circle",
            Self::OrderCancelled => "bi-x-circle",
            Self::PaymentReceived => "bi-credit-card",
            Self::PaymentFailed => "bi-exclamation-triangle",
            Self::ProductCreated => "bi-plus-circle",
            Self::ProductUpdated => "bi-pencil",
            Self::ProductPublished => "bi-eye",
            Self::ProductUnpublished => "bi-eye-slash",
            Self::ProductDeleted => "bi-trash",
            Self::UserRegistered => "bi-person-plus",
            Self::UserLogin => "bi-box-arrow-in-right",
            Self::UserProfileUpdated => "bi-person-gear",
            Self::PasswordChanged => "bi-shield-lock",
            Self::AdminAction => "bi-gear",
            Self::SystemUpdate => "bi-arrow-up-circle",
            Self::Custom(_) => "bi-info-circle",
        }
    }
    
    pub fn default_color(&self) -> &str {
        match self {
            Self::OrderCreated => "success",
            Self::OrderStatusChanged => "info",
            Self::OrderShipped => "primary",
            Self::OrderDelivered => "success",
            Self::OrderCancelled => "danger",
            Self::PaymentReceived => "success",
            Self::PaymentFailed => "danger",
            Self::ProductCreated => "success",
            Self::ProductUpdated => "info",
            Self::ProductPublished => "success",
            Self::ProductUnpublished => "warning",
            Self::ProductDeleted => "danger",
            Self::UserRegistered => "success",
            Self::UserLogin => "info",
            Self::UserProfileUpdated => "info",
            Self::PasswordChanged => "warning",
            Self::AdminAction => "secondary",
            Self::SystemUpdate => "primary",
            Self::Custom(_) => "info",
        }
    }
}
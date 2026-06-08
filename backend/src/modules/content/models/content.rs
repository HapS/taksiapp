// Base Content Model - Tüm content type'lar için ortak model
// Bu model page, blog, news gibi tüm içerik türleri için kullanılır
// Her content type kendi app'inde bu modeli extend eder

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
// use std::collections::HashMap;
/// Base Content Entity - Tüm içerik türleri için ortak tablo
/// content_type field'ı ile farklı türler ayrılır:
/// - "page" -> Page content
/// - "blog" -> Blog posts
/// - "news" -> News articles
/// vs.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "contents")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// JSON data - Her content type kendi structure'ını burada tutar
    /// Örnek: {"title": "...", "body": "...", "slug": "..."}
    pub data: JsonValue,

    /// Yayın durumu
    pub publish: bool,

    /// İçerik türü: "page", "blog", "product", "news" vs.
    pub content_type: String,

    /// Hiyerarşik yapı için parent ID
    pub parent_id: Option<i64>,

    /// Sıralama için order ID
    pub order_id: Option<i32>,

    /// İçeriği oluşturan kullanıcı ID
    pub user_id: Option<i64>,

    /// Global Context - Bu içerik global context'te kullanılabilir mi?
    pub gcx: bool,

    /// Timestamps
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(belongs_to = "Entity", from = "Column::ParentId", to = "Column::Id")]
    SelfRef,

    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id",
        // on_delete = "SetNull",
        // on_update = "Cascade"
    )]
    User,
}

impl Related<Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SelfRef.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Content type'a göre absolute URL üretir (slug-id formatında)
    /// Örnek:
    /// - "page" -> "/{lang}/page/{slug}-{id}"
    /// - "blog" -> "/{lang}/blog/{slug}-{id}"
    /// - "news" -> "/{lang}/news/{slug}-{id}"
    pub fn get_absolute_url(&self, lang: &str) -> Option<String> {
        // JSON data'dan slug'ı al
        let slug = self
            .data
            .get("langs")
            .and_then(|langs| langs.get(lang))
            .and_then(|lang_data| lang_data.get("slug"))
            .and_then(|s| s.as_str())?;

        // Content type'a göre URL formatla (slug-id formatı)
        match self.content_type.as_str() {
            "page" => Some(format!("/{}/page/{}-{}", lang, slug, self.id)),
            "blog" => Some(format!("/{}/blog/{}-{}", lang, slug, self.id)),
            "news" => Some(format!("/{}/news/{}-{}", lang, slug, self.id)),
            "product" => Some(format!("/{}/product/{}-{}", lang, slug, self.id)),
            "form" => Some(format!("/{}/form/{}-{}", lang, slug, self.id)),
            _ => Some(format!(
                "/{}/{}/{}-{}",
                lang, self.content_type, slug, self.id
            )),
        }
    }
}

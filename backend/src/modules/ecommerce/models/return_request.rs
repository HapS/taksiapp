use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// ReturnRequest - Ürün iade talepleri
/// Teslim edilen siparişlerdeki ürünlerin iade sürecini yönetir
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "return_requests")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Sipariş ID (carts tablosundan)
    pub cart_id: i64,

    /// Sipariş ürünü ID (cart_items tablosundan)
    pub cart_item_id: i64,

    /// Kullanıcı ID
    pub user_id: i64,

    /// İade edilecek adet
    pub quantity: i32,

    /// İade talebi durumu
    /// requested, approved, rejected, shipped, received, completed, cancelled
    pub status: String,

    /// İade sebebi
    /// defective, wrong_product, not_as_described, unwanted, damaged_in_shipping, other
    pub reason: String,

    /// Serbest metin açıklama (müşteri tarafından)
    pub reason_text: Option<String>,

    /// Müşterinin yüklediği fotoğraflar (JSON array of URLs)
    pub photos: Option<Json>,

    /// Admin notları (dahili)
    pub admin_notes: Option<String>,

    /// Red sebebi (müşteriye gösterilir)
    pub rejection_reason: Option<String>,

    /// İade kargo takip numarası (müşteri tarafından girilir)
    pub return_cargo_tracking_no: Option<String>,

    /// İade kargo şirketi
    pub return_cargo_company: Option<String>,

    /// İade tutarı (orijinal fiyattan farklı olabilir — kısmi iade, kesintiler vs.)
    pub refund_amount: Option<Decimal>,

    /// İade para birimi
    pub refund_currency: Option<String>,

    /// Timestamps
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: Option<DateTimeWithTimeZone>,
    pub approved_at: Option<DateTimeWithTimeZone>,
    pub shipped_at: Option<DateTimeWithTimeZone>,
    pub received_at: Option<DateTimeWithTimeZone>,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::cart::Entity",
        from = "Column::CartId",
        to = "super::cart::Column::Id"
    )]
    Cart,

    #[sea_orm(
        belongs_to = "super::cart_item::Entity",
        from = "Column::CartItemId",
        to = "super::cart_item::Column::Id"
    )]
    CartItem,

    #[sea_orm(
        belongs_to = "crate::modules::auth::models::user::Entity",
        from = "Column::UserId",
        to = "crate::modules::auth::models::user::Column::Id"
    )]
    User,
}

impl Related<super::cart::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cart.def()
    }
}

impl Related<super::cart_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CartItem.def()
    }
}

impl Related<crate::modules::auth::models::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// İade talebi durumları
pub mod status {
    /// Müşteri iade talebi oluşturdu, admin onayı bekleniyor
    pub const REQUESTED: &str = "requested";
    /// Admin onayladı, müşteri ürünü göndermeli
    pub const APPROVED: &str = "approved";
    /// Admin reddetti
    pub const REJECTED: &str = "rejected";
    /// Müşteri ürünü kargoya verdi
    pub const SHIPPED: &str = "shipped";
    /// Ürün depoya/admin'e ulaştı, inceleme bekliyor
    pub const RECEIVED: &str = "received";
    /// İade tamamlandı, para iadesi yapıldı
    pub const COMPLETED: &str = "completed";
    /// Müşteri iade talebini iptal etti
    pub const CANCELLED: &str = "cancelled";
}

/// İade sebepleri
pub mod reason {
    /// Ürün kusurlu/bozuk
    pub const DEFECTIVE: &str = "defective";
    /// Yanlış ürün gönderildi
    pub const WRONG_PRODUCT: &str = "wrong_product";
    /// Ürün açıklamaya uygun değil
    pub const NOT_AS_DESCRIBED: &str = "not_as_described";
    /// Ürünü istemiyorum / beğenmedim
    pub const UNWANTED: &str = "unwanted";
    /// Kargo sırasında hasar görmüş
    pub const DAMAGED_IN_SHIPPING: &str = "damaged_in_shipping";
    /// Diğer
    pub const OTHER: &str = "other";

    /// Tüm geçerli sebepler
    pub const ALL: &[&str] = &[
        DEFECTIVE,
        WRONG_PRODUCT,
        NOT_AS_DESCRIBED,
        UNWANTED,
        DAMAGED_IN_SHIPPING,
        OTHER,
    ];

    // Sebep kodundan görüntüleme metnine çevir (Türkçe)
    // pub fn display_text(reason: &str) -> &str {
    //     match reason {
    //         DEFECTIVE => "Ürün kusurlu/bozuk",
    //         WRONG_PRODUCT => "Yanlış ürün gönderildi",
    //         NOT_AS_DESCRIBED => "Ürün açıklamaya uygun değil",
    //         UNWANTED => "Ürünü istemiyorum",
    //         DAMAGED_IN_SHIPPING => "Kargo sırasında hasar görmüş",
    //         OTHER => "Diğer",
    //         _ => "Bilinmeyen sebep",
    //     }
    // }
}

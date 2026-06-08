use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// CartItem - Sepet öğeleri
/// Her satır sepetteki bir ürünü temsil eder
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cart_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Sepet ID
    pub cart_id: i64,

    /// Ürün ID (contents tablosundan)
    pub product_id: i64,

    /// Varyant anahtarı (option_values_display - benzersiz tanımlayıcı)
    /// Örnek: "XL / Kırmızı / 1 KG"
    pub variant_key: Option<String>,

    /// Varyant görüntüleme metni (kullanıcıya gösterilecek)
    /// Örnek: "Beden: XL, Renk: Kırmızı, Ağırlık: 1 KG"
    pub variant_display: Option<String>,

    /// Miktar
    pub quantity: i32,

    /// Ürün meta verisi (alışveriş tamamlandığında ürün bilgileri burada saklanır)
    /// Fiyat, başlık, varyant bilgileri vs. JSON formatında
    pub product_meta_data: Option<Json>,

    /// Ürünün orijinal para birimi (TRY, USD, EUR, vb.)
    pub currency: Option<String>,

    /// Ürünün orijinal fiyatı (kendi para biriminde)
    pub original_price: Option<Decimal>,

    /// Timestamps
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,

    /// CartItem durumu: null = normal, cancel_request = iptal talebi, cancel_accept = iptal onaylandı
    pub status: Option<String>,

    /// Refund fields
    pub refund_status: Option<String>, // credited_b2b, credited_b2c, bank_refunded
    pub refund_amount: Option<Decimal>,
    pub refund_date: Option<DateTimeWithTimeZone>,
    pub refund_method: Option<String>, // b2b_credit, b2c_credit, bank_transfer, credit_card
    pub refund_credit_id: Option<i64>, // user_credits.id
    pub refund_currency: Option<String>, // İade para birimi (TRY, AZN, USD, EUR vb.)

    /// Ürünün indirim oranı (eklenme anındaki değer)
    pub discount_percentage: Option<Decimal>,
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
        belongs_to = "crate::modules::content::models::content::Entity",
        from = "Column::ProductId",
        to = "crate::modules::content::models::content::Column::Id"
    )]
    Product,
}

impl ActiveModelBehavior for ActiveModel {}

/// CartItem durumları
pub mod status {
    // pub const NORMAL: Option<&str> = None; // Normal durum
    pub const CANCEL_REQUEST: &str = "cancel_request"; // İptal talebi
    pub const CANCEL_ACCEPT: &str = "cancel_accept"; // İptal onaylandı
}

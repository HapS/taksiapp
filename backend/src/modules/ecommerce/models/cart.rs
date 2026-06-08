use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Cart - Sepet tablosu
/// Her kullanıcı veya session için bir sepet
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "carts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Kullanıcı ID (zorunlu - sepet için login gerekli)
    pub user_id: i64,

    /// Seçilen kargo adresi ID'si
    pub address_id: Option<i64>,

    /// Seçilen fatura adresi ID'si (kurumsal)
    pub invoice_id: Option<i64>,

    /// Teslimat adresi metni (sipariş tamamlandığında dondurulur)
    pub address_line: Option<String>,

    /// Fatura adresi metni (sipariş tamamlandığında dondurulur)
    pub invoice_address_line: Option<String>,

    /// Seçilen ödeme yöntemi (taxonomy term slug)
    pub payment_method: Option<String>,

    /// Sipariş ID (kısa, özel karakter içermeyen unique string)
    pub order_id: Option<String>,

    /// Ödeme URL ID (UUID - ödeme sayfası ve callback için)
    pub payment_url: Option<String>,

    /// Cart durumu
    pub status: String,

    /// Callback verileri (JSONB)
    pub callback_data: Option<Json>,

    /// Sipariş notları (kullanıcı notu)
    pub notes: Option<String>,

    /// Toplam tutar (sipariş tamamlandığında sabitlenir)
    pub total_amount: Option<Decimal>,

    /// Sipariş para birimi (sale_currency - müşteriye gösterilen ve ödeme alınan para birimi)
    pub currency: Option<String>,

    /// Sipariş tamamlanma tarihi
    pub completed_at: Option<DateTimeWithTimeZone>,

    /// Sipariş tarihi (kullanıcı dostu görüntüleme için)
    pub order_date: Option<DateTimeWithTimeZone>,

    /// Kargo şirketi id daha önce string di şimdi id ye çevirdik
    pub cargo_company: Option<i64>,

    /// Kargo takip numarası
    pub cargo_tracking_no: Option<String>,

    /// Kargo ücreti (ödeme anında kaydedilir)
    pub cargo_price: Option<f64>,

    /// Kargo para birimi
    pub cargo_currency: Option<String>,

    /// Ödeme vade günü (kredili alışveriş için)
    pub payment_due_days: Option<i32>,

    /// Admin notları (admin'in sipariş hakkında yazdığı notlar)
    pub admin_notes: Option<String>,

    /// Cart tipi: 'b2c' (default) veya 'b2b'
    /// Kullanıcı B2B'ye geçse bile eski siparişleri B2C olarak kalır
    pub cart_type: String,

    /// Timestamps
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
        belongs_to = "crate::modules::ecommerce::models::kargo_sirketleri::Entity",
        from = "Column::CargoCompany",
        to = "crate::modules::ecommerce::models::kargo_sirketleri::Column::Id"
    )]
    CargoCompany,
}

impl ActiveModelBehavior for ActiveModel {}

/// Cart/Order durumları
pub mod status {
    pub const OPEN_CART: &str = "open_cart"; // Aktif sepet
    pub const PENDING: &str = "pending"; // Sipariş beklemede
    pub const CONFIRMED: &str = "confirmed"; // Onaylandı
    pub const PREPARING: &str = "preparing"; // Hazırlanıyor
    pub const SHIPPED: &str = "shipped"; // Kargoya verildi
    pub const DELIVERED: &str = "delivered"; // Teslim edildi
    pub const CANCELLED: &str = "cancelled"; // İptal edildi
    pub const REFUNDED: &str = "refunded"; // İade edildi
    pub const CANCEL_REQUEST: &str = "cancel_request"; // Kullanıcı tarafından iptal edildi
}

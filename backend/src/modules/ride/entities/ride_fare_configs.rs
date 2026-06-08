use sea_orm::entity::prelude::*;

/// Taksi ücretlendirme konfigürasyonu — il bazında.
///
/// Ücret formülü: min_fare + distance_km * per_km_fee
///
/// - min_fare    : Taban ücret (bindi-indi, kısa mesafe). Araç hareket etmese bile alınır.
/// - per_km_fee  : Her km için eklenen ücret.
/// - opening_fee : Taksimetre açılış ücreti (bilgi amaçlı, formülde kullanılmaz).
///
/// Örnek (Sakarya): min_fare=25, per_km_fee=8
///   2 km → 25 + 2*8 = 41 ₺
///   5 km → 25 + 5*8 = 65 ₺
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ride_fare_configs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub city_code: String,
    pub city_name: String,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub opening_fee: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub min_fare: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub per_km_fee: Decimal,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

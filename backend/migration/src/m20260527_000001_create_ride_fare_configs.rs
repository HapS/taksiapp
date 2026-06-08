use sea_orm_migration::prelude::*;

/// Taksi ücretlendirme konfigürasyonu — il bazında.
///
/// Şu an backend'de hardcode değerler kullanılmaktadır (controllers/ride.rs).
/// Bu tablo ileride il bazında dinamik ücretlendirme için kullanılacak.
///
/// Ücret formülü: max(min_fare, opening_fee + distance_km * per_km_fee)
///
/// Örnek kayıtlar:
///   city_code="sakarya", opening_fee=15.00, min_fare=25.00, per_km_fee=8.00
///   city_code="istanbul", opening_fee=25.00, min_fare=40.00, per_km_fee=12.00
///   city_code="ankara",   opening_fee=20.00, min_fare=35.00, per_km_fee=10.00
pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260527_000001_create_ride_fare_configs"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS ride_fare_configs (
                id           BIGSERIAL PRIMARY KEY,
                -- İl kodu (TR standart il kodu veya slug): "sakarya", "istanbul", "ankara"
                city_code    VARCHAR(50)    NOT NULL UNIQUE,
                -- Görünen il adı
                city_name    VARCHAR(100)   NOT NULL,
                -- Taksimetre açılış ücreti (₺) — araç durduğunda başlayan sabit ücret
                opening_fee  NUMERIC(10,2)  NOT NULL DEFAULT 15.00,
                -- Minimum ücret (₺) — bindi-indi, kısa mesafe tabanı
                min_fare     NUMERIC(10,2)  NOT NULL DEFAULT 25.00,
                -- Km başına ücret (₺)
                per_km_fee   NUMERIC(10,2)  NOT NULL DEFAULT 8.00,
                -- Bu konfigürasyon aktif mi? false ise hardcode fallback kullanılır
                is_active    BOOLEAN        NOT NULL DEFAULT true,
                created_at   TIMESTAMPTZ    NOT NULL DEFAULT NOW(),
                updated_at   TIMESTAMPTZ    NOT NULL DEFAULT NOW()
            );

            -- Sakarya için başlangıç verisi (hardcode ile aynı değerler)
            INSERT INTO ride_fare_configs (city_code, city_name, opening_fee, min_fare, per_km_fee)
            VALUES ('sakarya', 'Sakarya', 15.00, 25.00, 8.00)
            ON CONFLICT (city_code) DO NOTHING;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE IF EXISTS ride_fare_configs;")
            .await?;
        Ok(())
    }
}

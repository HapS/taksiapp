use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260523_000001_create_ride_tables"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            CREATE TYPE ride_status AS ENUM (
                'searching', 'accepted', 'picked_up', 'completed', 'cancelled', 'no_driver'
            );

            CREATE TYPE offer_status AS ENUM (
                'pending', 'accepted', 'rejected', 'timeout'
            );

            CREATE TABLE IF NOT EXISTS drivers (
                id                  BIGSERIAL PRIMARY KEY,
                user_id             BIGINT REFERENCES users(id) ON DELETE SET NULL,
                full_name           VARCHAR NOT NULL,
                phone               VARCHAR NOT NULL UNIQUE,
                vehicle_plate       VARCHAR NOT NULL,
                vehicle_model       VARCHAR NOT NULL,
                rating              DOUBLE PRECISION NOT NULL DEFAULT 5.0,
                is_active           BOOLEAN NOT NULL DEFAULT true,
                is_online           BOOLEAN NOT NULL DEFAULT false,
                current_lat         DOUBLE PRECISION,
                current_lon         DOUBLE PRECISION,
                location_updated_at TIMESTAMPTZ,
                created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX idx_drivers_is_online ON drivers(is_online);
            CREATE INDEX idx_drivers_location ON drivers(current_lat, current_lon)
                WHERE is_online = true AND is_active = true;

            CREATE TABLE IF NOT EXISTS rides (
                id              BIGSERIAL PRIMARY KEY,
                user_id         BIGINT NOT NULL REFERENCES users(id),
                driver_id       BIGINT REFERENCES drivers(id),
                status          ride_status NOT NULL DEFAULT 'searching',
                pickup_lat      DOUBLE PRECISION NOT NULL,
                pickup_lon      DOUBLE PRECISION NOT NULL,
                pickup_address  TEXT NOT NULL,
                dropoff_lat     DOUBLE PRECISION NOT NULL,
                dropoff_lon     DOUBLE PRECISION NOT NULL,
                dropoff_address TEXT NOT NULL,
                distance_km     DOUBLE PRECISION,
                duration_sec    INTEGER,
                fare_amount     NUMERIC(10, 2),
                requested_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                accepted_at     TIMESTAMPTZ,
                picked_up_at    TIMESTAMPTZ,
                completed_at    TIMESTAMPTZ,
                cancelled_at    TIMESTAMPTZ
            );

            CREATE INDEX idx_rides_user_id   ON rides(user_id);
            CREATE INDEX idx_rides_driver_id ON rides(driver_id);
            CREATE INDEX idx_rides_status    ON rides(status);

            CREATE TABLE IF NOT EXISTS ride_offers (
                id           BIGSERIAL PRIMARY KEY,
                ride_id      BIGINT NOT NULL REFERENCES rides(id) ON DELETE CASCADE,
                driver_id    BIGINT NOT NULL REFERENCES drivers(id),
                status       offer_status NOT NULL DEFAULT 'pending',
                offer_order  INTEGER NOT NULL,
                offered_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                responded_at TIMESTAMPTZ
            );

            CREATE INDEX idx_ride_offers_ride_id ON ride_offers(ride_id);
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            DROP TABLE IF EXISTS ride_offers;
            DROP TABLE IF EXISTS rides;
            DROP TABLE IF EXISTS drivers;
            DROP TYPE IF EXISTS offer_status;
            DROP TYPE IF EXISTS ride_status;
            "#,
        )
        .await?;
        Ok(())
    }
}

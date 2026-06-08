use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260604_000001_create_locations"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm;")
            .await?;

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS locations (
                id          BIGSERIAL PRIMARY KEY,
                name        TEXT NOT NULL,
                address     TEXT NOT NULL DEFAULT '',
                lat         DOUBLE PRECISION NOT NULL,
                lon         DOUBLE PRECISION NOT NULL,
                category    VARCHAR(64) NOT NULL DEFAULT 'other',
                is_active   BOOLEAN NOT NULL DEFAULT true,
                created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX idx_locations_active ON locations(is_active) WHERE is_active = true;
            CREATE INDEX idx_locations_name_trgm ON locations USING gin(name gin_trgm_ops);
            CREATE INDEX idx_locations_category ON locations(category) WHERE is_active = true;
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE IF EXISTS locations;")
            .await?;
        Ok(())
    }
}
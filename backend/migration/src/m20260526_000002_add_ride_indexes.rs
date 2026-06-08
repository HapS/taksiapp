use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // rides tablosu — aktif yolculuk sorguları için composite index
        db.execute_unprepared(
            r#"
            CREATE INDEX IF NOT EXISTS idx_rides_active_driver
            ON rides(driver_id, status)
            WHERE driver_id IS NOT NULL AND status IN ('accepted', 'picked_up');
            "#,
        )
        .await?;

        // ride_offers — sürücü-teklif eşleştirme için composite index
        db.execute_unprepared(
            r#"
            CREATE INDEX IF NOT EXISTS idx_ride_offers_driver_ride
            ON ride_offers(driver_id, ride_id);
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(r#"DROP INDEX IF EXISTS idx_rides_active_driver;"#)
            .await?;
        db.execute_unprepared(r#"DROP INDEX IF EXISTS idx_ride_offers_driver_ride;"#)
            .await?;
        Ok(())
    }
}

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260526_000001_add_driver_user_id_unique"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "DELETE FROM drivers WHERE id NOT IN (SELECT MIN(id) FROM drivers WHERE user_id IS NOT NULL GROUP BY user_id) AND user_id IS NOT NULL;",
        ).await?;

        db.execute_unprepared(
            "DO $$ BEGIN \
             ALTER TABLE drivers ADD CONSTRAINT drivers_user_id_unique UNIQUE (user_id); \
             EXCEPTION WHEN duplicate_object THEN NULL; \
             END $$;",
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "ALTER TABLE drivers DROP CONSTRAINT IF EXISTS drivers_user_id_unique;",
        ).await?;

        Ok(())
    }
}
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(include_str!("../sql/schema.sql")).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let tables = [
            "vocabulary_categories", "user_roles", "user_permissions",
            "timeline_events", "terms", "settings", "sessions",
            "ride_offers", "roles", "role_permissions", "rides",
            "ride_fare_configs", "permissions", "password_resets",
            "media", "mail_queue", "locations", "homepage",
            "form_submissions", "drivers", "districts", "countries",
            "contents", "content_terms", "comments", "cities",
        ];
        for table in &tables {
            let sql = format!("DROP TABLE IF EXISTS public.{} CASCADE", table);
            db.execute_unprepared(&sql).await?;
        }
        db.execute_unprepared("DROP TYPE IF EXISTS public.ride_status").await?;
        db.execute_unprepared("DROP TYPE IF EXISTS public.payment_method_enum").await?;
        db.execute_unprepared("DROP TYPE IF EXISTS public.offer_status").await?;
        Ok(())
    }
}

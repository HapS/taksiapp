use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        
        // Önce mevcut veriyi temizle (string -> NULL)
        db.execute_unprepared(
            "UPDATE carts SET cargo_company = NULL WHERE cargo_company IS NOT NULL"
        )
        .await
        .map(|_| ())?;
        
        // Tipi bigint olarak değiştir
        db.execute_unprepared(
            r#"ALTER TABLE carts 
               ALTER COLUMN cargo_company 
               TYPE bigint 
               USING cargo_company::bigint"#,
        )
        .await
        .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        
        db.execute_unprepared(
            r#"ALTER TABLE carts 
               ALTER COLUMN cargo_company 
               TYPE varchar(255) 
               USING cargo_company::varchar"#,
        )
        .await
        .map(|_| ())
    }
}

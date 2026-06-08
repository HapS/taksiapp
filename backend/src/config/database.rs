use sea_orm::{Database, DatabaseConnection, DbErr};

#[derive(Debug, Clone)]
pub struct DatabaseConfig;

impl DatabaseConfig {
    pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
        Database::connect(database_url).await
    }
}

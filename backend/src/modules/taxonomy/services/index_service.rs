// Index Service - Automatic JSONB index management for vocabularies
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use std::collections::HashSet;

pub struct IndexService;

impl IndexService {
    /// Ensure all necessary JSONB indexes exist for product attributes
    pub async fn ensure_product_attribute_indexes(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
        // Main GIN index for all product attributes
        let main_index_sql = r#"
            CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_contents_product_attributes 
            ON contents USING GIN ((data->'product'->'attributes'))
        "#;
        
        let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, main_index_sql);
        db.execute(statement).await?;
        
        // Composite indexes for efficient filtering
        let composite_indexes = vec![
            r#"CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_contents_type_publish 
               ON contents (content_type, publish) WHERE deleted_at IS NULL"#,
            r#"CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_contents_product_filter 
               ON contents (content_type, publish, deleted_at, id) WHERE content_type = 'product'"#,
        ];
        
        for index_sql in composite_indexes {
            let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, index_sql);
            db.execute(statement).await?;
        }
        
        Ok(())
    }
    
    /// Get all existing JSONB indexes for product attributes
    #[allow(dead_code)]
    pub async fn get_existing_attribute_indexes(db: &DatabaseConnection) -> Result<HashSet<String>, sea_orm::DbErr> {
        let sql = r#"
            SELECT indexname 
            FROM pg_indexes 
            WHERE tablename = 'contents' 
            AND indexname LIKE 'idx_contents_attr_%'
        "#;
        
        let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        let results = db.query_all(statement).await?;
        
        let indexes: HashSet<String> = results
            .iter()
            .filter_map(|row| row.try_get::<String>("", "indexname").ok())
            .collect();
            
        Ok(indexes)
    }
    
    /// Create index for a specific vocabulary attribute (optional optimization)
    pub async fn create_vocabulary_index(
        db: &DatabaseConnection, 
        vocabulary_name: &str
    ) -> Result<(), sea_orm::DbErr> {
        let index_name = format!("idx_contents_attr_{}", vocabulary_name);
        let sql = format!(
            r#"CREATE INDEX CONCURRENTLY IF NOT EXISTS {} 
               ON contents USING GIN ((data->'product'->'attributes'->'{}'))"#,
            index_name, vocabulary_name
        );
        
        let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        db.execute(statement).await?;
        
        println!("Created index for vocabulary: {}", vocabulary_name);
        Ok(())
    }
    
    /// Drop index for a vocabulary that no longer exists
    #[allow(dead_code)]
    pub async fn drop_vocabulary_index(
        db: &DatabaseConnection, 
        vocabulary_name: &str
    ) -> Result<(), sea_orm::DbErr> {
        let index_name = format!("idx_contents_attr_{}", vocabulary_name);
        let sql = format!("DROP INDEX CONCURRENTLY IF EXISTS {}", index_name);
        
        let statement = Statement::from_string(sea_orm::DatabaseBackend::Postgres, sql);
        db.execute(statement).await?;
        
        println!("Dropped index for vocabulary: {}", vocabulary_name);
        Ok(())
    }
    
    /// Sync indexes with current vocabularies (cleanup unused indexes)
    #[allow(dead_code)]
    pub async fn sync_vocabulary_indexes(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
        use crate::modules::taxonomy::models::Vocabulary;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
        
        // Get all product_attributes vocabularies
        let vocabularies = Vocabulary::find()
            .filter(crate::modules::taxonomy::models::vocabulary::Column::VocabularyType.eq("product_attributes"))
            .all(db)
            .await?;
        
        let current_vocab_names: HashSet<String> = vocabularies
            .iter()
            .filter_map(|v| {
                v.data.get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        
        // Get existing indexes
        let existing_indexes = Self::get_existing_attribute_indexes(db).await?;
        
        // Find indexes to drop (no longer have corresponding vocabulary)
        for index_name in existing_indexes {
            if let Some(vocab_name) = index_name.strip_prefix("idx_contents_attr_") {
                if !current_vocab_names.contains(vocab_name) {
                    Self::drop_vocabulary_index(db, vocab_name).await?;
                }
            }
        }
        
        println!("Synced vocabulary indexes");
        Ok(())
    }
    
    /// Initialize all necessary indexes on application startup
    #[allow(dead_code)]
    pub async fn initialize_indexes(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
        println!("Initializing JSONB indexes for product attributes...");
        
        // Ensure main indexes exist
        Self::ensure_product_attribute_indexes(db).await?;
        
        // Sync with existing vocabularies
        Self::sync_vocabulary_indexes(db).await?;
        
        println!("JSONB indexes initialized successfully");
        Ok(())
    }
}
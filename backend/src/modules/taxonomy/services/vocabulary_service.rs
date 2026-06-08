// Vocabulary Service - Business logic and database operations
use crate::modules::taxonomy::helpers::{
    CreateVocabularyRequest, UpdateVocabularyRequest, VocabularyExtensions, VocabularyResponse,
    VocabularyType,
};
use crate::modules::taxonomy::models::term::{Column as TermColumn, Entity as Term};
use crate::modules::taxonomy::models::vocabulary::{
    Column, Vocabulary, VocabularyActiveModel, VocabularyModel,
};
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::sea_query::{Expr, ExprTrait, Func};
use sea_orm::*;

#[derive(Debug)]
pub enum VocabularyError {
    NotFound,
    DatabaseError(DbErr),
    InvalidData(String),
}

impl std::fmt::Display for VocabularyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VocabularyError::NotFound => write!(f, "Vocabulary not found"),
            VocabularyError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
            VocabularyError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

impl std::error::Error for VocabularyError {}

impl From<DbErr> for VocabularyError {
    fn from(err: DbErr) -> Self {
        VocabularyError::DatabaseError(err)
    }
}

/// List all vocabularies
pub async fn list_vocabularies(
    db: &DatabaseConnection,
    search: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    lang: &str,
) -> Result<Vec<VocabularyResponse>, VocabularyError> {
    let mut query = Vocabulary::find();

    // Search filtering (JSON text search in data column)
    if let Some(search_term) = search {
        if !search_term.is_empty() {
            let search_pattern = format!("%{}%", search_term.to_lowercase());
            query = query.filter(
                Func::lower(Func::cast_as(
                    Expr::col(Column::Data),
                    sea_orm::sea_query::Alias::new("text"),
                ))
                .like(search_pattern),
            );
        }
    }

    // Apply sorting
    if let Some(field) = sort_by {
        let order = if sort_order
            .unwrap_or_else(|| "asc".to_string())
            .to_lowercase()
            == "desc"
        {
            sea_orm::Order::Desc
        } else {
            sea_orm::Order::Asc
        };

        match field.as_str() {
            "id" => {
                query = query.order_by(Column::Id, order);
            }
            "vocabulary_type" => {
                query = query.order_by(Column::VocabularyType, order);
            }
            "order" | "order_id" => {
                query = query.order_by(Column::OrderId, order);
            }
            "gcx" => {
                query = query.order_by(Column::Gcx, order);
            }
            "lock" => {
                query = query.order_by(Column::Lock, order);
            }
            "hide" => {
                query = query.order_by(Column::Hide, order);
            }
            "created_at" => {
                query = query.order_by(Column::CreatedAt, order);
            }
            _ => {
                query = query.order_by_asc(Column::OrderId);
            }
        }
    } else {
        query = query.order_by_asc(Column::OrderId);
    }

    let vocabularies = query.all(db).await?;

    let response = vocabularies
        .into_iter()
        .map(|v| v.to_response(lang))
        .collect();

    Ok(response)
}

pub async fn list_vocabularies_by_type(
    db: &DatabaseConnection,
    vocabulary_type: &str,
    search: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    lang: &str,
) -> Result<Vec<VocabularyResponse>, VocabularyError> {
    let mut query = Vocabulary::find().filter(Column::VocabularyType.eq(vocabulary_type));

    // Search filtering
    if let Some(search_term) = search {
        if !search_term.is_empty() {
            let search_pattern = format!("%{}%", search_term.to_lowercase());
            query = query.filter(
                Func::lower(Func::cast_as(
                    Expr::col(Column::Data),
                    sea_orm::sea_query::Alias::new("text"),
                ))
                .like(search_pattern),
            );
        }
    }

    // Apply sorting
    if let Some(field) = sort_by {
        let order = if sort_order
            .unwrap_or_else(|| "asc".to_string())
            .to_lowercase()
            == "desc"
        {
            sea_orm::Order::Desc
        } else {
            sea_orm::Order::Asc
        };

        match field.as_str() {
            "id" => {
                query = query.order_by(Column::Id, order);
            }
            "order" | "order_id" => {
                query = query.order_by(Column::OrderId, order);
            }
            "gcx" => {
                query = query.order_by(Column::Gcx, order);
            }
            "lock" => {
                query = query.order_by(Column::Lock, order);
            }
            "hide" => {
                query = query.order_by(Column::Hide, order);
            }
            "created_at" => {
                query = query.order_by(Column::CreatedAt, order);
            }
            _ => {
                query = query.order_by_asc(Column::OrderId);
            }
        }
    } else {
        query = query.order_by_asc(Column::OrderId);
    }

    let vocabularies = query.all(db).await?;

    let response = vocabularies
        .into_iter()
        .map(|v| v.to_response(lang))
        .collect();

    Ok(response)
}

/// Get vocabulary by ID
pub async fn get_vocabulary_by_id(
    db: &DatabaseConnection,
    id: i64,
    lang: &str,
) -> Result<VocabularyResponse, VocabularyError> {
    let vocabulary = Vocabulary::find_by_id(id)
        .one(db)
        .await?
        .ok_or(VocabularyError::NotFound)?;

    Ok(vocabulary.to_response(lang))
}

/// Create new vocabulary
pub async fn create_vocabulary(
    db: &DatabaseConnection,
    request: CreateVocabularyRequest,
) -> Result<VocabularyModel, VocabularyError> {
    let vocabulary_data = serde_json::to_value(&request.data)
        .map_err(|e| VocabularyError::InvalidData(e.to_string()))?;

    let now: DateTimeWithTimeZone = chrono::Utc::now().into();

    let vocabulary = VocabularyActiveModel {
        data: Set(vocabulary_data),
        vocabulary_type: Set(request.vocabulary_type.to_string()),
        gcx: Set(request.gcx.unwrap_or(false)),
        lock: Set(request.lock.unwrap_or(false)),
        hide: Set(request.hide.unwrap_or(false)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    };

    let vocabulary = vocabulary.insert(db).await?;

    // Auto-create indexes for product_attributes vocabularies
    if request.vocabulary_type == VocabularyType::ProductAttributes {
        if let Some(name) = request.data.get("name").and_then(|n| n.as_str()) {
            if let Err(e) =
                super::index_service::IndexService::create_vocabulary_index(db, name).await
            {
                eprintln!("Failed to create index for vocabulary {}: {}", name, e);
                // Don't fail the vocabulary creation if index creation fails
            }
        }

        // Ensure main product attributes index exists
        if let Err(e) =
            super::index_service::IndexService::ensure_product_attribute_indexes(db).await
        {
            eprintln!("Failed to ensure product attribute indexes: {}", e);
        }
    }

    Ok(vocabulary)
}

/// Create new vocabulary with cache refresh
pub async fn create_vocabulary_with_cache_refresh(
    db: &DatabaseConnection,
    request: CreateVocabularyRequest,
    global_context_cache: &std::sync::Arc<
        std::sync::RwLock<std::collections::BTreeMap<String, serde_json::Value>>,
    >,
) -> Result<VocabularyModel, VocabularyError> {
    let vocabulary = create_vocabulary(db, request).await?;

    // GCX ise cache'i yenile
    if vocabulary.gcx {
        if let Err(e) =
            crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
                db,
                global_context_cache,
            )
            .await
        {
            eprintln!("Global context cache yenileme hatası: {}", e);
            // Cache hatası ana işlemi etkilemesin
        }
    }

    Ok(vocabulary)
}

/// Update vocabulary
pub async fn update_vocabulary(
    db: &DatabaseConnection,
    id: i64,
    request: UpdateVocabularyRequest,
) -> Result<VocabularyModel, VocabularyError> {
    let vocabulary = Vocabulary::find_by_id(id)
        .one(db)
        .await?
        .ok_or(VocabularyError::NotFound)?;

    let mut active_model: VocabularyActiveModel = vocabulary.into();

    if let Some(data) = request.data {
        let vocabulary_data =
            serde_json::to_value(&data).map_err(|e| VocabularyError::InvalidData(e.to_string()))?;
        active_model.data = Set(vocabulary_data);
    }

    if let Some(vocabulary_type) = request.vocabulary_type {
        active_model.vocabulary_type = Set(vocabulary_type.to_string());
    }

    if let Some(gcx) = request.gcx {
        active_model.gcx = Set(gcx);
    }
    if let Some(lock) = request.lock {
        active_model.lock = Set(lock);
    }
    if let Some(hide) = request.hide {
        active_model.hide = Set(hide);
    }

    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    let updated = active_model.update(db).await?;
    Ok(updated)
}

/// Update vocabulary with cache refresh
pub async fn update_vocabulary_with_cache_refresh(
    db: &DatabaseConnection,
    id: i64,
    request: UpdateVocabularyRequest,
    global_context_cache: &std::sync::Arc<
        std::sync::RwLock<std::collections::BTreeMap<String, serde_json::Value>>,
    >,
) -> Result<VocabularyModel, VocabularyError> {
    let old_vocabulary = Vocabulary::find_by_id(id)
        .one(db)
        .await?
        .ok_or(VocabularyError::NotFound)?;

    let updated_vocabulary = update_vocabulary(db, id, request).await?;

    // GCX durumu değişmişse veya gcx=true ise cache'i yenile
    let gcx_changed = old_vocabulary.gcx != updated_vocabulary.gcx;
    let lock_changed = old_vocabulary.lock != updated_vocabulary.lock;
    let hide_changed = old_vocabulary.hide != updated_vocabulary.hide;

    if updated_vocabulary.gcx || gcx_changed || lock_changed || hide_changed {
        if let Err(e) =
            crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
                db,
                global_context_cache,
            )
            .await
        {
            eprintln!("Global context cache yenileme hatası: {}", e);
            // Cache hatası ana işlemi etkilemesin
        }
    }

    Ok(updated_vocabulary)
}

/// Delete vocabulary with cache refresh
pub async fn delete_vocabulary_with_cache_refresh(
    db: &DatabaseConnection,
    id: i64,
    global_context_cache: &std::sync::Arc<
        std::sync::RwLock<std::collections::BTreeMap<String, serde_json::Value>>,
    >,
) -> Result<(), VocabularyError> {
    let vocabulary = Vocabulary::find_by_id(id)
        .one(db)
        .await?
        .ok_or(VocabularyError::NotFound)?;

    // Prevent deleting a vocabulary that still has terms
    let has_terms = Term::find()
        .filter(TermColumn::VocabularyId.eq(id))
        .one(db)
        .await?
        .is_some();

    if has_terms {
        return Err(VocabularyError::InvalidData(
            "Bu vocabulary altında terimler bulunduğu için silinemez.".to_string(),
        ));
    }

    let was_gcx = vocabulary.gcx;

    vocabulary.delete(db).await?;

    if was_gcx {
        if let Err(e) =
            crate::modules::content::helpers::global_context_helper::refresh_global_context_cache(
                db,
                global_context_cache,
            )
            .await
        {
            eprintln!("Global context cache yenileme hatası: {}", e);
        }
    }

    Ok(())
}

/// Update vocabulary order
pub async fn update_vocabulary_order(
    db: &DatabaseConnection,
    orders: Vec<(i64, i32)>, // (id, order_id)
) -> Result<(), VocabularyError> {
    for (id, order_id) in orders {
        let vocabulary = Vocabulary::find_by_id(id)
            .one(db)
            .await?
            .ok_or(VocabularyError::NotFound)?;

        let mut active_model: VocabularyActiveModel = vocabulary.into();
        active_model.order_id = Set(Some(order_id));
        active_model.updated_at = Set(Some(chrono::Utc::now().into()));

        active_model.update(db).await?;
    }

    Ok(())
}

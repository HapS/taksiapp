// Term Service - Business logic and database operations
use crate::modules::taxonomy::helpers::{
    BreadcrumbItem, CreateTermRequest, TermExtensions, TermResponse, UpdateTermRequest,
};
use crate::modules::taxonomy::models::term::{
    self as term, Column, Term, TermActiveModel, TermModel,
};
use sea_orm::prelude::DateTimeWithTimeZone;
use sea_orm::sea_query::{Expr, ExprTrait, Func};
use sea_orm::*;

#[derive(Debug)]
pub enum TermError {
    NotFound,
    DatabaseError(DbErr),
    InvalidData(String),
}

impl std::fmt::Display for TermError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TermError::NotFound => write!(f, "Terim bulunamadı"),
            TermError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
            TermError::InvalidData(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for TermError {}

impl From<DbErr> for TermError {
    fn from(err: DbErr) -> Self {
        TermError::DatabaseError(err)
    }
}

/// List all terms
pub async fn list_terms(
    db: &DatabaseConnection,
    lang: &str,
) -> Result<Vec<TermResponse>, TermError> {
    let terms = Term::find()
        .order_by_asc(Column::VocabularyId)
        .order_by_asc(Column::OrderId)
        .all(db)
        .await?;

    let response = terms.into_iter().map(|t| t.to_response(lang)).collect();

    Ok(response)
}

/// Get term by ID
// pub async fn get_term_by_id(
//     db: &DatabaseConnection,
//     id: i64,
//     lang: &str,
// ) -> Result<TermResponse, TermError> {
//     let term = Term::find_by_id(id)
//         .one(db)
//         .await?
//         .ok_or(TermError::NotFound)?;

//     Ok(term.to_response(lang))
// }

/// Get term by ID with breadcrumbs (for edit page)
pub async fn get_term_by_id_with_breadcrumbs(
    db: &DatabaseConnection,
    id: i64,
    lang: &str,
) -> Result<(TermResponse, Vec<BreadcrumbItem>), TermError> {
    let term = Term::find_by_id(id)
        .one(db)
        .await?
        .ok_or(TermError::NotFound)?;

    let breadcrumbs = if let Some(parent_id) = term.parent_id {
        build_breadcrumb_path(db, parent_id, term.vocabulary_id, lang).await
    } else {
        Vec::new()
    };

    Ok((term.to_response(lang), breadcrumbs))
}

/// Create new term
pub async fn create_term(
    db: &DatabaseConnection,
    request: CreateTermRequest,
) -> Result<TermModel, TermError> {
    let term_data =
        serde_json::to_value(&request.data).map_err(|e| TermError::InvalidData(e.to_string()))?;

    // Validation: ensure title uniqueness within vocabulary and among siblings (case-insensitive)
    if let Some(title) = crate::modules::taxonomy::helpers::term_helper::get_string_from_json(
        &term_data, "title", "tr",
    ) {
        let title_norm = title.trim().to_lowercase();

        // Fetch existing terms in this vocabulary
        let existing_terms = Term::find()
            .filter(Column::VocabularyId.eq(request.vocabulary_id))
            .all(db)
            .await?;

        // 1) Check global vocabulary-level uniqueness
        for existing in existing_terms.iter() {
            if existing.get_title("tr").trim().to_lowercase() == title_norm {
                return Err(TermError::InvalidData(
                    "Bu vocabulary içinde aynı başlığa sahip başka bir terim zaten mevcut."
                        .to_string(),
                ));
            }
        }

        // 2) Check sibling uniqueness if a parent is specified
        if let Some(parent_id) = request.parent_id {
            for existing in existing_terms
                .iter()
                .filter(|t| t.parent_id == Some(parent_id))
            {
                if existing.get_title("tr").trim().to_lowercase() == title_norm {
                    return Err(TermError::InvalidData(
                        "Bu parentın altında aynı isimde bir alt terim zaten mevcut.".to_string(),
                    ));
                }
            }
        }
    }

    // Get max order_id for this vocabulary
    let max_order_id = Term::find()
        .filter(Column::VocabularyId.eq(request.vocabulary_id))
        .order_by_desc(Column::OrderId)
        .one(db)
        .await?
        .and_then(|t| t.order_id)
        .unwrap_or(0);

    let now: DateTimeWithTimeZone = chrono::Utc::now().into();

    let term = TermActiveModel {
        vocabulary_id: Set(request.vocabulary_id),
        data: Set(term_data),
        parent_id: Set(request.parent_id),
        order_id: Set(Some(max_order_id + 1)),
        publish: Set(request.publish),
        lock: Set(request.lock.unwrap_or(false)),
        hide: Set(request.hide.unwrap_or(false)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    };

    let term = term.insert(db).await?;
    Ok(term)
}

/// Update term
pub async fn update_term(
    db: &DatabaseConnection,
    id: i64,
    request: UpdateTermRequest,
) -> Result<TermModel, TermError> {
    let term = Term::find_by_id(id)
        .one(db)
        .await?
        .ok_or(TermError::NotFound)?;

    // Prevent update if term is locked
    // if term.lock {
    //     return Err(TermError::InvalidData(
    //         "Bu kategori kilitli olduğu için düzenlenemez.".to_string(),
    //     ));
    // }

    // Determine proposed title (current or updated) and proposed parent (current or updated)
    let current_title_norm = term.get_title("tr").trim().to_lowercase();
    let mut proposed_title_norm = current_title_norm.clone();
    let mut proposed_parent = term.parent_id;

    let mut title_changed = false;
    let mut parent_changed = false;

    if let Some(ref new_data) = request.data {
        let tmp_value =
            serde_json::to_value(new_data).map_err(|e| TermError::InvalidData(e.to_string()))?;
        if let Some(t) = crate::modules::taxonomy::helpers::term_helper::get_string_from_json(
            &tmp_value, "title", "tr",
        ) {
            let t_norm = t.trim().to_lowercase();
            if t_norm != current_title_norm {
                title_changed = true;
                proposed_title_norm = t_norm;
            }
        }
    }

    if let Some(new_parent_id_opt) = request.parent_id {
        if let Some(pid) = new_parent_id_opt {
            if pid == id {
                return Err(TermError::InvalidData(
                    "Bir terim kendisinin alt terimi olamaz.".to_string(),
                ));
            }
        }
        if proposed_parent != new_parent_id_opt {
            parent_changed = true;
            proposed_parent = new_parent_id_opt;
        }
    }

    // Only validate uniqueness if title or parent will change (to avoid blocking unrelated updates)
    if title_changed || parent_changed {
        // Fetch other terms in the same vocabulary excluding current term
        let other_terms = Term::find()
            .filter(term::Column::VocabularyId.eq(term.vocabulary_id))
            .filter(Column::Id.ne(id))
            .all(db)
            .await?;

        // If title changed, ensure no other term in the same vocabulary has the same title
        if title_changed {
            for other in &other_terms {
                if other.get_title("tr").trim().to_lowercase() == proposed_title_norm {
                    return Err(TermError::InvalidData(
                        "Bu vocabulary içinde aynı başlığa sahip başka bir terim zaten mevcut."
                            .to_string(),
                    ));
                }
            }
        }

        // If parent changed (or title changed), ensure no sibling in target parent has same title
        for other in other_terms
            .iter()
            .filter(|t| t.parent_id == proposed_parent)
        {
            if other.get_title("tr").trim().to_lowercase() == proposed_title_norm {
                return Err(TermError::InvalidData(
                    "Bu parentın altında aynı isimde bir alt terim zaten mevcut.".to_string(),
                ));
            }
        }
    }

    // Apply updates to active model
    let mut active_model: TermActiveModel = term.clone().into();

    if let Some(new_data) = request.data {
        let data_value =
            serde_json::to_value(&new_data).map_err(|e| TermError::InvalidData(e.to_string()))?;
        active_model.data = Set(data_value);
    }

    if let Some(parent_id_opt) = request.parent_id {
        active_model.parent_id = Set(parent_id_opt);
    }

    if let Some(publish) = request.publish {
        active_model.publish = Set(publish);
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

/// Delete term
pub async fn delete_term(db: &DatabaseConnection, id: i64) -> Result<(), TermError> {
    let term = Term::find_by_id(id)
        .one(db)
        .await?
        .ok_or(TermError::NotFound)?;

    // Prevent deletion if term is locked
    if term.lock {
        return Err(TermError::InvalidData(
            "Bu kategori kilitli olduğu için silinemez.".to_string(),
        ));
    }

    // Prevent deletion if this term has child terms
    let child_count = Term::find()
        .filter(Column::ParentId.eq(term.id))
        .count(db)
        .await?;

    if child_count > 0 {
        return Err(TermError::InvalidData(
            "Bu terimin altında başka terimler bulunduğu için silinemez.".to_string(),
        ));
    }

    term.delete(db).await?;
    Ok(())
}

/// Get hierarchical terms by vocabulary with pagination and filtering
pub async fn get_terms_hierarchical(
    db: &DatabaseConnection,
    vocabulary_id: i64,
    parent_id: Option<String>,
    page: u64,
    limit: u64,
    search: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    lang: &str,
) -> Result<(Vec<TermResponse>, u64, Vec<BreadcrumbItem>), TermError> {
    let offset = (page - 1) * limit;

    // Base query
    let mut query_builder = Term::find().filter(Column::VocabularyId.eq(vocabulary_id));

    // Parent ID filtering
    if let Some(parent_id_str) = &parent_id {
        // Skip filtering if parent_id is "all" - return all terms
        if parent_id_str != "all" {
            if parent_id_str == "null" {
                query_builder = query_builder.filter(Column::ParentId.is_null());
            } else if let Ok(pid) = parent_id_str.parse::<i64>() {
                query_builder = query_builder.filter(Column::ParentId.eq(pid));
            }
        }
    }

    // Search filtering (JSON text search)
    if let Some(search_term) = &search {
        if !search_term.is_empty() {
            let search_pattern = format!("%{}%", search_term.to_lowercase());
            query_builder = query_builder.filter(
                Func::lower(Func::cast_as(
                    Expr::col(Column::Data),
                    sea_orm::sea_query::Alias::new("text"),
                ))
                .like(search_pattern),
            );
        }
    }

    // Get total count
    let total = query_builder.clone().count(db).await?;

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
                query_builder = query_builder.order_by(Column::Id, order);
            }
            "publish" => {
                query_builder = query_builder.order_by(Column::Publish, order);
            }
            "lock" => {
                query_builder = query_builder.order_by(Column::Lock, order);
            }
            "hide" => {
                query_builder = query_builder.order_by(Column::Hide, order);
            }
            "created_at" => {
                query_builder = query_builder.order_by(Column::CreatedAt, order);
            }
            "order" | "order_id" => {
                query_builder = query_builder.order_by(Column::OrderId, order);
            }
            _ => {
                query_builder = query_builder
                    .order_by_asc(Column::OrderId)
                    .order_by_desc(Column::CreatedAt);
            }
        }
    } else {
        query_builder = query_builder
            .order_by_asc(Column::OrderId)
            .order_by_desc(Column::CreatedAt);
    }

    // Get paginated terms
    let terms = query_builder.offset(offset).limit(limit).all(db).await?;

    // Batch queries for N+1 fix
    // 1. Get children counts in one query
    let term_ids: Vec<i64> = terms.iter().map(|t| t.id).collect();
    let children_counts: std::collections::HashMap<i64, i64> = if !term_ids.is_empty() {
        let all_children = Term::find()
            .filter(Column::ParentId.is_in(term_ids.clone()))
            .all(db)
            .await
            .unwrap_or_default();

        let mut map = std::collections::HashMap::new();
        for child in all_children {
            if let Some(parent_id) = child.parent_id {
                *map.entry(parent_id).or_insert(0) += 1;
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    // 2. Get parent titles in one query
    let parent_ids: Vec<i64> = terms.iter().filter_map(|t| t.parent_id).collect();
    let parent_titles: std::collections::HashMap<i64, String> = if !parent_ids.is_empty() {
        Term::find()
            .filter(Column::Id.is_in(parent_ids))
            .all(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.id, p.get_title(lang)))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Build response with children count and parent title
    let mut response_terms = Vec::new();
    for term in terms {
        // Get from pre-fetched maps
        let children_count = children_counts.get(&term.id).copied().unwrap_or(0);
        let parent_title = term
            .parent_id
            .and_then(|pid| parent_titles.get(&pid).cloned());

        response_terms.push(term.to_response_with_meta(lang, children_count, parent_title));
    }

    // Build breadcrumbs
    let breadcrumbs = if let Some(parent_id_str) = &parent_id {
        if parent_id_str != "null" {
            if let Ok(pid) = parent_id_str.parse::<i64>() {
                build_breadcrumb_path(db, pid, vocabulary_id, lang).await
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Ok((response_terms, total, breadcrumbs))
}

/// Build breadcrumb path for hierarchical navigation
async fn build_breadcrumb_path(
    db: &DatabaseConnection,
    term_id: i64,
    vocabulary_id: i64,
    lang: &str,
) -> Vec<BreadcrumbItem> {
    let mut breadcrumbs = Vec::new();
    let mut current_id = Some(term_id);

    while let Some(id) = current_id {
        if let Ok(Some(term)) = Term::find_by_id(id).one(db).await {
            let children_count = Term::find()
                .filter(Column::ParentId.eq(term.id))
                .count(db)
                .await
                .unwrap_or(0);

            breadcrumbs.insert(
                0,
                BreadcrumbItem {
                    id: term.id,
                    title: term.get_title(lang),
                    url: format!(
                        "/admin/taxonomy/vocabularies/{}/terms?parent_id={}",
                        vocabulary_id, term.id
                    ),
                    has_children: children_count > 0,
                    children_count: children_count as i64,
                },
            );

            current_id = term.parent_id;
        } else {
            break;
        }
    }

    breadcrumbs
}

/// Load menu items for frontend (optimized with bulk queries)
pub async fn load_menu_items(db: &DatabaseConnection, lang: &str) -> Vec<serde_json::Value> {
    use crate::modules::content::models::Content;
    use serde_json::json;
    use std::collections::HashMap;

    // Settings'ten navbar menü vocabulary ID'sini al
    let vocab_id =
        crate::modules::admin::services::settings_service::get_vocab_id(db, "navbar_menu")
            .await
            .unwrap_or(2); // Fallback olarak 2 kullan

    // Vocabulary ID için term'leri çek
    let menu_terms = Term::find()
        .filter(Column::VocabularyId.eq(vocab_id))
        .filter(Column::Publish.eq(true))
        .filter(Column::ParentId.is_null()) // Sadece root
        .order_by_asc(Column::OrderId)
        .order_by_asc(Column::Id)
        .all(db)
        .await
        .unwrap_or_default();

    // Tüm content_id'leri topla (N+1 query'yi önlemek için)
    let content_ids: Vec<i64> = menu_terms
        .iter()
        .filter_map(|term| term.data.get("content_id").and_then(|v| v.as_i64()))
        .collect();

    // Tek sorguda tüm content'leri çek
    let contents = if !content_ids.is_empty() {
        Content::find()
            .filter(crate::modules::content::models::content::Column::Id.is_in(content_ids))
            .all(db)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Content'leri ID'ye göre map'e çevir
    let content_map: HashMap<i64, _> = contents.into_iter().map(|c| (c.id, c)).collect();

    // Her term için URL oluştur
    let mut menu_items = Vec::new();
    for term in menu_terms {
        let title = term
            .data
            .get("langs")
            .and_then(|langs| langs.get(lang))
            .and_then(|lang_data| lang_data.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");

        // URL oluştur (manuel URL veya content URL)
        let url = if let Some(menu_url) = term
            .data
            .get("langs")
            .and_then(|langs| langs.get(lang))
            .and_then(|lang_data| lang_data.get("menu_url"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            // Manuel URL varsa onu kullan
            menu_url.to_string()
        } else if let Some(content_id) = term.data.get("content_id").and_then(|v| v.as_i64()) {
            // Content URL'ini map'ten al (DB sorgusu yok!)
            content_map
                .get(&content_id)
                .and_then(|c| c.get_absolute_url(lang))
                .unwrap_or_else(|| "#".to_string())
        } else {
            "#".to_string()
        };

        menu_items.push(json!({
            "id": term.id,
            "title": title,
            "url": url
        }));
    }

    menu_items
}

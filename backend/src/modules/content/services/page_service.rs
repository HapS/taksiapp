// Page Service - Business logic for page operations
use crate::modules::content::helpers::page_helper::*;
use crate::modules::content::models::content::Column as ContentColumn;
use crate::modules::content::models::Content;
use sea_orm::*;

#[derive(Debug)]
pub enum ServiceError {
    NotFound,
    #[allow(dead_code)]
    DatabaseError(DbErr),
}

impl From<DbErr> for ServiceError {
    fn from(err: DbErr) -> Self {
        ServiceError::DatabaseError(err)
    }
}

pub async fn get_page(
    db: &DatabaseConnection,
    lang: &str,
    slug: Option<&str>,
    id: Option<i64>,
) -> Result<PageResponse, ServiceError> {
    let filter = Content::find()
        .filter(ContentColumn::Id.eq(id.unwrap_or(0)))
        // .filter(ContentColumn::ContentType.eq("page"))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null());

    let model = match (id, slug) {
        (Some(id), _) => filter.filter(ContentColumn::Id.eq(id)).one(db).await?,
        (_, Some(slug)) => filter.all(db).await?.into_iter().find(|p| {
            let page_slug = get_string_from_json(&p.data, lang, "slug").unwrap_or_default();
            page_slug == slug
        }),
        _ => None,
    }
    .ok_or(ServiceError::NotFound)?;

    Ok(to_page_response(&model, lang, db).await)
}

pub async fn list_pages(
    db: &DatabaseConnection,
    lang: &str,
) -> Result<Vec<PageResponse>, ServiceError> {
    let items = Content::find()
        // .filter(ContentColumn::ContentType.eq("page"))
        .filter(ContentColumn::Publish.eq(true))
        .filter(ContentColumn::DeletedAt.is_null())
        //exclude content_type filter product
        .filter(ContentColumn::ContentType.ne("product"))
        .order_by_asc(ContentColumn::OrderId)
        .all(db)
        .await?;

    // Pre-fetch all tags for all pages in a single batch query (N+1 fix)
    let content_ids: Vec<i64> = items.iter().map(|p| p.id).collect();
    let tags_map = if !content_ids.is_empty() {
        fetch_tags_for_contents(db, &content_ids, lang)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    let mut results = Vec::new();
    for page in items.iter() {
        if has_content_in_language(&page.data, lang) {
            results.push(to_page_response_with_tags(page, lang, &tags_map).await);
        }
    }

    Ok(results)
}

/// Fetch multiple pages by ids including unpublished/deleted ones.
///
/// This variant is intended for use by bookmarks (or other admin/debug views)
/// where we want to show the original content information even if the page
/// is not published or has a `deleted_at` timestamp. It still excludes
/// `product` content_type and otherwise behaves like `get_pages_map_by_ids`.
pub async fn get_pages_map_by_ids_including_unpublished(
    db: &sea_orm::DatabaseConnection,
    lang: &str,
    ids: &[i64],
) -> Result<
    std::collections::HashMap<i64, crate::modules::content::helpers::page_helper::PageResponse>,
    ServiceError,
> {
    use crate::modules::content::helpers::page_helper as helper;

    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    // Fetch contents without publish/deleted filters but still exclude products.
    let items = Content::find()
        .filter(ContentColumn::ContentType.ne("product"))
        .filter(ContentColumn::Id.is_in(ids.to_vec()))
        .all(db)
        .await?;

    // Pre-fetch tags for these contents (if any)
    let content_ids: Vec<i64> = items.iter().map(|p| p.id).collect();
    let tags_map = if !content_ids.is_empty() {
        helper::fetch_tags_for_contents(db, &content_ids, lang)
            .await
            .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };

    // Build PageResponse for each content and return a map id -> PageResponse
    let mut results: std::collections::HashMap<i64, helper::PageResponse> =
        std::collections::HashMap::new();

    for content in items.into_iter() {
        // For bookmarks we want to include pages even if they are not published.
        // to_page_response_with_tags will attempt to extract language-specific
        // fields and fall back as appropriate.
        let resp = helper::to_page_response_with_tags(&content, lang, &tags_map).await;
        results.insert(resp.id, resp);
    }

    Ok(results)
}

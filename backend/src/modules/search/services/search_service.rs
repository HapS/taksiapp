use crate::modules::content::models::content::Model as ContentModel;
use crate::modules::content::models::content::{self, Entity as Content};
use sea_orm::sea_query::Expr;
use sea_orm::*;

pub struct SearchService;

impl SearchService {
    /// Perform search on contents
    pub async fn search_content(
        db: &DatabaseConnection,
        query: &str,
        content_type: Option<String>,
        lang: &str,
    ) -> Result<Vec<ContentModel>, DbErr> {
        let q_lower = query.to_lowercase();
        let like_query = format!("%{}%", q_lower);

        let mut select = Content::find()
            .filter(content::Column::Publish.eq(true))
            .filter(content::Column::DeletedAt.is_null());

        if let Some(ctype) = content_type {
            if !ctype.is_empty() && ctype != "all" {
                select = select.filter(content::Column::ContentType.eq(ctype));
            } else {
                select = select.filter(content::Column::ContentType.is_in(vec!["product"]));
            }
        }

        // Filter by query
        select = select.filter(
            Condition::any()
                // Search in title (localized)
                .add(Expr::cust_with_values(
                    "lower(data->'langs'->$1->>'title') LIKE $2",
                    vec![Value::from(lang), Value::from(like_query.clone())],
                ))
                // Search in description (localized) (assuming 'description' or 'body' keys)
                .add(Expr::cust_with_values(
                    "lower(data->'langs'->$1->>'description') LIKE $2",
                    vec![Value::from(lang), Value::from(like_query.clone())],
                )),
        );

        // Sort by CreatedAt desc
        select = select.order_by_desc(content::Column::CreatedAt);

        // Limit results
        select.limit(20).all(db).await
    }
}

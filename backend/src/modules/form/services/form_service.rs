use crate::modules::content::models::content::Column as ContentColumn;
use crate::modules::content::models::content::Entity as ContentEntity;
use crate::modules::form::dto::form_dto::*;
use crate::modules::form::models::form_submission;
use crate::modules::form::models::form_submission::Entity as FormSubmissionEntity;
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use sea_orm::ColumnTrait;
use sea_orm::*;

pub struct FormService;

#[derive(Debug)]
#[allow(dead_code)]
pub enum FormError {
    NotFound,
    DatabaseError(String),
}

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormError::NotFound => write!(f, "Form not found"),
            FormError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for FormError {}

impl From<DbErr> for FormError {
    fn from(err: DbErr) -> Self {
        FormError::DatabaseError(err.to_string())
    }
}

impl FormService {
    /// Form listesi (pagination ile) - content tablosu ile left join
    pub async fn list_form_data(
        db: &DatabaseConnection,
        page: u64,
        per_page: u64,
        search: Option<String>,
        form_id: Option<i64>,
        start_date: Option<String>,
        end_date: Option<String>,
    ) -> Result<(Vec<FormListResponse>, u64), FormError> {
        // Build the base query with filters
        let base_query = FormSubmissionEntity::find()
            .find_also_related(ContentEntity)
            .filter(ContentColumn::ContentType.eq("form"));

        // Apply filters
        let mut filtered_query = base_query.clone();

        if let Some(form_id) = form_id {
            filtered_query = filtered_query.filter(form_submission::Column::FormId.eq(form_id));
        }

        if let Some(search) = search {
            let escaped = search.replace('\'', "''");
            let condition = format!("form_submissions.data::text ILIKE '%{}%'", escaped);
            filtered_query = filtered_query.filter(sea_orm::sea_query::Expr::cust(&condition));
        }

        if let Some(ref start_date) = start_date {
            if let Ok(date) = NaiveDate::parse_from_str(start_date, "%Y-%m-%d") {
                let start_datetime = Utc
                    .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
                    .single();
                if let Some(dt) = start_datetime {
                    filtered_query =
                        filtered_query.filter(form_submission::Column::CreatedAt.gte(dt));
                }
            }
        }

        if let Some(ref end_date) = end_date {
            if let Ok(date) = NaiveDate::parse_from_str(end_date, "%Y-%m-%d") {
                let end_datetime = Utc
                    .with_ymd_and_hms(date.year(), date.month(), date.day(), 23, 59, 59)
                    .single();
                if let Some(dt) = end_datetime {
                    filtered_query =
                        filtered_query.filter(form_submission::Column::CreatedAt.lte(dt));
                }
            }
        }

        // Get total count without pagination
        let total = FormSubmissionEntity::find()
            .join(JoinType::LeftJoin, form_submission::Relation::Form.def())
            .filter(ContentColumn::ContentType.eq("form"))
            .apply_if(form_id, |q, v| {
                q.filter(form_submission::Column::FormId.eq(v))
            })
            .apply_if(start_date.as_ref(), |q, v| {
                if let Ok(date) = NaiveDate::parse_from_str(v, "%Y-%m-%d") {
                    let dt = Utc
                        .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
                        .single();
                    if let Some(dt) = dt {
                        return q.filter(form_submission::Column::CreatedAt.gte(dt));
                    }
                }
                q
            })
            .apply_if(end_date.as_ref(), |q, v| {
                if let Ok(date) = NaiveDate::parse_from_str(v, "%Y-%m-%d") {
                    let dt = Utc
                        .with_ymd_and_hms(date.year(), date.month(), date.day(), 23, 59, 59)
                        .single();
                    if let Some(dt) = dt {
                        return q.filter(form_submission::Column::CreatedAt.lte(dt));
                    }
                }
                q
            })
            .count(db)
            .await?;

        // Fetch paginated results with join
        let forms = filtered_query
            .order_by_desc(<form_submission::Entity as sea_orm::EntityTrait>::Column::CreatedAt)
            .offset(((page - 1) * per_page) as u64)
            .limit(per_page)
            .all(db)
            .await?;

        let mut results = Vec::new();
        for (form, content) in forms {
            let content_data = content.map(|c| {
                let title = c
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|langs| {
                        langs
                            .values()
                            .next()
                            .and_then(|lang_data| lang_data.get("title"))
                            .and_then(|t| t.as_str())
                    })
                    .map(|s| s.to_string());

                let slug = c
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|langs| {
                        langs
                            .values()
                            .next()
                            .and_then(|lang_data| lang_data.get("slug"))
                            .and_then(|s| s.as_str())
                    })
                    .map(|s| s.to_string());

                FormContentData {
                    id: c.id,
                    title,
                    slug,
                    content_type: c.content_type,
                    publish: c.publish,
                }
            });

            results.push(FormListResponse {
                id: form.id,
                form_id: form.form_id,
                created_at: form.created_at.map(|d| d.to_string()),
                content: content_data,
            });
        }

        Ok((results, total))
    }

    /// Form detayını getir - content tablosu ile left join
    pub async fn get_form_data_by_id(
        db: &DatabaseConnection,
        id: i64,
    ) -> Result<Option<FormResponse>, FormError> {
        let result = FormSubmissionEntity::find_by_id(id)
            .find_also_related(ContentEntity)
            .filter(ContentColumn::ContentType.eq("form"))
            .one(db)
            .await?;

        if let Some((form, content)) = result {
            let content_data = content.map(|c| {
                let title = c
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|langs| {
                        langs
                            .values()
                            .next()
                            .and_then(|lang_data| lang_data.get("title"))
                            .and_then(|t| t.as_str())
                    })
                    .map(|s| s.to_string());

                let slug = c
                    .data
                    .get("langs")
                    .and_then(|langs| langs.as_object())
                    .and_then(|langs| {
                        langs
                            .values()
                            .next()
                            .and_then(|lang_data| lang_data.get("slug"))
                            .and_then(|s| s.as_str())
                    })
                    .map(|s| s.to_string());

                FormContentData {
                    id: c.id,
                    title,
                    slug,
                    content_type: c.content_type,
                    publish: c.publish,
                }
            });

            let created_at = form.created_at.map(|dt| dt.to_string());

            Ok(Some(FormResponse {
                id: form.id,
                form_id: form.form_id,
                data: form.data,
                ip: form.ip,
                user_id: form.user_id,
                created_at,
                content: content_data,
            }))
        } else {
            Ok(None)
        }
    }
}

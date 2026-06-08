use crate::modules::comment::entities::comments::{
    self, Entity as Comments, Model as CommentModel,
};
use anyhow::Result;
use sea_orm::*;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommentResponse {
    pub id: i64,
    pub user_id: i64,
    pub content_type: String,
    pub content_id: i64,
    pub content: String,
    pub star: i32,
    pub publish: bool,
    pub ip_address: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub user_name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PaginatedCommentsResponse {
    pub data: Vec<CommentResponse>,
    pub pagination: PaginationMeta,
    pub stats: CommentStats,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommentStats {
    pub total: u64,
    pub average_star: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PaginationMeta {
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}

pub struct CommentService;

impl CommentService {
    pub async fn list_comments(
        db: &DatabaseConnection,
        lang: &str,
        content_type: &str,
        content_id: i64,
        published_only: bool,
        page: u64,
        per_page: u64,
    ) -> Result<PaginatedCommentsResponse> {
        let mut base_query = Comments::find()
            .filter(comments::Column::Lang.eq(lang))
            .filter(comments::Column::ContentType.eq(content_type))
            .filter(comments::Column::ContentId.eq(content_id))
            .filter(comments::Column::DeletedAt.is_null());

        if published_only {
            base_query = base_query.filter(comments::Column::Publish.eq(true));
        }

        let total = base_query.clone().count(db).await?;

        let offset = (page.saturating_sub(1)) * per_page;
        let total_pages = if total == 0 {
            1
        } else {
            (total + per_page - 1) / per_page
        };

        let comments = base_query
            .order_by_desc(comments::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(db)
            .await?;

        let mut results = Vec::with_capacity(comments.len());
        for comment in comments {
            let user_name =
                crate::modules::auth::services::auth_service::get_user_by_id(db, comment.user_id)
                    .await
                    .ok()
                    .and_then(|u| u.first_name.clone().or(Some(u.email)));

            results.push(CommentResponse {
                id: comment.id,
                user_id: comment.user_id,
                content_type: comment.content_type,
                content_id: comment.content_id,
                content: comment.content,
                star: comment.star,
                publish: comment.publish,
                ip_address: comment.ip_address,
                created_at: Some(comment.created_at),
                updated_at: Some(comment.updated_at),
                user_name,
            });
        }

        Ok(PaginatedCommentsResponse {
            data: results,
            pagination: PaginationMeta {
                total,
                page,
                per_page,
                total_pages,
            },
            stats: CommentStats {
                total,
                average_star: Self::get_average_star(db, content_type, content_id)
                    .await
                    .ok()
                    .flatten(),
            },
        })
    }

    pub async fn get_comment(db: &DatabaseConnection, id: i64) -> Result<Option<CommentResponse>> {
        let comment = Comments::find_by_id(id)
            .filter(comments::Column::DeletedAt.is_null())
            .one(db)
            .await?;

        match comment {
            Some(c) => {
                let user_name =
                    crate::modules::auth::services::auth_service::get_user_by_id(db, c.user_id)
                        .await
                        .ok()
                        .and_then(|u| u.first_name.clone().or(Some(u.email)));

                Ok(Some(CommentResponse {
                    id: c.id,
                    user_id: c.user_id,
                    content_type: c.content_type,
                    content_id: c.content_id,
                    content: c.content,
                    star: c.star,
                    publish: c.publish,
                    ip_address: c.ip_address,
                    created_at: Some(c.created_at),
                    updated_at: Some(c.updated_at),
                    user_name,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn create_comment(
        db: &DatabaseConnection,
        user_id: i64,
        lang: String,
        content_type: String,
        content_id: i64,
        content: String,
        star: i32,
        ip_address: Option<String>,
    ) -> Result<CommentModel> {
        let five_minutes_ago = chrono::Utc::now() - chrono::Duration::minutes(5);

        let recent_comment = Comments::find()
            .filter(comments::Column::UserId.eq(user_id))
            .filter(comments::Column::ContentType.eq(&content_type))
            .filter(comments::Column::ContentId.eq(content_id))
            .filter(comments::Column::DeletedAt.is_null())
            .filter(comments::Column::CreatedAt.gt(five_minutes_ago))
            .one(db)
            .await?;

        if recent_comment.is_some() {
            return Err(anyhow::anyhow!(
                "Aynı içerik için 5 dakikada sadece bir yorum yapabilirsiniz."
            ));
        }

        let star = star.clamp(1, 5);

        let mut new_comment = comments::ActiveModel {
            user_id: Set(user_id),
            lang: Set(lang),
            content_type: Set(content_type),
            content_id: Set(content_id),
            content: Set(content),
            star: Set(star),
            publish: Set(true),
            ..Default::default()
        };

        if let Some(ip) = ip_address {
            new_comment.ip_address = Set(Some(ip));
        }

        let result = new_comment.insert(db).await?;
        Ok(result)
    }

    // pub async fn update_comment(
    //     db: &DatabaseConnection,
    //     id: i64,
    //     user_id: i64,
    //     content: String,
    //     star: Option<i32>,
    // ) -> Result<CommentModel> {
    //     let comment = Comments::find_by_id(id)
    //         .filter(comments::Column::DeletedAt.is_null())
    //         .one(db)
    //         .await?
    //         .ok_or_else(|| anyhow::anyhow!("Yorum bulunamadı."))?;

    //     if comment.user_id != user_id {
    //         return Err(anyhow::anyhow!("Bu yorumu güncelleme yetkiniz yok."));
    //     }

    //     let mut active_model: comments::ActiveModel = comment.into();
    //     active_model.content = Set(content);
    //     active_model.updated_at = Set(chrono::Utc::now());

    //     if let Some(s) = star {
    //         let clamped: i32 = if s < 1i32 { 1 } else if s > 5i32 { 5 } else { s };
    //         active_model.star = Set(clamped);
    //     }

    //     let result = active_model.update(db).await?;
    //     Ok(result)
    // }

    pub async fn delete_comment(db: &DatabaseConnection, id: i64, user_id: i64) -> Result<()> {
        let comment = Comments::find_by_id(id)
            .filter(comments::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Yorum bulunamadı."))?;

        if comment.user_id != user_id {
            return Err(anyhow::anyhow!("Bu yorumu silme yetkiniz yok."));
        }

        let mut active_model: comments::ActiveModel = comment.into();
        active_model.deleted_at = Set(Some(chrono::Utc::now()));
        active_model.update(db).await?;

        Ok(())
    }

    // #[allow(dead_code)]
    pub async fn get_average_star(
        db: &DatabaseConnection,
        content_type: &str,
        content_id: i64,
    ) -> Result<Option<f64>> {
        let comments = Comments::find()
            .filter(comments::Column::ContentType.eq(content_type))
            .filter(comments::Column::ContentId.eq(content_id))
            .filter(comments::Column::DeletedAt.is_null())
            .filter(comments::Column::Publish.eq(true))
            .all(db)
            .await?;

        if comments.is_empty() {
            return Ok(None);
        }

        let sum: i64 = comments.iter().map(|c| c.star as i64).sum();
        let count = comments.len() as f64;
        Ok(Some(sum as f64 / count))
    }

    pub async fn admin_list_comments(
        db: &DatabaseConnection,
        lang: &str,
        content_type: Option<&str>,
        content_id: Option<i64>,
        search: Option<&str>,
        start_date: Option<&str>,
        end_date: Option<&str>,
        include_unpublished: bool,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<CommentResponse>, u64)> {
        let mut query = Comments::find()
            .filter(comments::Column::Lang.eq(lang))
            .filter(comments::Column::DeletedAt.is_null());

        if let Some(ct) = content_type {
            query = query.filter(comments::Column::ContentType.eq(ct));
        }

        if let Some(cid) = content_id {
            query = query.filter(comments::Column::ContentId.eq(cid));
        }

        if let Some(s) = search {
            if !s.is_empty() {
                query = query.filter(comments::Column::Content.contains(s));
            }
        }

        if let Some(start) = start_date {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(start) {
                query =
                    query.filter(comments::Column::CreatedAt.gte(dt.with_timezone(&chrono::Utc)));
            }
        }

        if let Some(end) = end_date {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(end) {
                query =
                    query.filter(comments::Column::CreatedAt.lte(dt.with_timezone(&chrono::Utc)));
            }
        }

        if !include_unpublished {
            query = query.filter(comments::Column::Publish.eq(true));
        }

        let total = query.clone().count(db).await?;
        let offset = (page.saturating_sub(1)) * per_page;

        let comments = query
            .order_by_desc(comments::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(db)
            .await?;

        let mut results = Vec::with_capacity(comments.len());
        for comment in comments {
            let user_name =
                crate::modules::auth::services::auth_service::get_user_by_id(db, comment.user_id)
                    .await
                    .ok()
                    .and_then(|u| u.first_name.clone().or(Some(u.email)));

            results.push(CommentResponse {
                id: comment.id,
                user_id: comment.user_id,
                content_type: comment.content_type,
                content_id: comment.content_id,
                content: comment.content,
                star: comment.star,
                publish: comment.publish,
                ip_address: comment.ip_address,
                created_at: Some(comment.created_at),
                updated_at: Some(comment.updated_at),
                user_name,
            });
        }

        Ok((results, total))
    }

    pub async fn admin_delete_comment(db: &DatabaseConnection, id: i64) -> Result<()> {
        let comment = Comments::find_by_id(id)
            .filter(comments::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Yorum bulunamadı."))?;

        let mut active_model: comments::ActiveModel = comment.into();
        active_model.deleted_at = Set(Some(chrono::Utc::now()));
        active_model.update(db).await?;

        Ok(())
    }

    pub async fn admin_toggle_publish(db: &DatabaseConnection, id: i64) -> Result<CommentResponse> {
        let comment = Comments::find_by_id(id)
            .filter(comments::Column::DeletedAt.is_null())
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Yorum bulunamadı."))?;

        let new_publish_status = !comment.publish;

        let mut active_model: comments::ActiveModel = comment.clone().into();
        active_model.publish = Set(new_publish_status);
        active_model.updated_at = Set(chrono::Utc::now());
        active_model.update(db).await?;

        let user_name =
            crate::modules::auth::services::auth_service::get_user_by_id(db, comment.user_id)
                .await
                .ok()
                .and_then(|u| u.first_name.clone().or(Some(u.email)));

        Ok(CommentResponse {
            id: comment.id,
            user_id: comment.user_id,
            content_type: comment.content_type,
            content_id: comment.content_id,
            content: comment.content,
            star: comment.star,
            publish: new_publish_status,
            ip_address: comment.ip_address,
            created_at: Some(comment.created_at),
            updated_at: Some(comment.updated_at),
            user_name,
        })
    }
}

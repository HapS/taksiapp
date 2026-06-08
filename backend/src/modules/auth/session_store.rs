use async_trait::async_trait;
use sea_orm::*;
use std::collections::HashMap;
use tower_sessions::{
    session::{Id, Record},
    SessionStore,
};

use crate::modules::auth::models::session::{self, Entity as Session};

#[derive(Debug, Clone)]
pub struct SeaOrmSessionStore {
    db: DatabaseConnection,
}

impl SeaOrmSessionStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SessionStore for SeaOrmSessionStore {
    async fn save(&self, record: &Record) -> Result<(), tower_sessions::session_store::Error> {
        let session_data = serde_json::to_value(&record.data)
            .map_err(|e| tower_sessions::session_store::Error::Encode(e.to_string().into()))?;

        let now = chrono::Utc::now();

        // Convert OffsetDateTime to chrono DateTime
        let expiry_chrono =
            chrono::DateTime::from_timestamp(record.expiry_date.unix_timestamp(), 0).ok_or_else(
                || tower_sessions::session_store::Error::Encode("Invalid timestamp".into()),
            )?;

        // Session data'dan user_id'yi al
        let user_id = record.data.get("user_id").and_then(|v| v.as_i64());

        let active_model = session::ActiveModel {
            id: Set(record.id.to_string()),
            user_id: Set(user_id),
            data: Set(session_data),
            expires_at: Set(expiry_chrono.into()),
            created_at: Set(Some(now.into())),
            updated_at: Set(Some(now.into())),
        };

        Session::insert(active_model)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(session::Column::Id)
                    .update_columns([
                        session::Column::UserId,
                        session::Column::Data,
                        session::Column::ExpiresAt,
                        session::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await
            .map_err(|e| tower_sessions::session_store::Error::Encode(e.to_string().into()))?;

        Ok(())
    }

    async fn load(
        &self,
        session_id: &Id,
    ) -> Result<Option<Record>, tower_sessions::session_store::Error> {
        let session = Session::find()
            .filter(session::Column::Id.eq(session_id.to_string()))
            .filter(session::Column::ExpiresAt.gt(chrono::Utc::now()))
            .one(&self.db)
            .await
            .map_err(|e| tower_sessions::session_store::Error::Decode(e.to_string().into()))?;

        match session {
            Some(s) => {
                let data: HashMap<String, serde_json::Value> =
                    serde_json::from_value(s.data.clone()).map_err(|e| {
                        tower_sessions::session_store::Error::Decode(e.to_string().into())
                    })?;

                // Convert chrono DateTime to OffsetDateTime
                let expiry_offset = time::OffsetDateTime::from_unix_timestamp(
                    s.expires_at.timestamp(),
                )
                .map_err(|e| tower_sessions::session_store::Error::Decode(e.to_string().into()))?;

                Ok(Some(Record {
                    id: session_id.clone(),
                    data,
                    expiry_date: expiry_offset,
                }))
            }
            None => Ok(None),
        }
    }

    async fn delete(&self, session_id: &Id) -> Result<(), tower_sessions::session_store::Error> {
        Session::delete_many()
            .filter(session::Column::Id.eq(session_id.to_string()))
            .exec(&self.db)
            .await
            .map_err(|e| tower_sessions::session_store::Error::Encode(e.to_string().into()))?;

        Ok(())
    }
}

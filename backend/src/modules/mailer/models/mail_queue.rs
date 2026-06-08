use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Mail Queue - Mail kuyruğu
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mail_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    
    /// Template adı (NULL olabilir custom mail için)
    pub template_name: Option<String>,
    
    /// Alıcı email
    pub to_email: String,
    
    /// Alıcı adı
    pub to_name: Option<String>,
    
    /// Mail konusu (rendered)
    pub subject: String,
    
    /// Mail içeriği (rendered HTML)
    pub body: String,
    
    /// Template değişkenleri
    pub variables: Option<Json>,
    
    /// Hangi dilde gönderilecek
    pub language: Option<String>,
    
    /// Mail durumu: pending, sent, failed, retry
    pub status: Option<String>,
    
    /// Deneme sayısı
    pub attempts: Option<i32>,
    
    /// Maksimum deneme sayısı
    pub max_attempts: Option<i32>,
    
    /// Hata mesajı
    pub error_message: Option<String>,
    
    /// Zamanlanmış gönderim tarihi
    pub scheduled_at: Option<DateTimeWithTimeZone>,
    
    /// Gönderilme tarihi
    pub sent_at: Option<DateTimeWithTimeZone>,
    
    /// Timestamps
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Maksimum deneme sayısına ulaştı mı?
    pub fn has_reached_max_attempts(&self) -> bool {
        let attempts = self.attempts.unwrap_or(0);
        let max_attempts = self.max_attempts.unwrap_or(3);
        attempts >= max_attempts
    }
}


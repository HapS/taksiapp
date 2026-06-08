use crate::modules::admin::services::settings_service;
use crate::modules::mailer::models::{MailQueue, MailQueueModel};
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::{authentication::Credentials, response::Response},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use sea_orm::*;

#[derive(Debug)]
pub enum MailServiceError {
    DatabaseError(DbErr),
    SmtpError(String),
    SettingsError(String),
    MessageError(String),
}

impl std::fmt::Display for MailServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MailServiceError::DatabaseError(err) => write!(f, "Veritabanı hatası: {}", err),
            MailServiceError::SmtpError(err) => write!(f, "SMTP error: {}", err),
            MailServiceError::SettingsError(err) => write!(f, "Settings error: {}", err),
            MailServiceError::MessageError(err) => write!(f, "Message error: {}", err),
        }
    }
}

impl std::error::Error for MailServiceError {}

impl From<DbErr> for MailServiceError {
    fn from(err: DbErr) -> Self {
        MailServiceError::DatabaseError(err)
    }
}

impl From<lettre::error::Error> for MailServiceError {
    fn from(err: lettre::error::Error) -> Self {
        MailServiceError::SmtpError(format!("{:?}", err))
    }
}

impl From<lettre::transport::smtp::Error> for MailServiceError {
    fn from(err: lettre::transport::smtp::Error) -> Self {
        MailServiceError::SmtpError(format!("{:?}", err))
    }
}

impl From<lettre::address::AddressError> for MailServiceError {
    fn from(err: lettre::address::AddressError) -> Self {
        MailServiceError::MessageError(format!("{:?}", err))
    }
}

/// Mail gönderme servisi
pub struct MailService {
    db: DatabaseConnection,
}

impl MailService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Kuyruktaki mailleri işle
    pub async fn process_queue(&self) -> Result<u32, MailServiceError> {
        // Gönderime hazır mailleri al
        let pending_mails = MailQueue::find()
            .filter(
                Condition::any()
                    .add(crate::modules::mailer::models::mail_queue::Column::Status.eq("pending"))
                    .add(crate::modules::mailer::models::mail_queue::Column::Status.eq("retry")),
            )
            .filter(
                Condition::any()
                    .add(crate::modules::mailer::models::mail_queue::Column::ScheduledAt.is_null())
                    .add(
                        crate::modules::mailer::models::mail_queue::Column::ScheduledAt
                            .lte(chrono::Utc::now()),
                    ),
            )
            .order_by_asc(crate::modules::mailer::models::mail_queue::Column::CreatedAt)
            .limit(50) // Batch size
            .all(&self.db)
            .await?;

        let mut processed = 0;

        for mail in pending_mails {
            if mail.has_reached_max_attempts() {
                // Maksimum deneme sayısına ulaştı, failed olarak işaretle
                self.mark_as_failed(mail.id, "Maximum attempts reached".to_string())
                    .await?;
                continue;
            }

            match self.send_mail(&mail).await {
                Ok(_) => {
                    self.mark_as_sent(mail.id).await?;
                    processed += 1;
                }
                Err(e) => {
                    let error_msg = format!("{:?}", e);
                    self.increment_attempts(mail.id, error_msg).await?;
                }
            }
        }

        Ok(processed)
    }

    /// Tek bir mail gönder
    async fn send_mail(&self, mail: &MailQueueModel) -> Result<(), MailServiceError> {
        // Settings'ten SMTP ayarlarını al
        let settings = settings_service::get_settings(&self.db)
            .await
            .map_err(|e| MailServiceError::SettingsError(format!("{:?}", e)))?;

        let smtp_host = settings.smtp_host.ok_or_else(|| {
            MailServiceError::SettingsError("SMTP host not configured".to_string())
        })?;
        let smtp_port = settings.smtp_port.unwrap_or(587);
        let smtp_username = settings.smtp_username.ok_or_else(|| {
            MailServiceError::SettingsError("SMTP username not configured".to_string())
        })?;
        let smtp_password = settings.smtp_password.ok_or_else(|| {
            MailServiceError::SettingsError("SMTP password not configured".to_string())
        })?;
        let smtp_from_email = settings.smtp_from_email.ok_or_else(|| {
            MailServiceError::SettingsError("SMTP from email not configured".to_string())
        })?;
        let smtp_from_name = settings.smtp_from_name.unwrap_or("Backend-RS".to_string());

        // From mailbox oluştur
        let from_mailbox: Mailbox = format!("{} <{}>", smtp_from_name, smtp_from_email)
            .parse()
            .map_err(|e| {
                MailServiceError::MessageError(format!("Invalid from address: {:?}", e))
            })?;

        // To mailbox oluştur
        let to_mailbox: Mailbox = if let Some(to_name) = &mail.to_name {
            format!("{} <{}>", to_name, mail.to_email)
        } else {
            mail.to_email.clone()
        }
        .parse()
        .map_err(|e| MailServiceError::MessageError(format!("Invalid to address: {:?}", e)))?;

        // Mail mesajı oluştur
        let message = Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(&mail.subject)
            .header(ContentType::TEXT_HTML)
            .body(mail.body.clone())
            .map_err(|e| {
                MailServiceError::MessageError(format!("Failed to build message: {:?}", e))
            })?;

        // SMTP transport oluştur
        let creds = Credentials::new(smtp_username, smtp_password);

        let mailer = match settings.smtp_encryption.as_deref().unwrap_or("tls") {
            "ssl" => {
                // SSL/TLS (genellikle port 465)
                AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)?
                    .port(smtp_port)
                    .credentials(creds)
                    .timeout(Some(std::time::Duration::from_secs(30)))
                    .build()
            }
            "tls" => {
                // STARTTLS (genellikle port 587)
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host)?
                    .port(smtp_port)
                    .credentials(creds)
                    .timeout(Some(std::time::Duration::from_secs(30)))
                    .build()
            }
            "none" => {
                // Şifreleme yok (sadece test için)
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp_host)
                    .port(smtp_port)
                    .credentials(creds)
                    .timeout(Some(std::time::Duration::from_secs(30)))
                    .build()
            }
            _ => {
                // Varsayılan olarak TLS kullan
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host)?
                    .port(smtp_port)
                    .credentials(creds)
                    .timeout(Some(std::time::Duration::from_secs(30)))
                    .build()
            }
        };

        // Mail gönder
        let response: Response = mailer.send(message).await?;
        println!("Yanıt: {:?}", response);

        Ok(())
    }

    /// Mail'i gönderildi olarak işaretle
    async fn mark_as_sent(&self, mail_id: i64) -> Result<(), MailServiceError> {
        let mut mail: crate::modules::mailer::models::mail_queue::ActiveModel =
            MailQueue::find_by_id(mail_id)
                .one(&self.db)
                .await?
                .ok_or_else(|| {
                    MailServiceError::DatabaseError(DbErr::RecordNotFound(
                        "Mail not found".to_string(),
                    ))
                })?
                .into();

        mail.status = Set(Some("sent".to_string()));
        mail.sent_at = Set(Some(chrono::Utc::now().into()));
        mail.updated_at = Set(Some(chrono::Utc::now().into()));

        mail.update(&self.db).await?;
        Ok(())
    }

    /// Mail'i başarısız olarak işaretle
    async fn mark_as_failed(
        &self,
        mail_id: i64,
        error_message: String,
    ) -> Result<(), MailServiceError> {
        let mut mail: crate::modules::mailer::models::mail_queue::ActiveModel =
            MailQueue::find_by_id(mail_id)
                .one(&self.db)
                .await?
                .ok_or_else(|| {
                    MailServiceError::DatabaseError(DbErr::RecordNotFound(
                        "Mail not found".to_string(),
                    ))
                })?
                .into();

        mail.status = Set(Some("failed".to_string()));
        mail.error_message = Set(Some(error_message));
        mail.updated_at = Set(Some(chrono::Utc::now().into()));

        mail.update(&self.db).await?;
        Ok(())
    }

    /// Deneme sayısını artır ve retry zamanlaması yap
    async fn increment_attempts(
        &self,
        mail_id: i64,
        error_message: String,
    ) -> Result<(), MailServiceError> {
        let mail_model = MailQueue::find_by_id(mail_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                MailServiceError::DatabaseError(DbErr::RecordNotFound("Mail not found".to_string()))
            })?;

        let current_attempts = mail_model.attempts.unwrap_or(0) + 1;
        let max_attempts = mail_model.max_attempts.unwrap_or(3);

        let mut mail: crate::modules::mailer::models::mail_queue::ActiveModel = mail_model.into();
        mail.attempts = Set(Some(current_attempts));
        mail.error_message = Set(Some(error_message));
        mail.updated_at = Set(Some(chrono::Utc::now().into()));

        if current_attempts >= max_attempts {
            // Maksimum deneme sayısına ulaştı, failed olarak işaretle
            mail.status = Set(Some("failed".to_string()));
        } else {
            // Retry olarak işaretle ve sonraki deneme zamanını belirle
            mail.status = Set(Some("retry".to_string()));

            // Exponential backoff: 5 dakika * 2^(attempts-1)
            let retry_minutes = 5 * (2_i32.pow((current_attempts - 1) as u32));
            let retry_time = chrono::Utc::now() + chrono::Duration::minutes(retry_minutes as i64);
            mail.scheduled_at = Set(Some(retry_time.into()));

            println!(
                "📧 Mail retry scheduled in {} minutes (attempt {}/{})",
                retry_minutes, current_attempts, max_attempts
            );
        }

        mail.update(&self.db).await?;
        Ok(())
    }

    /// Kuyruk istatistikleri
    pub async fn get_queue_stats(&self) -> Result<QueueStats, MailServiceError> {
        let pending = MailQueue::find()
            .filter(crate::modules::mailer::models::mail_queue::Column::Status.eq("pending"))
            .count(&self.db)
            .await?;

        let retry = MailQueue::find()
            .filter(crate::modules::mailer::models::mail_queue::Column::Status.eq("retry"))
            .count(&self.db)
            .await?;

        let sent = MailQueue::find()
            .filter(crate::modules::mailer::models::mail_queue::Column::Status.eq("sent"))
            .count(&self.db)
            .await?;

        let failed = MailQueue::find()
            .filter(crate::modules::mailer::models::mail_queue::Column::Status.eq("failed"))
            .count(&self.db)
            .await?;

        Ok(QueueStats {
            pending,
            retry,
            sent,
            failed,
            total: pending + retry + sent + failed,
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct QueueStats {
    pub pending: u64,
    pub retry: u64,
    pub sent: u64,
    pub failed: u64,
    pub total: u64,
}

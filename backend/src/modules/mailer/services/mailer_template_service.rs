use sea_orm::*;
use std::collections::HashMap;
use tera::{Context, Tera};

#[derive(Debug)]
pub enum TemplateServiceError {
    DatabaseError(DbErr),
    RenderError(String),
}

impl std::fmt::Display for TemplateServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateServiceError::DatabaseError(err) => write!(f, "Veritabanı hatası: {}", err),
            TemplateServiceError::RenderError(err) => write!(f, "Render error: {}", err),
        }
    }
}

impl std::error::Error for TemplateServiceError {}

impl From<DbErr> for TemplateServiceError {
    fn from(err: DbErr) -> Self {
        TemplateServiceError::DatabaseError(err)
    }
}

impl From<tera::Error> for TemplateServiceError {
    fn from(err: tera::Error) -> Self {
        TemplateServiceError::RenderError(err.to_string())
    }
}

/// Template servisi - HTML dosyalarından template okur
pub struct TemplateService {
    db: DatabaseConnection,
    app_state: Option<std::sync::Arc<crate::app_state::AppState>>,
}

impl TemplateService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            app_state: None,
        }
    }

    pub fn with_app_state(
        db: DatabaseConnection,
        app_state: std::sync::Arc<crate::app_state::AppState>,
    ) -> Self {
        Self {
            db,
            app_state: Some(app_state),
        }
    }

    /// HTML template'i render et ve kuyruğa ekle
    pub async fn queue_mail(
        &self,
        template_name: &str,
        to_email: &str,
        to_name: Option<&str>,
        subject: &str,
        variables: HashMap<String, serde_json::Value>,
        language: &str,
        scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<i64, TemplateServiceError> {
        // HTML template'i render et - tema desteği ile
        let body = if let Some(app_state) = &self.app_state {
            // AppState ile tema desteği - mailer path kullan
            let mut context = tera::Context::new();
            for (key, value) in &variables {
                context.insert(key, value);
            }

            // Language değişkenini ekle (mail template'lerinde current_language yerine)
            context.insert("current_language", language);
            context.insert("language", language);

            // Settings'i context'e ekle
            if let Ok(settings) = app_state.settings_cache.read() {
                // Site bilgileri
                if let Some(v) = settings.get("site_name_langs") {
                    context.insert("site_name_langs", &v);
                }
                if let Some(v) = &settings.site_logo {
                    context.insert("site_logo", v);
                }
                if let Some(v) = &settings.site_favicon {
                    context.insert("site_favicon", v);
                }
                // İletişim bilgileri
                if let Some(v) = &settings.contact_email {
                    context.insert("contact_email", v);
                }
                if let Some(v) = &settings.contact_phone {
                    context.insert("contact_phone", v);
                }
                if let Some(v) = &settings.contact_address {
                    context.insert("contact_address", v);
                }
                // Sosyal medya
                if let Some(v) = &settings.social_facebook {
                    context.insert("social_facebook", v);
                }
                if let Some(v) = &settings.social_twitter {
                    context.insert("social_twitter", v);
                }
                if let Some(v) = &settings.social_instagram {
                    context.insert("social_instagram", v);
                }
                if let Some(v) = &settings.social_linkedin {
                    context.insert("social_linkedin", v);
                }
                if let Some(v) = &settings.social_youtube {
                    context.insert("social_youtube", v);
                }

                // Site name - dil bazlı
                if let Some(site_name) = settings.get_site_name(language) {
                    context.insert("site_name", &site_name);
                }
            }

            let template_path_inner = format!("mailer/{}.html", template_name);
            match app_state.render_frontend_template(&template_path_inner, &context) {
                Ok(rendered) => rendered,
                Err(e) => return Err(TemplateServiceError::RenderError(e.to_string())),
            }
        } else {
            // Fallback - base tema kullan, settings'i DB'den çek
            Self::render_html_template_with_db(&self.db, template_name, &variables, language)
                .await?
        };

        // Kuyruğa ekle
        let mail = crate::modules::mailer::models::mail_queue::ActiveModel {
            template_name: Set(Some(template_name.to_string())),
            to_email: Set(to_email.to_string()),
            to_name: Set(to_name.map(|s| s.to_string())),
            subject: Set(subject.to_string()),
            body: Set(body),
            variables: Set(Some(serde_json::to_value(variables).unwrap_or_default())),
            language: Set(Some(language.to_string())),
            status: Set(Some("pending".to_string())),
            attempts: Set(Some(0)),
            max_attempts: Set(Some(3)),
            scheduled_at: Set(scheduled_at.map(|dt| dt.into())),
            created_at: Set(Some(chrono::Utc::now().into())),
            updated_at: Set(Some(chrono::Utc::now().into())),
            ..Default::default()
        };

        let result = mail.insert(&self.db).await?;
        Ok(result.id)
    }

    /// HTML template dosyasını render et - eski method (fallback)
    // pub fn render_html_template(
    //     template_name: &str,
    //     variables: &HashMap<String, serde_json::Value>,
    // ) -> Result<String, TemplateServiceError> {
    //     Self::render_html_template_with_settings(template_name, variables, None, "tr")
    // }

    /// HTML template dosyasını DB'den settings çekerek render et
    pub async fn render_html_template_with_db(
        db: &DatabaseConnection,
        template_name: &str,
        variables: &HashMap<String, serde_json::Value>,
        language: &str,
    ) -> Result<String, TemplateServiceError> {
        // Settings'i DB'den yükle
        let settings = crate::middleware::global_context::SettingsCache::load_from_db(db)
            .await
            .ok();

        Self::render_html_template_with_settings(
            template_name,
            variables,
            settings.as_ref(),
            language,
        )
    }

    /// HTML template dosyasını render et - settings desteği ile
    pub fn render_html_template_with_settings(
        template_name: &str,
        variables: &HashMap<String, serde_json::Value>,
        settings: Option<&crate::middleware::global_context::SettingsCache>,
        language: &str,
    ) -> Result<String, TemplateServiceError> {
        // Aktif temayı al
        let theme = if let Some(settings) = settings {
            settings
                .frontend_theme
                .clone()
                .unwrap_or_else(|| "base".to_string())
        } else {
            "base".to_string()
        };

        // Template dosya yolu - aktif tema kullan
        let template_path = format!("{}/mailer/{}.html", theme, template_name);
        let template_file_path = format!("templates/{}", template_path);

        // Boş Tera instance oluştur - sadece mailer template'lerini yükle
        let mut tera = Tera::default();
        tera.autoescape_on(vec!["html", "htm", "xml"]);

        // Ana template ve base template'i oku
        let template_content = std::fs::read_to_string(&template_file_path).map_err(|e| {
            TemplateServiceError::RenderError(format!("Template okuma hatası: {}", e))
        })?;

        // @@theme@@ placeholder'ını değiştir
        let processed_template = template_content.replace("@@theme@@", &theme);

        // Base template'i de oku ve işle (eğer extends varsa)
        let base_template_path = format!("templates/{}/mailer/base.html", theme);
        if let Ok(base_content) = std::fs::read_to_string(&base_template_path) {
            let processed_base = base_content.replace("@@theme@@", &theme);
            let base_path = format!("{}/mailer/base.html", theme);
            tera.add_raw_template(&base_path, &processed_base)
                .map_err(|e| {
                    TemplateServiceError::RenderError(format!("Base template hatası: {}", e))
                })?;
        }

        // Ana template'i ekle
        tera.add_raw_template(&template_path, &processed_template)
            .map_err(|e| {
                TemplateServiceError::RenderError(format!("Template ekleme hatası: {}", e))
            })?;

        // Register i18n function for mail templates
        let i18n = crate::i18n::I18n::new();
        let i18n_clone = i18n.clone();
        tera.register_function(
            "t",
            move |args: &std::collections::HashMap<String, serde_json::Value>| {
                let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("tr");
                let default = args.get("default").and_then(|v| v.as_str());

                let translated = i18n_clone.t_with_default(key, lang, default);

                Ok(serde_json::to_value(translated).unwrap())
            },
        );

        // Custom filters
        // format_price filter - currency parametresi alır
        // Usage: {{ price | format_price(currency="USD") }} veya {{ price | format_price }}
        tera.register_filter(
            "format_price",
            |value: &serde_json::Value,
             args: &std::collections::HashMap<String, serde_json::Value>|
             -> tera::Result<serde_json::Value> {
                // Currency parametresini al, yoksa TRY kullan
                let currency = args
                    .get("currency")
                    .and_then(|v| v.as_str())
                    .unwrap_or("TRY");

                match value.as_f64() {
                    Some(price) => Ok(serde_json::Value::String(
                        crate::modules::utils::format_price::format_price(price, currency),
                    )),
                    None => {
                        // Belki string olarak gelmiştir (Decimal to string gibi)
                        if let Some(s) = value.as_str() {
                            if let Ok(price) = s.parse::<f64>() {
                                return Ok(serde_json::Value::String(
                                    crate::modules::utils::format_price::format_price(
                                        price, currency,
                                    ),
                                ));
                            }
                        }
                        Ok(value.clone())
                    }
                }
            },
        );

        // Context oluştur ve değişkenleri ekle
        let mut context = Context::new();
        for (key, value) in variables {
            context.insert(key, value);
        }

        // Language değişkenini ekle (mail template'lerinde current_language yerine)
        context.insert("current_language", language);
        context.insert("language", language);

        // Settings varsa ekle
        if let Some(settings) = settings {
            // Site bilgileri
            if let Some(v) = settings.get("site_name_langs") {
                context.insert("site_name_langs", &v);
            }
            if let Some(v) = &settings.site_logo {
                context.insert("site_logo", v);
            }
            if let Some(v) = &settings.site_favicon {
                context.insert("site_favicon", v);
            }
            // İletişim bilgileri
            if let Some(v) = &settings.contact_email {
                context.insert("contact_email", v);
            }
            if let Some(v) = &settings.contact_phone {
                context.insert("contact_phone", v);
            }
            if let Some(v) = &settings.contact_address {
                context.insert("contact_address", v);
            }
            // Sosyal medya
            if let Some(v) = &settings.social_facebook {
                context.insert("social_facebook", v);
            }
            if let Some(v) = &settings.social_twitter {
                context.insert("social_twitter", v);
            }
            if let Some(v) = &settings.social_instagram {
                context.insert("social_instagram", v);
            }
            if let Some(v) = &settings.social_linkedin {
                context.insert("social_linkedin", v);
            }
            if let Some(v) = &settings.social_youtube {
                context.insert("social_youtube", v);
            }

            // Site name - dil bazlı
            if let Some(site_name) = settings.get_site_name(language) {
                context.insert("site_name", &site_name);
            }
        }

        // Template'i render et - tema sistemi otomatik olarak doğru path'i bulacak
        let rendered = match tera.render(&template_path, &context) {
            Ok(html) => html,
            Err(e) => {
                // Tema template'i bulunamazsa base'e fallback
                if theme != "base" {
                    eprintln!(
                        "⚠️  Mail template '{}' bulunamadı, base tema deneniyor...",
                        template_path
                    );
                    let base_template_path = format!("base/mailer/{}.html", template_name);
                    tera.render(&base_template_path, &context)?
                } else {
                    return Err(e.into());
                }
            }
        };

        Ok(rendered)
    }
}

/// Hızlı mail gönderme helper'ları
pub struct MailHelper;

impl MailHelper {
    /// Basit mail gönderimi - tek parametre ile tüm bilgileri içeren data
    pub async fn send_simple_mail(
        db: &DatabaseConnection,
        data: serde_json::Value,
    ) -> Result<i64, TemplateServiceError> {
        let template_service = TemplateService::new(db.clone());

        // Zorunlu alanları kontrol et
        let to_email = data
            .get("to_email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                TemplateServiceError::RenderError("to_email field is required".to_string())
            })?;

        let subject = data
            .get("subject")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                TemplateServiceError::RenderError("subject field is required".to_string())
            })?;

        // Opsiyonel alanlar
        let to_name = data.get("to_name").and_then(|v| v.as_str());
        let template_name = data
            .get("template_name")
            .and_then(|v| v.as_str())
            .unwrap_or("simple_mail");
        let language = data
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("tr");
        let scheduled_at = data
            .get("scheduled_at")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        // Variables - data'nın kendisini variables olarak kullan
        let mut variables = std::collections::HashMap::new();
        if let serde_json::Value::Object(map) = &data {
            for (key, value) in map {
                // Mail sistem alanlarını variables'a ekleme
                if !matches!(
                    key.as_str(),
                    "to_email"
                        | "to_name"
                        | "subject"
                        | "template_name"
                        | "language"
                        | "scheduled_at"
                ) {
                    variables.insert(key.clone(), value.clone());
                }
            }
        }

        template_service
            .queue_mail(
                template_name,
                to_email,
                to_name,
                subject,
                variables,
                language,
                scheduled_at,
            )
            .await
    }

    /// Kullanıcı doğrulama maili - AppState ile tema desteği
    pub async fn send_user_verification_with_app_state(
        app_state: &crate::app_state::AppState,
        user_email: &str,
        user_name: &str,
        verification_code: &str,
        language: &str,
    ) -> Result<i64, TemplateServiceError> {
        let template_service = TemplateService::with_app_state(
            app_state.db.clone(),
            std::sync::Arc::new(app_state.clone()),
        );

        let mut variables = HashMap::new();
        variables.insert(
            "name".to_string(),
            serde_json::Value::String(user_name.to_string()),
        );
        variables.insert(
            "email".to_string(),
            serde_json::Value::String(user_email.to_string()),
        );
        variables.insert(
            "verification_code".to_string(),
            serde_json::Value::String(verification_code.to_string()),
        );

        let subject = if language == "tr" {
            "Hesap Doğrulama"
        } else {
            "Account Verification"
        };

        template_service
            .queue_mail(
                "user_verification",
                user_email,
                Some(user_name),
                subject,
                variables,
                language,
                None,
            )
            .await
    }

    /// Sipariş onay maili
    pub async fn send_order_confirmation(
        db: &DatabaseConnection,
        user_email: &str,
        user_name: &str,
        order_id: &str,
        order_date: &str,
        payment_method: &str,
        order_summary: &str,
        total_amount: &str,
        delivery_address: &str,
        order_url: &str,
        order_items: Option<&serde_json::Value>,
        currency: Option<&str>,
        language: &str,
    ) -> Result<i64, TemplateServiceError> {
        let template_service = TemplateService::new(db.clone());

        let mut variables = HashMap::new();
        variables.insert(
            "customer_name".to_string(),
            serde_json::Value::String(user_name.to_string()),
        );
        variables.insert(
            "order_id".to_string(),
            serde_json::Value::String(order_id.to_string()),
        );
        variables.insert(
            "order_date".to_string(),
            serde_json::Value::String(order_date.to_string()),
        );
        variables.insert(
            "payment_method".to_string(),
            serde_json::Value::String(payment_method.to_string()),
        );
        variables.insert(
            "order_summary".to_string(),
            serde_json::Value::String(order_summary.to_string()),
        );
        variables.insert(
            "total_amount".to_string(),
            serde_json::Value::String(total_amount.to_string()),
        );
        variables.insert(
            "delivery_address".to_string(),
            serde_json::Value::String(delivery_address.to_string()),
        );
        variables.insert(
            "order_url".to_string(),
            serde_json::Value::String(order_url.to_string()),
        );

        // Currency bilgisi
        variables.insert(
            "currency".to_string(),
            serde_json::Value::String(currency.unwrap_or("TRY").to_string()),
        );

        if let Some(items) = order_items {
            variables.insert("order_items".to_string(), items.clone());
        }

        let subject = if language == "tr" {
            format!("Sipariş Onayı - {}", order_id)
        } else {
            format!("Order Confirmation - {}", order_id)
        };

        template_service
            .queue_mail(
                "order_confirmation",
                user_email,
                Some(user_name),
                &subject,
                variables,
                language,
                None,
            )
            .await
    }

    /// Şifre sıfırlama maili
    pub async fn send_password_reset(
        app_state: &crate::app_state::AppState,
        user_email: &str,
        user_name: &str,
        reset_url: &str,
        language: &str,
    ) -> Result<i64, TemplateServiceError> {
        let template_service = TemplateService::with_app_state(
            app_state.db.clone(),
            std::sync::Arc::new(app_state.clone()),
        );

        let mut variables = HashMap::new();
        variables.insert(
            "name".to_string(),
            serde_json::Value::String(user_name.to_string()),
        );
        variables.insert(
            "reset_url".to_string(),
            serde_json::Value::String(reset_url.to_string()),
        );

        let subject = if language == "tr" {
            "Şifre Sıfırlama Talebi"
        } else {
            "Password Reset Request"
        };

        template_service
            .queue_mail(
                "password_reset",
                user_email,
                Some(user_name),
                subject,
                variables,
                language,
                None,
            )
            .await
    }
}

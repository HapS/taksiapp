use crate::modules::utils::format_price::format_price;
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

    /// Sipariş durumu güncelleme maili
    pub async fn send_order_status_update(
        db: &DatabaseConnection,
        user_email: &str,
        user_name: &str,
        order_id: &str,
        old_status: &str,
        new_status: &str,
        status_message: &str,
        cargo_company: Option<&i64>,
        cargo_tracking_no: Option<&str>,
        order_url: &str,
        total_amount: Option<&str>,
        order_items: Option<&serde_json::Value>, // Sipariş ürünleri JSON olarak
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
            "old_status".to_string(),
            serde_json::Value::String(old_status.to_string()),
        );
        variables.insert(
            "new_status".to_string(),
            serde_json::Value::String(new_status.to_string()),
        );
        variables.insert(
            "status_message".to_string(),
            serde_json::Value::String(status_message.to_string()),
        );
        variables.insert(
            "order_url".to_string(),
            serde_json::Value::String(order_url.to_string()),
        );
        variables.insert(
            "update_date".to_string(),
            serde_json::Value::String(chrono::Utc::now().format("%d.%m.%Y %H:%M").to_string()),
        );

        // Currency bilgisi
        variables.insert(
            "currency".to_string(),
            serde_json::Value::String(currency.unwrap_or("TRY").to_string()),
        );

        // Toplam tutar varsa ekle
        if let Some(amount) = total_amount {
            variables.insert(
                "total_amount".to_string(),
                serde_json::Value::String(amount.to_string()),
            );
        }

        // Sipariş ürünleri varsa ekle
        if let Some(items) = order_items {
            variables.insert("order_items".to_string(), items.clone());
        }

        // Kargo bilgileri varsa ekle — ID yerine kargo şirketi adını çöz
        if let Some(company_id) = cargo_company {
            let cargo_name = resolve_cargo_company_name(db, *company_id).await;
            variables.insert(
                "cargo_company".to_string(),
                serde_json::Value::String(cargo_name),
            );
        }
        if let Some(tracking) = cargo_tracking_no {
            variables.insert(
                "cargo_tracking_no".to_string(),
                serde_json::Value::String(tracking.to_string()),
            );
        }

        // Duruma göre özel mesajlar
        let (subject, status_title) = match new_status {
            "confirmed" => {
                if language == "tr" {
                    ("Siparişiniz Onaylandı", "Sipariş Alındı")
                } else {
                    ("Your Order is Confirmed", "Order Received")
                }
            }
            "preparing" => {
                if language == "tr" {
                    ("Siparişiniz Hazırlanıyor", "Sipariş Hazırlanıyor")
                } else {
                    ("Your Order is Being Prepared", "Order Preparing")
                }
            }
            "shipped" => {
                if language == "tr" {
                    ("Siparişiniz Kargoya Verildi", "Kargoya Verildi")
                } else {
                    ("Your Order has been Shipped", "Shipped")
                }
            }
            "delivered" => {
                if language == "tr" {
                    ("Siparişiniz Teslim Edildi", "Teslim Edildi")
                } else {
                    ("Your Order has been Delivered", "Delivered")
                }
            }
            "cancelled" => {
                if language == "tr" {
                    ("Siparişiniz İptal Edildi", "İptal Edildi")
                } else {
                    ("Your Order has been Cancelled", "Cancelled")
                }
            }
            _ => {
                if language == "tr" {
                    ("Sipariş Durumu Güncellendi", "Durum Güncellendi")
                } else {
                    ("Order Status Updated", "Status Updated")
                }
            }
        };

        variables.insert(
            "status_title".to_string(),
            serde_json::Value::String(status_title.to_string()),
        );

        let full_subject = if language == "tr" {
            format!("{} - {}", subject, order_id)
        } else {
            format!("{} - {}", subject, order_id)
        };

        template_service
            .queue_mail(
                "order_status_update",
                user_email,
                Some(user_name),
                &full_subject,
                variables,
                language,
                None,
            )
            .await
    }

    /// İptal talebi onaylandı maili
    pub async fn send_cancel_request_accepted(
        db: &DatabaseConnection,
        user_email: &str,
        user_name: &str,
        order_id: &str,
        product_title: &str,
        variant_display: Option<&str>,
        product_cover: Option<&str>,
        quantity: i32,
        unit_price: f64,
        refund_amount: f64,
        currency: &str,
        order_url: &str,
        language: &str,
    ) -> Result<i64, TemplateServiceError> {
        let template_service = TemplateService::new(db.clone());

        let formatted_price = format_price(unit_price, currency);
        let formatted_amount = format_price(refund_amount, currency);

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
            "product_title".to_string(),
            serde_json::Value::String(product_title.to_string()),
        );
        if let Some(variant) = variant_display {
            variables.insert(
                "variant_display".to_string(),
                serde_json::Value::String(variant.to_string()),
            );
        }
        if let Some(cover) = product_cover {
            variables.insert(
                "product_cover".to_string(),
                serde_json::Value::String(cover.to_string()),
            );
        }
        variables.insert(
            "quantity".to_string(),
            serde_json::Value::Number(quantity.into()),
        );
        variables.insert(
            "unit_price".to_string(),
            serde_json::Value::String(formatted_price),
        );
        variables.insert(
            "refund_amount".to_string(),
            serde_json::Value::String(formatted_amount),
        );
        variables.insert(
            "currency".to_string(),
            serde_json::Value::String(currency.to_string()),
        );
        variables.insert(
            "order_url".to_string(),
            serde_json::Value::String(order_url.to_string()),
        );
        variables.insert(
            "date".to_string(),
            serde_json::Value::String(chrono::Utc::now().format("%d.%m.%Y %H:%M").to_string()),
        );

        let subject = if language == "tr" {
            "İptal Talebiniz Onaylandı"
        } else {
            "Your Cancel Request Has Been Accepted"
        };

        let full_subject = if language == "tr" {
            format!("{} - {}", subject, order_id)
        } else {
            format!("{} - {}", subject, order_id)
        };

        template_service
            .queue_mail(
                "cancel_request_accepted",
                user_email,
                Some(user_name),
                &full_subject,
                variables,
                language,
                None,
            )
            .await
    }

    /// İptal talebi reddedildi maili
    pub async fn send_cancel_request_rejected(
        db: &DatabaseConnection,
        user_email: &str,
        user_name: &str,
        order_id: &str,
        product_title: &str,
        variant_display: Option<&str>,
        product_cover: Option<&str>,
        quantity: i32,
        unit_price: f64,
        currency: &str,
        order_url: &str,
        language: &str,
    ) -> Result<i64, TemplateServiceError> {
        let template_service = TemplateService::new(db.clone());

        let formatted_price = format_price(unit_price, currency);

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
            "product_title".to_string(),
            serde_json::Value::String(product_title.to_string()),
        );
        if let Some(variant) = variant_display {
            variables.insert(
                "variant_display".to_string(),
                serde_json::Value::String(variant.to_string()),
            );
        }
        if let Some(cover) = product_cover {
            variables.insert(
                "product_cover".to_string(),
                serde_json::Value::String(cover.to_string()),
            );
        }
        variables.insert(
            "quantity".to_string(),
            serde_json::Value::Number(quantity.into()),
        );
        variables.insert(
            "unit_price".to_string(),
            serde_json::Value::String(formatted_price),
        );
        variables.insert(
            "currency".to_string(),
            serde_json::Value::String(currency.to_string()),
        );
        variables.insert(
            "order_url".to_string(),
            serde_json::Value::String(order_url.to_string()),
        );
        variables.insert(
            "date".to_string(),
            serde_json::Value::String(chrono::Utc::now().format("%d.%m.%Y %H:%M").to_string()),
        );

        let subject = if language == "tr" {
            "İptal Talebiniz Reddedildi"
        } else {
            "Your Cancel Request Has Been Rejected"
        };

        let full_subject = if language == "tr" {
            format!("{} - {}", subject, order_id)
        } else {
            format!("{} - {}", subject, order_id)
        };

        template_service
            .queue_mail(
                "cancel_request_rejected",
                user_email,
                Some(user_name),
                &full_subject,
                variables,
                language,
                None,
            )
            .await
    }

    /// Ödeme onay maili
    pub async fn send_payment_confirmation(
        db: &DatabaseConnection,
        user_email: &str,
        user_name: &str,
        order_id: &str,
        payment_date: &str,
        payment_method: &str,
        transaction_id: &str,
        subtotal: &str,
        total_amount: &str,
        invoice_url: &str,
        order_items: Option<&serde_json::Value>,
        currency: Option<&str>,
        language: &str,
    ) -> Result<i64, TemplateServiceError> {
        eprintln!(
            "📧 MailHelper::send_payment_confirmation started for Order: {}",
            order_id
        );
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
            "payment_date".to_string(),
            serde_json::Value::String(payment_date.to_string()),
        );
        variables.insert(
            "payment_method".to_string(),
            serde_json::Value::String(payment_method.to_string()),
        );
        variables.insert(
            "transaction_id".to_string(),
            serde_json::Value::String(transaction_id.to_string()),
        );
        variables.insert(
            "subtotal".to_string(),
            serde_json::Value::String(subtotal.to_string()),
        );
        variables.insert(
            "total_amount".to_string(),
            serde_json::Value::String(total_amount.to_string()),
        );
        variables.insert(
            "invoice_url".to_string(),
            serde_json::Value::String(invoice_url.to_string()),
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
            format!("Ödeme Onayı - {}", order_id)
        } else {
            format!("Payment Confirmation - {}", order_id)
        };

        let result = template_service
            .queue_mail(
                "payment_confirmation",
                user_email,
                Some(user_name),
                &subject,
                variables,
                language,
                None,
            )
            .await;

        if let Err(e) = &result {
            eprintln!("❌ MailHelper::send_payment_confirmation failed: {:?}", e);
        } else {
            eprintln!("✅ MailHelper::send_payment_confirmation successfully queued mail");
        }

        result
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

    /// İade durumu güncelleme maili
    pub async fn send_return_status_update(
        db: &DatabaseConnection,
        user_email: &str,
        user_name: &str,
        order_id: &str,
        return_id: i64,
        return_status: &str, // "approved", "rejected", "received", "completed"
        product_title: &str,
        variant_display: Option<&str>,
        product_cover: Option<&str>,
        quantity: i32,
        unit_price_formatted: &str,
        return_reason: Option<&str>,
        rejection_reason: Option<&str>,
        refund_amount_formatted: Option<&str>,
        refund_method_text: Option<&str>,
        order_url: &str,
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
            "return_id".to_string(),
            serde_json::Value::Number(return_id.into()),
        );
        variables.insert(
            "product_title".to_string(),
            serde_json::Value::String(product_title.to_string()),
        );
        if let Some(variant) = variant_display {
            variables.insert(
                "variant_display".to_string(),
                serde_json::Value::String(variant.to_string()),
            );
        }
        if let Some(cover) = product_cover {
            variables.insert(
                "product_cover".to_string(),
                serde_json::Value::String(cover.to_string()),
            );
        }
        variables.insert(
            "quantity".to_string(),
            serde_json::Value::Number(quantity.into()),
        );
        variables.insert(
            "unit_price".to_string(),
            serde_json::Value::String(unit_price_formatted.to_string()),
        );
        if let Some(reason) = return_reason {
            variables.insert(
                "return_reason".to_string(),
                serde_json::Value::String(reason.to_string()),
            );
        }
        if let Some(reason) = rejection_reason {
            variables.insert(
                "rejection_reason".to_string(),
                serde_json::Value::String(reason.to_string()),
            );
        }
        if let Some(amount) = refund_amount_formatted {
            variables.insert(
                "refund_amount".to_string(),
                serde_json::Value::String(amount.to_string()),
            );
        }
        if let Some(method) = refund_method_text {
            variables.insert(
                "refund_method_text".to_string(),
                serde_json::Value::String(method.to_string()),
            );
        }
        variables.insert(
            "order_url".to_string(),
            serde_json::Value::String(order_url.to_string()),
        );
        variables.insert(
            "update_date".to_string(),
            serde_json::Value::String(chrono::Utc::now().format("%d.%m.%Y %H:%M").to_string()),
        );

        // Duruma göre status_title, status_message, status_detail, next_step, icon, color
        let (
            subject,
            status_title,
            status_message,
            status_detail,
            next_step,
            status_icon_text,
            status_color,
        ) = match return_status {
            "approved" => {
                if language == "tr" {
                    (
                            "İade Talebiniz Onaylandı",
                            "İade Talebi Onaylandı",
                            "İade talebiniz yönetici tarafından onaylanmıştır.",
                            "Ürünü kargoya vererek iade sürecini başlatabilirsiniz.",
                            Some("Lütfen ürünü orijinal ambalajında kargoya verin ve kargo takip numarasını hesabınızdan girin."),
                            "✅",
                            "#28a745",
                        )
                } else {
                    (
                            "Your Return Request Has Been Approved",
                            "Return Request Approved",
                            "Your return request has been approved by the administrator.",
                            "You can now ship the product back to start the return process.",
                            Some("Please ship the product in its original packaging and enter the tracking number in your account."),
                            "✅",
                            "#28a745",
                        )
                }
            }
            "rejected" => {
                if language == "tr" {
                    (
                            "İade Talebiniz Reddedildi",
                            "İade Talebi Reddedildi",
                            "Üzgünüz, iade talebiniz yönetici tarafından reddedilmiştir.",
                            "Aşağıda red sebebini bulabilirsiniz.",
                            Some("Herhangi bir sorunuz varsa müşteri hizmetlerimizle iletişime geçebilirsiniz."),
                            "❌",
                            "#dc3545",
                        )
                } else {
                    (
                        "Your Return Request Has Been Rejected",
                        "Return Request Rejected",
                        "Sorry, your return request has been rejected by the administrator.",
                        "You can find the reason for rejection below.",
                        Some("If you have any questions, please contact our customer service."),
                        "❌",
                        "#dc3545",
                    )
                }
            }
            "received" => {
                if language == "tr" {
                    (
                            "İade Ürününüz Teslim Alındı",
                            "Ürün Teslim Alındı",
                            "İade ettiğiniz ürün depomuzda teslim alınmıştır.",
                            "Ürün incelendikten sonra iade işleminiz tamamlanacaktır.",
                            Some("İade tutarınız inceleme tamamlandıktan sonra hesabınıza yansıtılacaktır."),
                            "📦",
                            "#17a2b8",
                        )
                } else {
                    (
                        "Your Return Product Has Been Received",
                        "Product Received",
                        "The product you returned has been received at our warehouse.",
                        "Your return will be completed after the product is inspected.",
                        Some("Your refund will be processed after the inspection is complete."),
                        "📦",
                        "#17a2b8",
                    )
                }
            }
            "completed" => {
                if language == "tr" {
                    (
                        "İade İşleminiz Tamamlandı",
                        "İade Tamamlandı",
                        "İade işleminiz başarıyla tamamlanmıştır.",
                        "İade tutarınız aşağıda belirtilen yöntemle hesabınıza yansıtılacaktır.",
                        None,
                        "🎉",
                        "#28a745",
                    )
                } else {
                    (
                        "Your Return Has Been Completed",
                        "Return Completed",
                        "Your return has been completed successfully.",
                        "Your refund will be credited using the method indicated below.",
                        None,
                        "🎉",
                        "#28a745",
                    )
                }
            }
            _ => {
                if language == "tr" {
                    (
                        "İade Durumu Güncellendi",
                        "İade Durumu Güncellendi",
                        "İade talebinizin durumu güncellenmiştir.",
                        "",
                        None,
                        "🔄",
                        "#007bff",
                    )
                } else {
                    (
                        "Return Status Updated",
                        "Return Status Updated",
                        "Your return request status has been updated.",
                        "",
                        None,
                        "🔄",
                        "#007bff",
                    )
                }
            }
        };

        variables.insert(
            "status_title".to_string(),
            serde_json::Value::String(status_title.to_string()),
        );
        variables.insert(
            "status_message".to_string(),
            serde_json::Value::String(status_message.to_string()),
        );
        variables.insert(
            "status_detail".to_string(),
            serde_json::Value::String(status_detail.to_string()),
        );
        variables.insert(
            "status_icon_text".to_string(),
            serde_json::Value::String(status_icon_text.to_string()),
        );
        variables.insert(
            "status_color".to_string(),
            serde_json::Value::String(status_color.to_string()),
        );
        if let Some(step) = next_step {
            variables.insert(
                "next_step".to_string(),
                serde_json::Value::String(step.to_string()),
            );
        }

        let full_subject = format!("{} - {}", subject, order_id);

        template_service
            .queue_mail(
                "return_status_update",
                user_email,
                Some(user_name),
                &full_subject,
                variables,
                language,
                None,
            )
            .await
    }
}

/// Kargo şirketi ID'sinden adını çöz
async fn resolve_cargo_company_name(db: &DatabaseConnection, company_id: i64) -> String {
    use crate::modules::ecommerce::models::kargo_sirketleri::Entity as KargoEntity;

    match KargoEntity::find_by_id(company_id as i32).one(db).await {
        Ok(Some(kargo)) => kargo.title,
        _ => format!("Kargo #{}", company_id),
    }
}

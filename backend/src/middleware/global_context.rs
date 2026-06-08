use crate::app_state::AppState;
use crate::config;
// use crate::middleware::auth::CurrentUser;
use crate::modules::auth::models::SessionData;
// use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tower_sessions::Session;

//======= BURASI SADECE TEMPLATE İÇİN GEREKLİ VERİLERİ TAŞIYAN GLOBAL CONTEXT MIDDLEWARE DIR =======//
// Bu middleware, her istekte global olarak kullanılacak verileri toplar ve request in extensions kısmına ekler.
// Örneğin: Giriş yapmış kullanıcı bilgisi, sepet sayısı, desteklenen diller vb.
//BAŞKA BİR SİKİM İÇİN KULLANMAYIN! YAPAY ZEKA GPT BURADAN UZAK DUR!

/// Settings cache - şifreler ve hassas veriler hariç
/// Template'lerde kullanılabilecek tüm ayarlar
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SettingsCache {
    // Site Bilgileri (Çok Dilli)
    pub site_name_langs: Option<serde_json::Value>,
    pub site_description_langs: Option<serde_json::Value>,
    pub site_keywords_langs: Option<serde_json::Value>,
    pub site_logo: Option<String>,
    pub site_logo_dark: Option<String>,
    pub site_favicon: Option<String>,

    // SEO Ayarları (Çok Dilli)
    pub seo_title_langs: Option<serde_json::Value>,
    pub seo_description_langs: Option<serde_json::Value>,
    pub seo_image_langs: Option<serde_json::Value>,

    // Sosyal Medya
    pub social_facebook: Option<String>,
    pub social_twitter: Option<String>,
    pub social_instagram: Option<String>,
    pub social_linkedin: Option<String>,
    pub social_youtube: Option<String>,

    // İletişim Bilgileri
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_address: Option<String>,
    pub contact_map_embed: Option<String>,

    // Banka Bilgileri (sadece public kısımlar)
    pub bank1_name: Option<String>,
    pub bank1_account_holder: Option<String>,
    pub bank1_iban: Option<String>,
    pub bank1_branch_code: Option<String>,
    pub bank2_name: Option<String>,
    pub bank2_account_holder: Option<String>,
    pub bank2_iban: Option<String>,
    pub bank2_branch_code: Option<String>,

    // Diğer Ayarlar
    pub maintenance_mode: Option<bool>,
    pub analytics_code: Option<String>,
    pub custom_css: Option<String>,
    pub custom_js: Option<String>,
    pub debug_logs: Option<bool>,

    // Vocabulary Ayarları
    pub vocab_navbar_menu: Option<i64>,
    pub vocab_footer_menu: Option<i64>,
    pub vocab_product_categories: Option<i64>,
    pub vocab_blog_categories: Option<i64>,
    pub vocab_news_categories: Option<i64>,
    pub vocab_page_categories: Option<i64>,
    pub vocab_tags_categories: Option<i64>,

    // Payment Provider Ayarları (sadece genel bilgiler, API key/secret yok)
    pub default_payment_provider: Option<String>,

    // Frontend Theme Ayarı
    pub frontend_theme: Option<String>,

    // Default Content Ayarları
    pub default_home_content_id: Option<i64>,

    // Varsayılan Para Birimi
    pub default_currency: Option<String>, // "TRY", "USD", "EUR"

    // Robots.txt
    pub robots: Option<String>,
    // NOT: smtp_password, payment_providers config (api_key, secret_key) gibi
    // hassas veriler BU STRUCT'A DAHİL DEĞİL!
    pub free_shipping_threshold: Option<f64>,

    // Desteklenen Para Birimleri
    pub supported_currencies: Option<Vec<String>>,
}

impl SettingsCache {
    /// DB'den ayarları yükle ve hassas verileri filtrele
    pub async fn load_from_db(db: &sea_orm::DatabaseConnection) -> Result<Self, String> {
        use sea_orm::EntityTrait;

        let settings = crate::modules::admin::models::settings::Entity::find_by_id(1)
            .one(db)
            .await
            .map_err(|e| format!("Ayarlar yüklenemedi: {}", e))?;

        if let Some(settings) = settings {
            if let Some(serde_json::Value::Object(data)) = settings.data {
                Ok(Self::from_settings_data(&data))
            } else {
                Ok(Self::default())
            }
        } else {
            Ok(Self::default())
        }
    }

    /// Settings data JSON'undan SettingsCache oluştur (hassas veriler hariç)
    fn from_settings_data(data: &serde_json::Map<String, serde_json::Value>) -> Self {
        Self {
            site_name_langs: data.get("site_name_langs").cloned(),
            site_description_langs: data.get("site_description_langs").cloned(),
            site_keywords_langs: data.get("site_keywords_langs").cloned(),
            site_logo: data
                .get("site_logo")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            site_logo_dark: data
                .get("site_logo_dark")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            site_favicon: data
                .get("site_favicon")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),

            seo_title_langs: data.get("seo_title_langs").cloned(),
            seo_description_langs: data.get("seo_description_langs").cloned(),
            seo_image_langs: data.get("seo_image_langs").cloned(),

            social_facebook: data
                .get("social_facebook")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            social_twitter: data
                .get("social_twitter")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            social_instagram: data
                .get("social_instagram")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            social_linkedin: data
                .get("social_linkedin")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            social_youtube: data
                .get("social_youtube")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),

            contact_email: data
                .get("contact_email")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            contact_phone: data
                .get("contact_phone")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            contact_address: data
                .get("contact_address")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),

            contact_map_embed: data
                .get("contact_map_embed")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),

            bank1_name: data
                .get("bank1_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bank1_account_holder: data
                .get("bank1_account_holder")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bank1_iban: data
                .get("bank1_iban")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bank1_branch_code: data
                .get("bank1_branch_code")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bank2_name: data
                .get("bank2_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bank2_account_holder: data
                .get("bank2_account_holder")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bank2_iban: data
                .get("bank2_iban")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            bank2_branch_code: data
                .get("bank2_branch_code")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),

            maintenance_mode: data.get("maintenance_mode").and_then(|v| v.as_bool()),
            analytics_code: data
                .get("analytics_code")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            custom_css: data
                .get("custom_css")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            custom_js: data
                .get("custom_js")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),

            vocab_navbar_menu: data.get("vocab_navbar_menu").and_then(|v| v.as_i64()),
            vocab_footer_menu: data.get("vocab_footer_menu").and_then(|v| v.as_i64()),
            vocab_product_categories: data
                .get("vocab_product_categories")
                .and_then(|v| v.as_i64()),
            vocab_blog_categories: data.get("vocab_blog_categories").and_then(|v| v.as_i64()),
            vocab_news_categories: data.get("vocab_news_categories").and_then(|v| v.as_i64()),
            vocab_page_categories: data.get("vocab_page_categories").and_then(|v| v.as_i64()),
            vocab_tags_categories: data.get("vocab_tags_categories").and_then(|v| v.as_i64()),

            default_payment_provider: data
                .get("default_payment_provider")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            frontend_theme: data
                .get("frontend_theme")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            default_home_content_id: data.get("default_home_content_id").and_then(|v| v.as_i64()),
            default_currency: data
                .get("default_currency")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            robots: data
                .get("robots")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            free_shipping_threshold: data.get("free_shipping_threshold").and_then(|v| v.as_f64()),
            supported_currencies: data.get("supported_currencies").and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect()
                })
            }),
            debug_logs: data.get("debug_logs").and_then(|v| v.as_bool()),
        }
    }

    /// Belirli bir ayar anahtarını al
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        match key {
            "site_name_langs" => self.site_name_langs.clone(),
            "site_description_langs" => self.site_description_langs.clone(),
            "site_keywords_langs" => self.site_keywords_langs.clone(),
            "site_logo" => self
                .site_logo
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "site_logo_dark" => self
                .site_logo_dark
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "site_favicon" => self
                .site_favicon
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),

            "seo_title_langs" => self.seo_title_langs.clone(),
            "seo_description_langs" => self.seo_description_langs.clone(),
            "seo_image_langs" => self.seo_image_langs.clone(),

            "social_facebook" => self
                .social_facebook
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "social_twitter" => self
                .social_twitter
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "social_instagram" => self
                .social_instagram
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "social_linkedin" => self
                .social_linkedin
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "social_youtube" => self
                .social_youtube
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),

            "contact_email" => self
                .contact_email
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "contact_phone" => self
                .contact_phone
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "contact_address" => self
                .contact_address
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),

            "contact_map_embed" => self
                .contact_map_embed
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),

            "bank1_name" => self
                .bank1_name
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "bank1_account_holder" => self
                .bank1_account_holder
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "bank1_iban" => self
                .bank1_iban
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "bank1_branch_code" => self
                .bank1_branch_code
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "bank2_name" => self
                .bank2_name
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "bank2_account_holder" => self
                .bank2_account_holder
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "bank2_iban" => self
                .bank2_iban
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "bank2_branch_code" => self
                .bank2_branch_code
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),

            "maintenance_mode" => self.maintenance_mode.map(|b| serde_json::Value::Bool(b)),
            "analytics_code" => self
                .analytics_code
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "custom_css" => self
                .custom_css
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "custom_js" => self
                .custom_js
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),

            "vocab_navbar_menu" => self
                .vocab_navbar_menu
                .map(|i| serde_json::Value::Number(i.into())),
            "vocab_footer_menu" => self
                .vocab_footer_menu
                .map(|i| serde_json::Value::Number(i.into())),
            "vocab_product_categories" => self
                .vocab_product_categories
                .map(|i| serde_json::Value::Number(i.into())),
            "vocab_blog_categories" => self
                .vocab_blog_categories
                .map(|i| serde_json::Value::Number(i.into())),
            "vocab_news_categories" => self
                .vocab_news_categories
                .map(|i| serde_json::Value::Number(i.into())),
            "vocab_page_categories" => self
                .vocab_page_categories
                .map(|i| serde_json::Value::Number(i.into())),
            "vocab_tags_categories" => self
                .vocab_tags_categories
                .map(|i| serde_json::Value::Number(i.into())),

            "default_payment_provider" => self
                .default_payment_provider
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "frontend_theme" => self
                .frontend_theme
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "robots" => self
                .robots
                .as_ref()
                .map(|s| serde_json::Value::String(s.clone())),
            "supported_currencies" => self.supported_currencies.as_ref().map(|currencies| {
                serde_json::Value::Array(
                    currencies
                        .iter()
                        .map(|c| serde_json::Value::String(c.clone()))
                        .collect(),
                )
            }),
            "debug_logs" => self
                .debug_logs
                .as_ref()
                .map(|b| serde_json::Value::Bool(*b)),

            _ => None,
        }
    }

    /// Default home content ID'sini al
    pub fn get_default_home_content_id(&self) -> i64 {
        self.default_home_content_id.unwrap_or(70)
    }

    /// Varsayılan para birimini al (default_currency, yoksa TRY)
    pub fn get_sale_currency(&self) -> String {
        self.default_currency
            .clone()
            .unwrap_or_else(|| "TRY".to_string())
    }

    /// Desteklenen para birimlerini al (varsayılan: ["TRY"])
    pub fn get_supported_currencies(&self) -> Vec<String> {
        self.supported_currencies
            .clone()
            .unwrap_or_else(|| vec!["TRY".to_string()])
    }

    /// Site adını dil bazlı al
    pub fn get_site_name(&self, lang: &str) -> Option<String> {
        self.site_name_langs.as_ref().and_then(|v| {
            v.get("langs")
                .and_then(|langs| langs.get(lang))
                .and_then(|lang_data| lang_data.get("title"))
                .and_then(|title| title.as_str())
                .map(|s| s.to_string())
        })
    }
}

/// Menu cache - Her dil için menü verileri
/// Template'lerde kullanılabilecek menü item'ları
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MenuCache {
    /// Her dil için menu items: "tr" -> [...], "en" -> [...]
    pub items: HashMap<String, Vec<serde_json::Value>>,
}

impl MenuCache {
    /// DB'den tüm diller için menü verilerini yükle
    pub async fn load_from_db(
        db: &sea_orm::DatabaseConnection,
        languages: &[String],
    ) -> Result<Self, String> {
        let mut items = HashMap::new();

        for lang in languages {
            let menu_items =
                crate::modules::taxonomy::services::term_service::load_menu_items(db, lang).await;
            items.insert(lang.clone(), menu_items);
        }

        Ok(Self { items })
    }

    /// Belirli bir dil için menü item'larını al
    pub fn get(&self, lang: &str) -> Vec<serde_json::Value> {
        self.items.get(lang).cloned().unwrap_or_default()
    }

    /// Cache'i yenile (term güncellenince)
    pub async fn refresh(
        &mut self,
        db: &sea_orm::DatabaseConnection,
        languages: &[String],
    ) -> Result<(), String> {
        let new_cache = Self::load_from_db(db, languages).await?;
        self.items = new_cache.items;
        Ok(())
    }
}

/// Current language wrapper for API functions
#[derive(Clone, Debug)]
pub struct CurrentLanguage(pub String);

#[derive(Clone, Debug, Default)]
pub struct GlobalContext {
    // pub user: Option<CurrentUser>,
    pub session_data: Option<SessionData>,
    pub has_admin_access: bool,
    pub has_b2b_access: bool,
    pub is_authenticated: bool,
    pub is_guest: bool,
    pub cart_count: i64,
    pub cart_total_amount: Option<f64>,
    pub cart_total_amount_formatted: Option<String>,
    pub bookmark_product_count: u64,
    pub free_shipping_threshold: Option<f64>,
    pub free_shipping_threshold_formatted: Option<String>,
    pub supported_languages: std::collections::HashMap<String, String>,
    pub current_language: String,
    pub settings: SettingsCache,
    pub menu_items: Vec<serde_json::Value>,
    pub debug: bool,
    /// Kullanıcının seçtiği görüntüleme para birimi (session'dan)
    pub display_currency: String,
    /// Desteklenen para birimleri listesi (admin ayarlarından)
    pub supported_currencies: Vec<CurrencyDisplayInfo>,
    pub base_url: String,
}

/// Frontend template'lerinde para birimi dropdown'ı için bilgi
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CurrencyDisplayInfo {
    pub code: String,
    pub name: String,
    pub symbol: String,
    pub flag: String,
    pub is_active: bool,
}

pub async fn global_context_middleware(
    State(state): State<AppState>,
    session: Session,
    mut request: Request,
    next: Next,
) -> Response {
    let mut global_context = GlobalContext::default();
    let config = config::get_config();

    // 0. Settings cache'den ayarları al
    if let Ok(settings) = state.settings_cache.read() {
        global_context.settings = settings.clone();
    }

    // 1. Session'dan kullanıcı kimliğini al (misafir kullanıcı oluşturma artık sepet ve favori işlemlerinde yapılıyor)
    let user_id = session.get::<i64>("user_id").await.unwrap_or(None);

    // 2. Kullanıcı bilgilerini alarak misafir mi yoksa giriş yapmış mı olduğunu belirle
    let mut is_authenticated = false;
    let mut is_guest = false;

    if let Some(uid) = user_id {
        // Kullanıcı bilgilerini al
        if let Ok(Some(user)) = crate::modules::auth::models::user::Entity::find_by_id(uid)
            .one(&state.db)
            .await
        {
            is_guest = user.is_guest;
            is_authenticated = !user.is_guest; // Guest değilse authenticated
        }
    }

    // 3. Get cart count for the user (guest or authenticated)
    //burayı değiştiriyoruz guest gelir gelmez cart açmanın manası yok çünkü cart  boş olacak, eğer sepete bir şey atarsa cart açılıyor zaten atmazsa boşuna db de cart yer işgal etmesin
    //bu yüzden guest veya register olmuş kullanıcıya cart açma işlemi yapmıyoruz, sadece var olan cart ı kontrol ediyoruz, cart yoksa 0 dönecek

    // Sepet fiyatlarını doğru para biriminde göstermek için display_currency'yi erken hesapla
    // (Tam hesaplama aşağıda tekrar yapılacak — burada sadece sepet çağrısı için gerekli)
    let early_display_currency = {
        let sale_cur = global_context.settings.get_sale_currency();
        let supported = global_context.settings.get_supported_currencies();

        // Session'dan
        let sess_cur = session
            .get::<String>("display_currency")
            .await
            .unwrap_or(None);

        // Cookie'den
        let cookie_cur = request
            .headers()
            .get("cookie")
            .and_then(|h| h.to_str().ok())
            .and_then(|cookie_str| {
                for cookie in cookie_str.split(';') {
                    let cookie = cookie.trim();
                    if cookie.starts_with("display_currency=") {
                        let cur = &cookie[17..];
                        if supported.contains(&cur.to_string()) {
                            return Some(cur.to_string());
                        }
                    }
                }
                None
            });

        sess_cur.or(cookie_cur).unwrap_or(sale_cur)
    };

    if let Some(uid) = user_id {
        if let Ok(Some(cart)) =
            crate::modules::ecommerce::services::cart_service::find_active_cart_by_user(
                &state.db, uid,
            )
            .await
        {
            if let Ok(cart_data) = crate::modules::ecommerce::services::cart_service::get_cart(
                &state.db,
                cart.id,
                None,
                user_id,
                Some(early_display_currency.clone()),
            )
            .await
            {
                global_context.cart_count = cart_data.item_count as i64;
                global_context.cart_total_amount = Some(cart_data.total);
                global_context.cart_total_amount_formatted = Some(cart_data.total_formatted);
            }
        }
    }

    if let Some(uid) = user_id {
        if let Ok(favorite_count) =
            crate::modules::bookmarks::services::bookmark_service::BookmarkService::bookmarks_product_count(&state.db, uid)
                .await
        {
            global_context.bookmark_product_count = favorite_count as u64;
        }
    }

    // 4. Get Session Data (includes admin access, permissions, profile, etc.)
    if let Ok(Some(session_data)) = session.get::<SessionData>("user_data").await {
        global_context.has_admin_access = session_data.has_admin_access;
        global_context.has_b2b_access = session_data.has_b2b_access;
        global_context.session_data = Some(session_data);
    }

    // Set authentication status
    global_context.is_authenticated = is_authenticated;
    global_context.is_guest = is_guest;

    // free_shipping_threshold admin ayarlarında sale_currency cinsinden (ör. 500 TRY)
    // Kullanıcının seçtiği display_currency farklıysa döviz kuru ile dönüştür
    let raw_threshold =
        crate::modules::admin::services::settings_service::get_free_shipping_threshold(&state.db)
            .await;
    let sale_cur_for_threshold = global_context.settings.get_sale_currency();

    let converted_threshold = if sale_cur_for_threshold == early_display_currency {
        raw_threshold
    } else if let Some(raw_val) = raw_threshold {
        let rates =
            crate::modules::currency::services::exchange_rate_service::get_cached_rates(&state.db)
                .await;
        if let Some(ref r) = rates {
            crate::modules::currency::services::exchange_rate_service::convert_currency(
                raw_val,
                &sale_cur_for_threshold,
                &early_display_currency,
                r,
            )
        } else {
            Some(raw_val)
        }
    } else {
        None
    };

    global_context.free_shipping_threshold = converted_threshold;

    global_context.free_shipping_threshold_formatted =
        Some(crate::modules::utils::format_price::format_price(
            converted_threshold.unwrap_or(0.0),
            &early_display_currency,
        ));

    //5. Supported Languages and Current Language from URL or Cookie
    // URL'den dil kodunu çıkar: /tr/page/123 -> "tr"
    let path = request.uri().path();
    let url_language = if path.starts_with('/') && path.len() > 1 {
        let parts: Vec<&str> = path[1..].split('/').collect();
        if !parts.is_empty() && parts[0].len() == 2 {
            // İlk segment 2 karakter ise dil kodu olabilir
            let potential_lang = parts[0];
            if config.is_language_supported(potential_lang) {
                Some(potential_lang)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Cookie'den dil kodunu al
    let cookie_language = request
        .headers()
        .get("cookie")
        .and_then(|cookie_header| cookie_header.to_str().ok())
        .and_then(|cookie_str| {
            // Cookie'den language= değerini bul
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if cookie.starts_with("language=") {
                    let lang = &cookie[9..]; // "language=" kısmını atla
                    if config.is_language_supported(lang) {
                        return Some(lang);
                    }
                }
            }
            None
        });

    // Öncelik sırası: URL > Cookie > Default
    let detected_language = url_language.or(cookie_language);

    global_context.supported_languages = config.supported_languages.clone();
    global_context.current_language = config.get_language_or_default(detected_language);
    global_context.debug = config.is_debug();
    global_context.base_url = config.get_base_url();

    // Menu cache'den menü item'larını al (dil belirlendikten sonra)
    if let Ok(menu_cache) = state.menu_cache.read() {
        global_context.menu_items = menu_cache.get(&global_context.current_language);
    }

    // Para birimi tercihi: Session > Cookie > sale_currency
    let sale_currency = global_context.settings.get_sale_currency();
    let supported_codes = global_context.settings.get_supported_currencies();

    // Session'dan display_currency tercihini al
    let session_currency = session
        .get::<String>("display_currency")
        .await
        .unwrap_or(None);

    // Cookie'den display_currency tercihini al (session yoksa)
    let cookie_currency = request
        .headers()
        .get("cookie")
        .and_then(|cookie_header| cookie_header.to_str().ok())
        .and_then(|cookie_str| {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if cookie.starts_with("display_currency=") {
                    let cur = &cookie[17..]; // "display_currency=" kısmını atla
                    if supported_codes.contains(&cur.to_string()) {
                        return Some(cur.to_string());
                    }
                }
            }
            None
        });

    // Öncelik: session > cookie > sale_currency
    let display_currency = session_currency
        .or(cookie_currency)
        .unwrap_or_else(|| sale_currency.clone());

    // Desteklenen para birimleri detay listesi (template dropdown için)
    let currency_details_fn = |code: &str, active_code: &str| -> CurrencyDisplayInfo {
        let (name, symbol, flag) = match code {
            "TRY" => ("Türk Lirası", "₺", "🇹🇷"),
            "USD" => ("US Dollar", "$", "🇺🇸"),
            "EUR" => ("Euro", "€", "🇪🇺"),
            "GBP" => ("British Pound", "£", "🇬🇧"),
            "CHF" => ("Swiss Franc", "CHF", "🇨🇭"),
            "AUD" => ("Australian Dollar", "A$", "🇦🇺"),
            "CAD" => ("Canadian Dollar", "C$", "🇨🇦"),
            "AZN" => ("Azerbaycan Manatı", "₼", "🇦🇿"),
            "JPY" => ("Japanese Yen", "¥", "🇯🇵"),
            _ => (code, code, "🏳️"),
        };
        CurrencyDisplayInfo {
            code: code.to_string(),
            name: name.to_string(),
            symbol: symbol.to_string(),
            flag: flag.to_string(),
            is_active: code == active_code,
        }
    };

    global_context.display_currency = display_currency.clone();
    global_context.supported_currencies = supported_codes
        .iter()
        .map(|code| currency_details_fn(code, &display_currency))
        .collect();

    // Global context içeriklerini cache'den al
    if let Ok(global_cache) = state.global_context_cache.read() {
        request.extensions_mut().insert(global_cache.clone());
    } else {
        // Cache okunamazsa boş BTreeMap ekle
        request
            .extensions_mut()
            .insert(std::collections::BTreeMap::<String, serde_json::Value>::new());
    }

    // Add user_id as extension for backward compatibility
    // controllerda böyle Extension(user_id): Extension<Option<i64>>
    // html render controller
    request.extensions_mut().insert(user_id);

    // Add current_language as extension for API functions
    request
        .extensions_mut()
        .insert(CurrentLanguage(global_context.current_language.clone()));

    request.extensions_mut().insert(global_context);

    next.run(request).await
}

// Custom Extractor for Tera Context
pub struct ViewContext(pub tera::Context); // tera templat econtext artık ViewContext içinde

impl<S> FromRequestParts<S> for ViewContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let mut context = tera::Context::new();

        if let Some(global_data) = parts.extensions.get::<GlobalContext>() {
            if let Some(session_data) = &global_data.session_data {
                context.insert("session_data", session_data);
                // context.insert("user_permissions", &session_data.permissions);
                // context.insert("user_profile", &session_data.profile);
            }
            context.insert("cart_count", &global_data.cart_count);
            context.insert(
                "free_shipping_threshold",
                &global_data.free_shipping_threshold,
            );
            context.insert(
                "free_shipping_threshold_formatted",
                &global_data.free_shipping_threshold_formatted,
            );
            context.insert("cart_total_amount", &global_data.cart_total_amount);
            context.insert(
                "cart_total_amount_formatted",
                &global_data.cart_total_amount_formatted,
            );
            context.insert(
                "bookmark_product_count",
                &global_data.bookmark_product_count,
            );

            context.insert("has_admin_access", &global_data.has_admin_access);
            context.insert("has_b2b_access", &global_data.has_b2b_access);
            context.insert("is_authenticated", &global_data.is_authenticated);
            context.insert("is_guest", &global_data.is_guest);
            context.insert("supported_languages", &global_data.supported_languages);
            context.insert("current_language", &global_data.current_language);
            context.insert("settings", &global_data.settings);
            context.insert("menu_items", &global_data.menu_items);
            context.insert("debug", &global_data.debug);
            context.insert("display_currency", &global_data.display_currency);
            context.insert(
                "supported_currencies_list",
                &global_data.supported_currencies,
            );
            context.insert("base_url", &global_data.base_url);
        }

        // Global context içeriklerini ekle
        if let Some(global_contents) = parts
            .extensions
            .get::<std::collections::BTreeMap<String, serde_json::Value>>()
        {
            context.insert("global", global_contents);
        }

        Ok(ViewContext(context))
    }
}

// Custom Extractor for Current Language in API functions
impl<S> FromRequestParts<S> for CurrentLanguage
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<CurrentLanguage>().cloned().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Current language not found",
        ))
    }
}

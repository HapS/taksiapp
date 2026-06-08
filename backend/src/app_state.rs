use crate::config::app_config::AppConfig;
use crate::i18n::I18n;
use crate::middleware::global_context::{MenuCache, SettingsCache};
use crate::modules::ride::ws::hub::Hub;
use rust_embed::RustEmbed;
use sea_orm::DatabaseConnection;
use std::sync::{Arc, Mutex, RwLock};
use tera::Error as TeraError;
use tera::Tera;
use tracing::{debug, error, info, warn};

#[derive(RustEmbed)]
#[folder = "templates/admin/"]
struct AdminTemplates;

#[derive(RustEmbed)]
#[folder = "templates/admin/static/"]
#[allow(dead_code)]
pub struct AdminStatic;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub tera: Arc<Mutex<Tera>>,
    pub admin_tera: Arc<Mutex<Tera>>, // Cached admin templates
    pub config: AppConfig,
    pub i18n: I18n,
    pub settings_cache: Arc<RwLock<SettingsCache>>,
    pub menu_cache: Arc<RwLock<MenuCache>>,
    pub global_context_cache: Arc<RwLock<std::collections::BTreeMap<String, serde_json::Value>>>,
    pub media_locks: Arc<dashmap::DashMap<std::path::PathBuf, Arc<tokio::sync::Mutex<()>>>>,
    pub i18n_map: Arc<RwLock<std::collections::HashMap<String, I18n>>>, // Theme-specific i18n instances

    // Thumbnail generation controls
    pub thumbnail_semaphore: Arc<tokio::sync::Semaphore>,
    pub thumbnail_cache_max_bytes: u64,

    // Ride module
    pub hub: Arc<Hub>,
    pub redis: Arc<redis::aio::ConnectionManager>,
}

impl axum::extract::FromRef<AppState> for DatabaseConnection {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}

impl AppState {
    pub fn new(
        db: DatabaseConnection,
        tera: Tera,
        config: AppConfig,
        i18n: I18n,
        settings_cache: SettingsCache,
        menu_cache: MenuCache,
        global_context_cache: std::collections::BTreeMap<String, serde_json::Value>,
        redis: redis::aio::ConnectionManager,
    ) -> Self {
        let tera_instance = tera;

        // Create and cache admin Tera instance
        let admin_tera = Self::create_admin_tera();

        // Pre-load i18n instances for all themes in templates folder
        let mut i18n_map = std::collections::HashMap::new();

        // Always include admin
        i18n_map.insert("admin".to_string(), I18n::with_theme("admin"));

        // Scan templates folder for themes
        if let Ok(entries) = std::fs::read_dir("templates") {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Some(dir_name) = entry.file_name().to_str() {
                            // Skip admin
                            if dir_name == "admin" {
                                continue;
                            }
                            // Check if theme has base.html
                            let base_html_path = format!("templates/{}/base.html", dir_name);
                            if std::path::Path::new(&base_html_path).exists() {
                                if !i18n_map.contains_key(dir_name) {
                                    i18n_map
                                        .insert(dir_name.to_string(), I18n::with_theme(dir_name));
                                }
                            }
                        }
                    }
                }
            }
        }

        Self {
            db,
            tera: Arc::new(Mutex::new(tera_instance)),
            admin_tera: Arc::new(Mutex::new(admin_tera)),
            config,
            i18n,
            settings_cache: Arc::new(RwLock::new(settings_cache)),
            menu_cache: Arc::new(RwLock::new(menu_cache)),
            global_context_cache: Arc::new(RwLock::new(global_context_cache)),
            media_locks: Arc::new(dashmap::DashMap::new()),
            i18n_map: Arc::new(RwLock::new(i18n_map)),
            thumbnail_semaphore: Arc::new(tokio::sync::Semaphore::new(4)), // default concurrency
            thumbnail_cache_max_bytes: 1024 * 1024 * 1024,                 // 1GB default
            hub: Arc::new(Hub::new()),
            redis: Arc::new(redis),
        }
    }

    /// Get i18n instance for a specific theme
    pub fn get_i18n(&self, theme: &str) -> I18n {
        if let Ok(i18n_map) = self.i18n_map.read() {
            if let Some(i18n) = i18n_map.get(theme) {
                return i18n.clone();
            }
        }
        // Fallback to default i18n
        self.i18n.clone()
    }

    /// Create admin Tera instance with all embedded templates pre-loaded
    fn create_admin_tera() -> Tera {
        let mut admin_tera = Tera::default();

        // Load all embedded admin templates into Tera
        let mut embedded_templates: Vec<(String, String)> = Vec::new();
        for file_path in AdminTemplates::iter() {
            if let Some(file) = AdminTemplates::get(&file_path) {
                if let Ok(content) = std::str::from_utf8(&file.data) {
                    let template_key = format!("admin/{}", file_path);
                    embedded_templates.push((template_key, content.to_string()));
                }
            }
        }

        embedded_templates.sort_by_key(|(key, _)| {
            if key.ends_with("/base.html") {
                0
            } else if key.ends_with("/settings/layout.html") || key.ends_with("/layout.html") {
                1
            } else {
                2
            }
        });

        for (template_key, content) in embedded_templates {
            if let Err(e) = admin_tera.add_raw_template(&template_key, &content) {
                warn!(
                    template = %template_key,
                    error = %e,
                    "Gömülü admin şablonu eklenemedi"
                );
            }
        }

        admin_tera
    }

    //aktif tema adını al, tema yoksa silinmiş veya başka bir şey olmuşsa "base" döner
    pub fn get_frontend_theme(&self) -> String {
        if let Ok(settings_cache) = self.settings_cache.read() {
            let theme = settings_cache
                .frontend_theme
                .clone()
                .unwrap_or_else(|| "base".to_string());
            debug!(theme = %theme, "Ön yüz teması önbellekten alındı");
            theme
        } else {
            warn!("Ayarlar önbelleği okunamadı, base tema kullanılacak");
            "base".to_string()
        }
    }

    /// Build template path with active theme
    pub fn build_template_path(&self, template_path: &str) -> String {
        let theme = self.get_frontend_theme();

        // admin panel için admin ile başlayan renderlar için admin/falan/filan
        if template_path.starts_with("admin/") {
            return template_path.to_string();
        }

        // admin değil frontend için render edilecek template'ler
        // Eğer template_path zaten tema klasöründen başlıyorsa (base/home/index.html gibi)
        let first_segment = template_path.split('/').next().unwrap_or("");
        if first_segment == "base" {
            return template_path.to_string();
        }

        // Eğer template_path doğrudan templates/ klasöründe bir dizini işaret ediyorsa (örneğin "pages/home.html")
        let candidate_dir = std::path::Path::new("templates").join(first_segment);
        if candidate_dir.is_dir() {
            return template_path.to_string();
        }

        // Otherwise prefix with the active theme
        format!("{}/{}", theme, template_path)
    }

    pub fn render_frontend_template(
        &self,
        template_inner: &str,
        context: &tera::Context,
    ) -> Result<String, TeraError> {
        let inner = template_inner;
        let theme = self.get_frontend_theme();
        let template_name = format!("{}/{}", theme, inner);

        // Inject theme name into context for i18n
        let mut enriched_context = context.clone();
        enriched_context.insert("current_theme", &theme);

        self.render_template(&template_name, &enriched_context)
    }

    /// Hot reload kapalıyken: Direkt cache'den render (ÇOK HIZLI ~1-5ms)
    /// Hot reload açıkken: Sadece değişen template'i reload et (ORTA ~15-30ms)
    pub fn render_template(
        &self,
        template_name: &str,
        context: &tera::Context,
    ) -> Result<String, TeraError> {
        // Build themed template path
        let themed_template_path = self.build_template_path(template_name);

        let config = crate::config::get_config();
        let is_debug = config.is_debug();

        let start = if is_debug {
            Some(std::time::Instant::now())
        } else {
            None
        };

        if is_debug {
            debug!(template = template_name, themed_path = %themed_template_path, "Şablon render başlatılıyor");
        }

        // Context validation (sadece debug modda)
        if is_debug {
            debug!(template = template_name, "Şablon context hazırlandı");
        }

        let final_result = self.render_template_internal(&themed_template_path, context);

        // Log sadece debug modda
        if is_debug {
            match &final_result {
                Ok(_) => {
                    if let Some(start_time) = start {
                        let duration = start_time.elapsed();
                        info!(
                            template = template_name,
                            themed_path = %themed_template_path,
                            duration_ms = duration.as_millis(),
                            "Şablon başarıyla render edildi"
                        );
                    }
                }
                Err(e) => {
                    error!(
                        template = template_name,
                        themed_path = %themed_template_path,
                        error = %e,
                        error_debug = ?e,
                        "Şablon render işlemi başarısız"
                    );
                    warn!(
                        template = template_name,
                        themed_path = %themed_template_path,
                        error = %e,
                        "Şablon render başarısız (debug detayları)"
                    );
                }
            }
        } else {
            // Production'da sadece error'ları logla
            if let Err(e) = &final_result {
                error!(
                    template = template_name,
                    themed_path = %themed_template_path,
                    error = %e,
                    "Şablon render işlemi başarısız"
                );
            }
        }

        final_result
    }

    /// Internal template rendering logic
    fn render_template_internal(
        &self,
        template_name: &str,
        context: &tera::Context,
    ) -> Result<String, TeraError> {
        let config = crate::config::get_config();

        // Theme'i belirle ve thread-local i18n'i ayarla
        let theme = if template_name.starts_with("admin/") {
            "admin".to_string()
        } else {
            template_name
                .split('/')
                .next()
                .unwrap_or("base")
                .to_string()
        };

        let theme_i18n = self.get_i18n(&theme);
        crate::i18n::set_current_theme_i18n(theme_i18n);

        // Artık menü verileri global_context_middleware tarafından ekleniyor
        // Her render'da DB sorgusu yok! Cache'den geliyor.
        let enriched_context = context.clone();

        // Admin templates are embedded, handle them separately
        if template_name.starts_with("admin/") {
            return self.render_embedded_admin_template(template_name, &enriched_context);
        }

        if config.template_hot_reload {
            // Hot reload: Sadece değişen dosyaları yükle
            let mut tera = match self.tera.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    warn!("Tera kilidi bozulmuş; iç değerle devam ediliyor");
                    poisoned.into_inner()
                }
            };

            // template_name zaten themed path (base/home/index.html gibi)
            let template_file_path = format!("templates/{}", template_name);

            // Ana template'i yükle
            if let Ok(content) = std::fs::read_to_string(&template_file_path) {
                info!(
                    "🔥 Hot-reload: {} dosyası yeniden okundu!",
                    template_file_path
                );
                let theme_name = template_name.split('/').next().unwrap_or("base");
                let processed_content = content.replace("@@theme@@", theme_name);
                if let Err(e) = tera.add_raw_template(template_name, &processed_content) {
                    warn!(
                        template = template_name,
                        error = %e,
                        "Ana şablon hot-reload edilemedi"
                    );
                }
            } else {
                error!(
                    "❌ Hot-reload hatası: {} dosyası okunamadı!",
                    template_file_path
                );
            }

            // _partials klasöründeki dosyaları da yükle (navbar, footer vb.)
            let theme = template_name.split('/').next().unwrap_or("base");
            let partials_path = format!("templates/{}/_partials", theme);
            if let Ok(entries) = std::fs::read_dir(&partials_path) {
                for entry in entries.flatten() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.ends_with(".html") {
                            let partial_template_name =
                                format!("{}/_partials/{}", theme, file_name);
                            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                                let processed_content = content.replace("@@theme@@", theme);
                                if let Err(e) = tera
                                    .add_raw_template(&partial_template_name, &processed_content)
                                {
                                    warn!(
                                        template = %partial_template_name,
                                        error = %e,
                                        "Partial şablon hot-reload edilemedi"
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // base.html'i de yükle (extends için)
            let base_path = format!("templates/{}/base.html", theme);
            if let Ok(content) = std::fs::read_to_string(&base_path) {
                let base_template_name = format!("{}/base.html", theme);
                let processed_content = content.replace("@@theme@@", theme);
                if let Err(e) = tera.add_raw_template(&base_template_name, &processed_content) {
                    warn!(
                        template = %base_template_name,
                        error = %e,
                        "Base şablon hot-reload edilemedi"
                    );
                }
            }

            tera.render(template_name, &enriched_context)
        } else {
            // Hot reload kapalı - sadece cache'den render (ÇOK HIZLI)
            let tera = match self.tera.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    warn!("Tera kilidi bozulmuş; iç değerle devam ediliyor");
                    poisoned.into_inner()
                }
            };
            tera.render(template_name, &enriched_context)
        }
    }

    /// Render embedded admin templates (FAST - uses cached Tera instance)
    fn render_embedded_admin_template(
        &self,
        template_name: &str,
        context: &tera::Context,
    ) -> Result<String, TeraError> {
        // Inject global settings into admin template context (from cache)
        let mut enriched_context = context.clone();
        if let Ok(settings) = self.settings_cache.read() {
            enriched_context.insert("admin_settings", &*settings);
        } else {
            warn!("Admin şablon render sırasında ayarlar önbelleği okunamadı");
        }

        // Inject theme for i18n
        enriched_context.insert("current_theme", &"admin".to_string());

        // admin navbar için vocabulary_id ve vocabulary_type ekle  (boş da olsa) çünküüü diğer fonksiyonlarda bu variablellar kullanılmıyor ve navbar patlıyor
        //active class için attığım bir takla burada atılıyor

        if enriched_context.get("vocabulary_id").is_none() {
            enriched_context.insert("vocabulary_id", &Option::<i64>::None);
        }
        if enriched_context.get("vocabulary_type").is_none() {
            enriched_context.insert("vocabulary_type", &"".to_string());
        }

        // Use cached admin Tera instance - NO template loading overhead!
        let admin_tera = match self.admin_tera.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                warn!("Admin Tera kilidi bozulmuş; iç değerle devam ediliyor");
                poisoned.into_inner()
            }
        };

        admin_tera.render(template_name, &enriched_context)
    }

    /// Read template source from disk or embedded admin templates (if present).
    /// Returns `Some(String)` with the template content or `None` if not found.
    pub fn read_template_source(&self, template_name: &str) -> Option<String> {
        // First try the templates directory on disk
        let disk_path = std::path::Path::new("templates").join(template_name);
        if disk_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&disk_path) {
                return Some(content);
            }
        }

        // Fallback to embedded admin templates (if applicable)
        if let Some(stripped) = template_name.strip_prefix("admin/") {
            if let Some(file) = AdminTemplates::get(stripped) {
                if let Ok(content) = std::str::from_utf8(&file.data) {
                    return Some(content.to_string());
                }
            }
        }

        None
    }
}

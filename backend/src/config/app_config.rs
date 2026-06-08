// Config - TOML dosyasından uygulama ayarlarını okur, diğer modüllere eski interface ile sunar
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

static CONFIG: OnceLock<TomlConfig> = OnceLock::new();

// TOML dosyası yapısı (internal)
#[derive(Debug, Clone, Deserialize)]
struct TomlConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    template: TemplateConfig,
    paths: PathsConfig,
    session: SessionConfig,
    jwt: JwtConfig,
    languages: LanguagesConfig,
    logging: LoggingConfig,
    #[allow(dead_code)]
    security: SecurityConfig,
    media: MediaConfig,
    oauth: OAuthConfig,
    #[serde(default)]
    modules: ModulesConfig,
    #[serde(default)]
    campaign: CampaignConfig,
    #[serde(default)]
    ors: Option<OrsConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrsConfig {
    pub api_key: String,
    pub base_url: String,
}

impl Default for OrsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openrouteservice.org".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
    #[serde(default = "default_debug")]
    debug: bool,
    #[serde(default = "default_base_url")]
    base_url: String,
    #[serde(default = "default_ssl")]
    ssl: bool,
}

fn default_debug() -> bool {
    true // Default to debug mode for safety
}

fn default_base_url() -> String {
    "http://localhost:3000".to_string() // Default to localhost
}

fn default_ssl() -> bool {
    false // Default to HTTP for development
}

#[derive(Debug, Clone, Deserialize)]
struct DatabaseConfig {
    url: String,
    #[allow(dead_code)]
    max_connections: u32,
    #[allow(dead_code)]
    min_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct TemplateConfig {
    hot_reload: bool,
    path: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PathsConfig {
    static_dir: String,
    media_dir: String,
    #[allow(dead_code)]
    upload_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SessionConfig {
    max_age: u64,
    #[allow(dead_code)]
    secret_key: String,
}

#[derive(Debug, Clone, Deserialize)]
struct JwtConfig {
    secret: String,
    access_token_expiry: u64,
    refresh_token_expiry: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct LanguagesConfig {
    default: String,
    #[allow(dead_code)]
    supported: Vec<String>,
    names: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LoggingConfig {
    #[allow(dead_code)]
    level: String,
    ignore_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct SecurityConfig {
    cors_enabled: bool,
    allowed_origins: Vec<String>,
    rate_limit_enabled: bool,
    rate_limit_requests: u32,
    rate_limit_window_seconds: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthConfig {
    pub google: Option<OAuthClientConfig>,
    pub apple: Option<OAuthClientConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthClientConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MediaConfig {
    upload_root: String,
    max_file_size: u64,
    allowed_image_types: Vec<String>,
    allowed_video_types: Vec<String>,
    allowed_audio_types: Vec<String>,
    allowed_document_types: Vec<String>,
}

// Public API - ESKİ INTERFACE (backward compatible)
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub supported_languages: HashMap<String, String>,
    pub default_language: String,
    pub template_hot_reload: bool,
    // Yeni alanlar (internal)
    toml: TomlConfig,
    pub modules: ModulesConfig,
    pub campaign_config: CampaignConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModulesConfig {
    #[serde(default = "default_b2b")]
    pub b2b: bool,
    #[serde(default = "default_b2c")]
    pub b2c: bool,
}

impl Default for ModulesConfig {
    fn default() -> Self {
        Self {
            b2b: default_b2b(),
            b2c: default_b2c(),
        }
    }
}

fn default_b2b() -> bool {
    false
}

fn default_b2c() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct CampaignConfig {
    #[serde(default = "default_campaign_dry_run")]
    pub dry_run: bool,
}

fn default_campaign_dry_run() -> bool {
    true
}

impl Default for CampaignConfig {
    fn default() -> Self {
        Self {
            dry_run: default_campaign_dry_run(),
        }
    }
}

impl AppConfig {
    fn from_toml(toml: TomlConfig) -> Self {
        Self {
            database_url: toml.database.url.clone(),
            supported_languages: toml.languages.names.clone(),
            default_language: toml.languages.default.clone(),
            template_hot_reload: toml.template.hot_reload,
            modules: ModulesConfig {
                b2b: toml.modules.b2b,
                b2c: toml.modules.b2c,
            },
            campaign_config: CampaignConfig {
                dry_run: toml.campaign.dry_run,
            },
            toml,
        }
    }

    pub fn is_language_supported(&self, lang: &str) -> bool {
        self.supported_languages.contains_key(lang)
    }

    pub fn get_language_or_default(&self, lang: Option<&str>) -> String {
        match lang {
            Some(l) if self.is_language_supported(l) => l.to_string(),
            _ => self.default_language.clone(),
        }
    }

    /// Return supported language codes with `tr` ensured as first element.
    /// Uses the `languages.supported` vec from TOML to preserve configured order when available.
    pub fn ordered_languages(&self) -> Vec<String> {
        // Start with configured order (if present)
        let mut langs: Vec<String> = Vec::new();

        if !self.toml.languages.supported.is_empty() {
            for code in &self.toml.languages.supported {
                if self.supported_languages.contains_key(code) {
                    langs.push(code.clone());
                }
            }
        } else {
            // Fallback: keys of the names map (HashMap order is not guaranteed)
            langs.extend(self.supported_languages.keys().cloned());
        }

        // Ensure 'tr' is first. If missing, insert it at the beginning.
        if let Some(pos) = langs.iter().position(|l| l == "tr") {
            if pos != 0 {
                let tr = langs.remove(pos);
                langs.insert(0, tr);
            }
        } else {
            langs.insert(0, "tr".to_string());
        }

        langs
    }

    // Yeni helper metodlar
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.toml.server.host, self.toml.server.port)
    }

    pub fn static_dir(&self) -> &str {
        &self.toml.paths.static_dir
    }

    pub fn media_dir(&self) -> &str {
        &self.toml.paths.media_dir
    }

    pub fn template_path(&self) -> &str {
        &self.toml.template.path
    }

    pub fn session_max_age(&self) -> u64 {
        self.toml.session.max_age
    }

    pub fn should_log_path(&self, path: &str) -> bool {
        !self
            .toml
            .logging
            .ignore_paths
            .iter()
            .any(|ignore_path| path.contains(ignore_path))
    }

    // Media config helpers
    pub fn media_upload_root(&self) -> &str {
        &self.toml.media.upload_root
    }

    pub fn media_max_file_size(&self) -> u64 {
        self.toml.media.max_file_size
    }

    pub fn is_allowed_mime_type(&self, mime_type: &str) -> bool {
        self.toml
            .media
            .allowed_image_types
            .contains(&mime_type.to_string())
            || self
                .toml
                .media
                .allowed_video_types
                .contains(&mime_type.to_string())
            || self
                .toml
                .media
                .allowed_audio_types
                .contains(&mime_type.to_string())
            || self
                .toml
                .media
                .allowed_document_types
                .contains(&mime_type.to_string())
    }

    pub fn get_media_type_from_mime(&self, mime_type: &str) -> &str {
        if self
            .toml
            .media
            .allowed_image_types
            .contains(&mime_type.to_string())
        {
            "image"
        } else if self
            .toml
            .media
            .allowed_video_types
            .contains(&mime_type.to_string())
        {
            "video"
        } else if self
            .toml
            .media
            .allowed_audio_types
            .contains(&mime_type.to_string())
        {
            "audio"
        } else if self
            .toml
            .media
            .allowed_document_types
            .contains(&mime_type.to_string())
        {
            "document"
        } else {
            "other"
        }
    }

    /// Check if debug mode is enabled (shows detailed error pages)
    ///
    /// When true: Shows detailed error pages with stack traces (development)
    /// When false: Shows generic error pages (production)
    pub fn is_debug(&self) -> bool {
        self.toml.server.debug
    }

    // JWT config helpers
    pub fn jwt_secret(&self) -> &str {
        &self.toml.jwt.secret
    }

    pub fn jwt_access_token_expiry(&self) -> u64 {
        self.toml.jwt.access_token_expiry
    }

    pub fn jwt_refresh_token_expiry(&self) -> u64 {
        self.toml.jwt.refresh_token_expiry
    }

    /// Get base URL for the application (for payment callbacks, etc.)
    pub fn get_base_url(&self) -> String {
        self.toml.server.base_url.clone()
    }

    pub fn oauth(&self) -> &OAuthConfig {
        &self.toml.oauth
    }

    /// Check if SSL is enabled (for cookie security settings)
    ///
    /// When true: Cookies will be secure (HTTPS only)
    /// When false: Cookies will not be secure (HTTP allowed)
    pub fn is_ssl_enabled(&self) -> bool {
        self.toml.server.ssl
    }

    pub fn modules(&self) -> &ModulesConfig {
        &self.toml.modules
    }

    pub fn campaign_dry_run(&self) -> bool {
        self.campaign_config.dry_run
    }

    pub fn ors_config(&self) -> Option<&OrsConfig> {
        self.toml.ors.as_ref()
    }

    /// Startup'ta çağrılır — ORS config'in yüklenip yüklenmediğini loglar
    pub fn log_config_status(&self) {
        match &self.toml.ors {
            Some(ors) => {
                let masked_key = if ors.api_key.len() > 8 {
                    format!("{}...", &ors.api_key[..8])
                } else {
                    "***".to_string()
                };
                tracing::info!(url = %ors.base_url, key = %masked_key, "ORS config yüklendi");
            }
            None => {
                tracing::warn!("ORS config bulunamadı (config.toml'da [ors] bölümü eksik)");
                tracing::info!("OSRM public API kullanılacak (router.project-osrm.org)");
            }
        }
    }
}

fn load_toml() -> TomlConfig {
    match std::fs::read_to_string("config.toml") {
        Ok(content) => match toml::from_str(&content) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("❌ Config ayrıştırma hatası: {}", e);
                eprintln!("💡 Varsayılan ayarlar kullanılıyor...");
                default_toml_config()
            }
        },
        Err(_) => {
            eprintln!("⚠️  config.toml bulunamadı, varsayılan ayarlar kullanılıyor");
            default_toml_config()
        }
    }
}

fn default_toml_config() -> TomlConfig {
    TomlConfig {
        server: ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 3000,
            debug: true,
            base_url: "http://localhost:3000".to_string(),
            ssl: false,
        },
        database: DatabaseConfig {
            url: "postgresql://postgres:as45dfck@localhost:5432/backend_rs".to_string(),
            max_connections: 10,
            min_connections: 2,
        },
        template: TemplateConfig {
            hot_reload: true,
            path: "templates/**/*.html".to_string(),
        },
        paths: PathsConfig {
            static_dir: "static".to_string(),
            media_dir: "media".to_string(),
            upload_dir: "media/uploads".to_string(),
        },
        session: SessionConfig {
            max_age: 2592000,
            secret_key: "your-secret-key-change-this".to_string(),
        },
        jwt: JwtConfig {
            secret: "your-super-secret-jwt-key-change-this-in-production".to_string(),
            access_token_expiry: 3600,
            refresh_token_expiry: 2592000,
        },
        languages: LanguagesConfig {
            default: "tr".to_string(),
            supported: vec!["tr".to_string(), "en".to_string()],
            names: {
                let mut map = HashMap::new();
                map.insert("tr".to_string(), "Türkçe".to_string());
                map.insert("en".to_string(), "English".to_string());
                map
            },
        },
        logging: LoggingConfig {
            level: "info".to_string(),
            ignore_paths: vec!["/favicon.ico".to_string(), "/static/".to_string()],
        },
        security: SecurityConfig {
            cors_enabled: true,
            allowed_origins: vec!["*".to_string()],
            rate_limit_enabled: false,
            rate_limit_requests: 100,
            rate_limit_window_seconds: 60,
        },
        media: MediaConfig {
            upload_root: "media/uploads".to_string(),
            max_file_size: 10485760,
            allowed_image_types: vec![
                "image/jpeg".to_string(),
                "image/png".to_string(),
                "image/gif".to_string(),
                "image/webp".to_string(),
                "image/svg+xml".to_string(),
            ],
            allowed_video_types: vec![
                "video/mp4".to_string(),
                "video/webm".to_string(),
                "video/ogg".to_string(),
            ],
            allowed_audio_types: vec![
                "audio/mpeg".to_string(),
                "audio/wav".to_string(),
                "audio/ogg".to_string(),
            ],
            allowed_document_types: vec![
                "application/pdf".to_string(),
                "application/msword".to_string(),
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            ],
        },
        oauth: OAuthConfig {
            google: None,
            apple: None,
        },
        modules: ModulesConfig {
            b2b: false,
            b2c: true,
        },
        campaign: CampaignConfig {
            dry_run: true,
        },
        ors: None,
    }
}

// ESKİ API - Hiçbir şey değişmedi
pub fn get_config() -> AppConfig {
    let toml = CONFIG.get_or_init(|| load_toml());
    AppConfig::from_toml(toml.clone())
}

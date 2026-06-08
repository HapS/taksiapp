use crate::middleware::global_context::ViewContext;

// Compile-time i18n for t! macro in Rust code
rust_i18n::i18n!("locales");

mod app_state;
mod config;
mod i18n;
mod macros;
mod middleware;
mod modules;
mod tera_custom_filters;
mod tera_custom_functions;
mod version;
// use std::sync::Arc;

use crate::modules::media::controllers::web::media::thumbnail_handler;
use app_state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    http::HeaderMap,
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use colored::*;
use config::{get_config, DatabaseConfig};
use middleware::logger::LoggerLayer;
use migration::{Migrator, MigratorTrait};
use modules::auth::session_store::SeaOrmSessionStore;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_sessions::SessionManagerLayer;
// Root redirect - önce cookie, sonra tarayıcı diline göre yönlendir
async fn root_redirect(
    headers: HeaderMap,
    jar: axum_extra::extract::CookieJar,
) -> impl IntoResponse {
    let config = get_config();

    // 1. Önce cookie'deki dili kontrol et
    if let Some(cookie_lang) = jar.get("language") {
        let lang = cookie_lang.value();
        if config.is_language_supported(lang) {
            return Redirect::to(&format!("/{}", lang));
        }
    }

    // 2. Cookie yoksa Accept-Language header'ından dil algıla
    let detected_lang = if let Some(accept_lang) = headers.get("accept-language") {
        if let Ok(accept_lang_str) = accept_lang.to_str() {
            let mut found_lang = None;
            for lang_part in accept_lang_str.split(',') {
                let lang_code = lang_part
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .split('-')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

                if lang_code == "api" || lang_code == "favicon.ico" || lang_code == "admin" {
                    continue;
                }

                if config.is_language_supported(&lang_code) {
                    found_lang = Some(lang_code);
                    break;
                }
            }
            found_lang.unwrap_or_else(|| config.default_language.clone())
        } else {
            config.default_language.clone()
        }
    } else {
        config.default_language.clone()
    };

    Redirect::to(&format!("/{}", detected_lang))
}

async fn handle_api_root() -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_FOUND
}

// Robots.txt handler
async fn robots_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    use axum::http::{header, StatusCode};

    let robots_content = match state.settings_cache.read() {
        Ok(settings) => settings
            .robots
            .clone()
            .unwrap_or_else(|| "User-agent: *\nAllow: /".to_string()),
        Err(_) => "User-agent: *\nAllow: /".to_string(),
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        robots_content,
    )
}

// 404 Not Found fallback
async fn not_found_fallback(
    axum::extract::State(state): axum::extract::State<AppState>,
    mut context: ViewContext,
) -> axum::response::Response {
    use axum::response::{Html, IntoResponse};
    context.0.insert("title", "Sayfa Bulunamadı (404)");
    context.0.insert("request_path", "/404");

    let theme = state.get_frontend_theme();
    let error_template = format!("{}/errors/404.html", theme);

    match state.render_template(&error_template, &context.0) {
        Ok(html) => (axum::http::StatusCode::NOT_FOUND, Html(html)).into_response(),
        Err(e) => {
            // If debug is enabled, show the detailed Tera error page (raw details).
            // Otherwise fall back to the simple 404 page for production safety.
            if state.config.is_debug() {
                return crate::middleware::error_handler::handle_template_error_with_context(
                    &e,
                    state.config.is_debug(),
                    false,
                    Some(&state),
                );
            }

            // Fallback to simple 404 if template fails (production)
            (
                axum::http::StatusCode::NOT_FOUND,
                Html(
                    r#"
                    <!DOCTYPE html>
                    <html>
                    <head><title>404 Not Found</title></head>
                    <body style="font-family: sans-serif; text-align: center; padding: 50px;">
                        <h1>404</h1>
                        <p>Page Not Found</p>
                        <a href="/">Go Home</a>
                    </body>
                    </html>
                    "#,
                ),
            )
                .into_response()
        }
    }
}

use tokio::signal;
//güvenli kapatma durdurma sinyali, mevcut istekler işlenir yeni istekler kabul edilmez
async fn shutdown_signal() {
    signal::ctrl_c().await.unwrap();
    println!("Shutting down gracefully");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --version argümanını kontrol et
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Initialize tracing with better formatting
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // Default log levels
                "backend_rs=debug,tower_http=debug,sea_orm=info".into()
            }),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true),
        )
        .init();

    // Load configuration
    let config = get_config();
    config.log_config_status();

    // Connect to database
    let db = DatabaseConfig::connect(&config.database_url).await?;
    println!("🍀 Veritabanı bağlantısı kuruldu");

    // Redis bağlantısı
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_client = redis::Client::open(redis_url.as_str())?;
    let redis_manager = redis::aio::ConnectionManager::new(redis_client).await?;
    println!("🍀 Redis bağlantısı kuruldu");

    // Run migrations
    Migrator::up(&db, None).await?;
    println!("🍀 Veritabanı migration'ları tamamlandı");

    // Check mail templates (development only)
    if config.is_debug() {
        if let Err(e) = modules::mailer::SeedService::check_templates() {
            eprintln!("⚠️  Mail template kontrol hatası: {}", e);
        }
    }

    println!("🎃 B2B Durumu {}", config.modules().b2b);
    println!("🎃 B2C Durumu {}", config.modules().b2c);

    // Initialize Tera template engine manually to support @@theme@@ placeholder
    let mut tera = tera::Tera::default();

    let template_glob = config.template_path(); // e.g., "templates/**/*.html" glob kullanmadan önce template dosyaları..
    let mut templates = Vec::new();

    match glob::glob(template_glob) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if entry.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&entry) {
                        // Normalize path and get relative path after "templates/" folder more robustly
                        let path_str = entry.to_string_lossy().replace('\\', "/");
                        let template_name = if let Some(pos) = path_str.find("templates/") {
                            &path_str[pos + 10..]
                        } else {
                            path_str.as_ref()
                        };

                        if template_name.is_empty() {
                            continue;
                        }

                        // Determine theme from the first segment of the path
                        let theme_name = template_name.split('/').next().unwrap_or("base"); //tema yoksa default base temayı kullan

                        // Admin template release dosyanın içine gömüldüğü için admin template'lerini diskten okumadan atla
                        if theme_name == "admin" {
                            continue;
                        }

                        let processed_content = content.replace("@@theme@@", theme_name);
                        templates.push((template_name.to_string(), processed_content));
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Template glob error: {}", e);
            std::process::exit(1);
        }
    }

    // Add all templates at once to ensure inheritance relations are resolved correctly
    if let Err(e) = tera.add_raw_templates(templates) {
        eprintln!("⚠️  Template yükleme sırasında bazı hatalar oluştu: {}", e);
        // Not: Bazı template'ler yüklenememiş olabilir ama uygulama çalışmaya devam eder.
    }

    // Configure template engine
    tera.autoescape_on(vec![".html"]);

    // Initialize runtime i18n
    let i18n = i18n::I18n::new();

    // Register custom Tera functions
    crate::tera_custom_functions::register_functions(&mut tera, i18n.clone());

    // Register custom Tera filters from module
    crate::tera_custom_filters::register_filters(&mut tera);

    // Check loaded locales for debugging
    println!("🌍 Yüklü Diller: {:?}", i18n.available_locales());

    // Show hot reload status
    if config.template_hot_reload {
        println!("🍀 Template'ler yüklendi (🔥 hot reload: AKTİF)");
    } else {
        println!("🍀 Template'ler yüklendi (⚡ production modu)");
    }

    // Load settings cache from database (şifreler ve hassas veriler hariç)
    println!("⚙️  Ayarlar yükleniyor...");
    let settings_cache = crate::middleware::global_context::SettingsCache::load_from_db(&db)
        .await
        .unwrap_or_else(|e| {
            eprintln!("⚠️  Ayarlar yüklenemedi (varsayılan kullanılacak): {}", e); //varsayılan ayarlar patlak olabilir //TODO: bu durumda varsayılan ayarlar patlak olabilir
            crate::middleware::global_context::SettingsCache::default()
        });
    println!("🍀 Ayarlar cache'e yüklendi (hassas veriler hariç)");

    // Load menu cache from database (tüm diller için)
    println!("🍔 Menü verileri yükleniyor...");
    let languages: Vec<String> = config.supported_languages.keys().cloned().collect();
    let menu_cache = crate::middleware::global_context::MenuCache::load_from_db(&db, &languages)
        .await
        .unwrap_or_else(|e| {
            eprintln!(
                "⚠️  Menü verileri yüklenemedi (varsayılan kullanılacak): {}",
                e
            );
            crate::middleware::global_context::MenuCache::default()
        });
    println!("🍀 Menü cache'e yüklendi ({} dil)", languages.len());

    // Load global context cache from database (gcx=true contents)
    println!("🌍 Global context yükleniyor...");
    let global_context_cache =
        crate::modules::content::helpers::global_context_helper::load_global_context(&db)
            .await
            .unwrap_or_else(|e| {
                eprintln!(
                    "⚠️  Global context yüklenemedi (varsayılan kullanılacak): {}",
                    e
                );
                std::collections::BTreeMap::new()
            });
    println!(
        "🍀 Global context cache'e yüklendi ({} içerik)",
        global_context_cache.len()
    );

    // Create application state
    let app_state = AppState::new(
        db.clone(),
        tera,
        config.clone(),
        i18n,
        settings_cache,
        menu_cache,
        global_context_cache,
        redis_manager,
    );

    // Session store - config'den max age al
    // SSL ayarına göre çerez güvenliğini ayarla
    let is_ssl_enabled = config.is_ssl_enabled();

    let session_max_age = time::Duration::seconds(config.session_max_age() as i64);
    let session_store = SeaOrmSessionStore::new(db);
    let session_layer = SessionManagerLayer::new(session_store)
        .with_name("session_id")
        .with_http_only(true)
        // SSL ayarına göre secure flag'i ayarla
        .with_secure(is_ssl_enabled)
        // SSL durumuna göre SameSite politikası:
        // SSL aktifse (production): SameSite=None + Secure=true (payment provider redirects için)
        // SSL kapalıysa (development): SameSite=Lax + Secure=false (local development için)
        .with_same_site(if is_ssl_enabled {
            tower_sessions::cookie::SameSite::None // Production: payment provider redirects için
        } else {
            tower_sessions::cookie::SameSite::Lax // Development: local testing için
        })
        .with_expiry(tower_sessions::Expiry::OnInactivity(session_max_age));

    // Build router
    let app = Router::new()
        // Root redirect
        .route("/", get(root_redirect))
        // Explicitly handle /api and /favicon.ico to prevent them falling through to /{lang}
        .route("/api", get(handle_api_root))
        // Admin static files - Development: disk, Production: embedded
        .route(
            "/static/admin/{*path}",
            get(modules::admin::static_files::serve_admin_static),
        )
        // Static files - theme-aware serving from templates/{theme}/static/
        .route(
            "/static/{*path}",
            get(modules::static_files::serve_theme_static),
        )
        // Media files
        .nest_service("/media", ServeDir::new(config.media_dir()))
        .route("/media/thumb/{size}/{crop}/{*path}", get(thumbnail_handler))
        // Robots.txt
        .route("/robots.txt", get(robots_handler))
        // Configure routes
        .merge(modules::auth::routes::routes())
        .merge(modules::bookmarks::routes::routes())
        .merge(modules::comment::routes::routes())
        .merge(modules::admin::routes::routes())
        .merge(modules::ecommerce::routes::routes())
        .merge(modules::payment_provider::routes::payment_provider_routes())
        .merge(modules::media::routes::routes())
        .merge(modules::content::routes::routes())
        .merge(modules::form::routes::routes())
        .merge(modules::timeline::routes::routes())
        .merge(modules::search::routes::routes())
        .merge(modules::b2b::routes::routes())
        .merge(modules::iot::routes::routes())
        .merge(modules::ride::routes::routes())
        .merge(modules::location::routes::routes())
        // 404 fallback - must be last
        .fallback(not_found_fallback)
        // Add state and middleware
        .with_state(app_state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(session_layer)
                .layer(axum::middleware::from_fn_with_state(
                    app_state.clone(),
                    middleware::global_context::global_context_middleware,
                ))
                .layer(LoggerLayer::new())
                .layer(DefaultBodyLimit::max(300 * 1024 * 1024)), // 300MB limit
        );

    let server_addr = config.server_address();

    // Background tasks başlat (exchange rate updater, vb.)
    modules::background_tasks::start_all(std::sync::Arc::new(app_state.db.clone()));

    // Location flush task başlat
    {
        let state_arc = std::sync::Arc::new(app_state.clone());
        tokio::spawn(modules::background_tasks::location_flush::start_location_flush(state_arc));
    }

    println!("🚀 Sunucu başlatılıyor...");
    let app_version = version::AppVersion::new();
    println!(
        "📦 Versiyon: {}",
        app_version.version.color(Color::BrightYellow)
    );
    println!(
        "📱 Frontend: {}",
        format!("http://{}", server_addr).color(Color::BrightBlue)
    );
    println!(
        "🔧 Admin: {}",
        format!("http://{}/admin", server_addr).color(Color::BrightGreen)
    );
    // println!(
    //     "📡 API: {}",
    //     format!("http://{}/api", server_addr).color(Color::BrightYellow)
    // );

    let base_url = config.get_base_url();

    println!("🌐 Base URL: {}", base_url.color(Color::BrightCyan));
    println!(
        "Durdurmak için Ctrl+C'ye basın, tüm istekler sonlandırıldıktan sonra sunucu kapanacak"
    );

    // Start server
    let listener = tokio::net::TcpListener::bind(&server_addr).await?;
    // axum::serve(listener, app).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

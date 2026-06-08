#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        if $crate::config::get_config().is_debug() {
            tracing::debug!($($arg)*);
        }
    };
}

/// Bilgi logu - sadece debug modunda
#[macro_export]
macro_rules! info_log {
    ($($arg:tt)*) => {
        if $crate::config::get_config().is_debug() {
            tracing::info!($($arg)*);
        }
    };
}

/// Uyarı logu - her zaman loglanır
#[macro_export]
macro_rules! warn_log {
    ($($arg:tt)*) => {
        tracing::warn!($($arg)*);
    };
}

/// Hata logu - her zaman loglanır
#[macro_export]
macro_rules! error_log {
    ($($arg:tt)*) => {
        tracing::error!($($arg)*);
    };
}

/// Request extension'dan mevcut dili alır
/// Kullanım: let lang = current_language!(req);
/// Dönüş: String (mevcut dil kodu, örn: "tr", "en")
#[macro_export]
macro_rules! current_language {
    ($req:expr) => {
        $req.extensions()
            .get::<$crate::middleware::global_context::CurrentLanguage>()
            .map(|cl| cl.0.clone())
            .unwrap_or_else(|| $crate::config::get_config().default_language.clone())
    };
}

/// Mevcut dili özel varsayılan değerle alır
/// Kullanım: let lang = current_language_or!(req, "en");
/// Dönüş: String (mevcut dil veya verilen varsayılan)
#[macro_export]
macro_rules! current_language_or {
    ($req:expr, $default:expr) => {
        $req.extensions()
            .get::<$crate::middleware::global_context::CurrentLanguage>()
            .map(|cl| cl.0.clone())
            .unwrap_or_else(|| $default.to_string())
    };
}

/// Mevcut dilin belirtilen dile eşit olup olmadığını kontrol eder
/// Kullanım: if is_language!(req, "tr") { ... }
/// Dönüş: bool
#[macro_export]
macro_rules! is_language {
    ($req:expr, $lang:expr) => {
        $req.extensions()
            .get::<$crate::middleware::global_context::CurrentLanguage>()
            .map(|cl| cl.0 == $lang)
            .unwrap_or(false)
    };
}

/// Config'den desteklenen dilleri alır
/// Kullanım: let langs = supported_languages!();
/// Dönüş: Vec<String> (desteklenen dil kodları listesi)
#[macro_export]
macro_rules! supported_languages {
    () => {
        $crate::config::get_config().ordered_languages()
    };
}

/// Config'den desteklenen dilleri JSON formatında (HashMap) alır
/// Kullanım: let langs = supported_languages_json!();
/// Dönüş: HashMap<String, String> (dil kodu -> dil adı)
/// Örnek çıktı: {"en": "English", "tr": "Türkçe"}
#[macro_export]
macro_rules! supported_languages_map {
    () => {
        $crate::config::get_config().supported_languages.clone()
    };
}

/// Bir dilin desteklenip desteklenmediğini kontrol eder
/// Kullanım: if is_supported_language!("en") { ... }
/// Dönüş: bool
#[macro_export]
macro_rules! is_supported_language {
    ($lang:expr) => {
        $crate::config::get_config().is_language_supported($lang)
    };
}

/// Config'den varsayılan dili alır
/// Kullanım: let default = default_language!();
/// Dönüş: String (varsayılan dil kodu)
#[macro_export]
macro_rules! default_language {
    () => {
        $crate::config::get_config().default_language.clone()
    };
}

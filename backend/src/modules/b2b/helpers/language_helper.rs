// Language Helper - Utilities for language-based routing

use crate::config::app_config::AppConfig;

/// Reserved paths that should NOT be handled by language routes
/// These paths should be handled by their specific route handlers
pub const RESERVED_PATHS: &[&str] = &[
    "api",
    "admin",
    "my-account",
    "static",
    "media",
    "login",
    "logout",
    "register",
    "profile",
    "favicon.ico",
    "robots.txt",
    "sitemap.xml",
    "health",
    "_next",
    "assets",
    "images",
    "css",
    "js",
    "fonts",
    "sw.js",
    "manifest.json",
    "service-worker.js",
];

/// Check if a path segment is a reserved path that should not be treated as a language code
pub fn is_reserved_path(path: &str) -> bool {
    RESERVED_PATHS.contains(&path.to_lowercase().as_str())
}

/// Result of validating a language parameter
pub enum LanguageValidation {
    /// Valid and supported language code
    Valid(String),
    /// Not a valid language code, should return 404 for other routes to handle
    ReservedPath,
    /// Valid format but unsupported language, should redirect to default
    Unsupported { redirect_to: String },
}

/// Validate a language parameter and determine the appropriate action
pub fn validate_language(language: &str, config: &AppConfig) -> LanguageValidation {
    // Check if this is a reserved path
    if is_reserved_path(language) {
        return LanguageValidation::ReservedPath;
    }

    // Check if language is supported
    if config.is_language_supported(language) {
        return LanguageValidation::Valid(language.to_string());
    }

    // Language looks like a valid format but is not supported
    // Redirect to default language
    LanguageValidation::Unsupported {
        redirect_to: format!("/{}", config.default_language),
    }
}

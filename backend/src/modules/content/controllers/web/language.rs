use axum::{
    extract::{Path, State},
    http::{StatusCode, HeaderMap, HeaderValue},
    response::{Response, Redirect, IntoResponse},
};
use crate::app_state::AppState;
use crate::config;

// Dil değiştirme endpoint'i - cookie set eder ve redirect yapar
pub async fn set_language(
    State(_state): State<AppState>,
    Path(lang_code): Path<String>,
) -> Response {
    let config = config::get_config();
    
    // Dil destekleniyorsa cookie set et
    if config.is_language_supported(&lang_code) {
        // Cookie header oluştur - 1 yıl geçerli
        let cookie_value = format!(
            "language={}; Path=/; Max-Age=31536000; SameSite=Lax", 
            lang_code
        );
        
        let mut headers = HeaderMap::new();
        if let Ok(header_value) = HeaderValue::from_str(&cookie_value) {
            headers.insert("set-cookie", header_value);
        }
        
        // Ana sayfaya yönlendir
        let redirect_url = format!("/{}", lang_code);
        
        let mut response = Redirect::to(&redirect_url).into_response();
        response.headers_mut().extend(headers);
        
        response
    } else {
        // Desteklenmeyen dil - 404
        (StatusCode::NOT_FOUND, "Language not supported").into_response()
    }
}

// Dil değiştirme endpoint'i - belirli bir sayfaya redirect ile
pub async fn set_language_with_redirect(
    State(_state): State<AppState>,
    Path((lang_code, redirect_path)): Path<(String, String)>,
) -> Response {
    let config = config::get_config();
    
    // Dil destekleniyorsa cookie set et
    if config.is_language_supported(&lang_code) {
        // Cookie header oluştur - 1 yıl geçerli
        let cookie_value = format!(
            "language={}; Path=/; Max-Age=31536000; SameSite=Lax", 
            lang_code
        );
        
        let mut headers = HeaderMap::new();
        if let Ok(header_value) = HeaderValue::from_str(&cookie_value) {
            headers.insert("set-cookie", header_value);
        }
        
        // Belirtilen sayfaya yönlendir
        let redirect_url = format!("/{}/{}", lang_code, redirect_path);
        
        let mut response = Redirect::to(&redirect_url).into_response();
        response.headers_mut().extend(headers);
        
        response
    } else {
        // Desteklenmeyen dil - 404
        (StatusCode::NOT_FOUND, "Language not supported").into_response()
    }
}
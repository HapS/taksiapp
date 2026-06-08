use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, error, info, warn};

use crate::app_state::AppState;

/// Get content type based on file extension
fn get_content_type(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("otf") => "font/otf",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Serve static files from the active theme's static directory
/// URL: /static/css/base.css -> templates/{theme}/static/css/base.css
pub async fn serve_theme_static(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response<Body> {
    let theme = state.get_frontend_theme();
    let path_for_log = path.clone();
    
    info!(
        theme = %theme,
        path = %path_for_log,
        "Statik dosya isteği alındı"
    );
    
    // Security check: prevent directory traversal
    if path.contains("..") {
        warn!(path = %path, "Geçersiz statik dosya yolu (directory traversal)");
        return not_found_response();
    }
    
    // Ensure path starts with /
    let relative_path = if path.starts_with('/') {
        path
    } else {
        format!("/{}", path)
    };
    
    // Build file path: templates/{theme}/static/{relative_path}
    let file_path = PathBuf::from(format!("templates/{}/static{}", theme, relative_path));
    
    debug!(
        theme = %theme,
        path = %path_for_log,
        file_path = %file_path.display(),
        "Tema statik dosyası sunuluyor"
    );

    // Check if file exists
    match fs::metadata(&file_path).await {
        Ok(metadata) => {
            if !metadata.is_file() {
                warn!(path = %file_path.display(), "Statik dosya bir dizin");
                return not_found_response();
            }

            // Read file
            match fs::read(&file_path).await {
                Ok(contents) => {
                    let content_type = get_content_type(&file_path);
                    
                    info!(
                        theme = %theme,
                        path = %path_for_log,
                        file_path = %file_path.display(),
                        content_type = %content_type,
                        size = contents.len(),
                        "Statik dosya başarıyla sunuldu"
                    );
                    
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, content_type)
                        .header(header::CACHE_CONTROL, "public, max-age=86400") // 24 hour cache
                        .body(Body::from(contents))
                        .unwrap_or_else(|_| not_found_response())
                }
                Err(e) => {
                    error!(
                        path = %file_path.display(),
                        error = %e,
                        "Statik dosya okunamadı"
                    );
                    not_found_response()
                }
            }
        }
        Err(e) => {
            // If file doesn't exist in theme, fallback to base theme
            if theme != "base" {
                let fallback_path = PathBuf::from(format!("templates/base/static{}", relative_path));
                debug!(
                    theme = %theme,
                    fallback = %fallback_path.display(),
                    "Tema dosyası bulunamadı, base tema fallback"
                );
                
                if let Ok(metadata) = fs::metadata(&fallback_path).await {
                    if metadata.is_file() {
                        if let Ok(contents) = fs::read(&fallback_path).await {
                            let content_type = get_content_type(&fallback_path);
                            return Response::builder()
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, content_type)
                                .header(header::CACHE_CONTROL, "public, max-age=86400")
                                .body(Body::from(contents))
                                .unwrap_or_else(|_| not_found_response());
                        }
                    }
                }
            }
            
            error!(
                theme = %theme,
                requested_path = %path_for_log,
                tried_path = %file_path.display(),
                fallback_tried = if theme != "base" { "evet" } else { "hayır" },
                error = %e,
                "Statik dosya BULUNAMADI"
            );
            not_found_response()
        }
    }
}

fn not_found_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}

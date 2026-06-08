use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
};

// Development mode imports
#[cfg(debug_assertions)]
use std::path::PathBuf;
#[cfg(debug_assertions)]
use tokio::fs;
#[cfg(debug_assertions)]
use tracing::{debug, error};

// Production mode imports  
#[cfg(not(debug_assertions))]
use crate::app_state::AdminStatic;

use tracing::{info, warn};

use crate::app_state::AppState;

/// Get content type based on file extension
fn get_content_type(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else if path.ends_with(".eot") {
        "application/vnd.ms-fontobject"
    } else if path.ends_with(".otf") {
        "font/otf"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}

/// Serve admin static files
/// Development: templates/admin/static/ directory
/// Production: Embedded files
pub async fn serve_admin_static(
    _state: State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response<Body> {
    // Security check: prevent directory traversal
    if path.contains("..") {
        warn!(path = %path, "Geçersiz admin statik dosya yolu");
        return not_found_response();
    }

    info!(path = %path, "Admin statik dosya isteği");

    // Development mode: Read from disk
    #[cfg(debug_assertions)]
    {
        let file_path = PathBuf::from(format!("templates/admin/static/{}", path));
        
        debug!(
            path = %path,
            file_path = %file_path.display(),
            "Development: Diskten okunuyor"
        );

        match fs::metadata(&file_path).await {
            Ok(metadata) => {
                if !metadata.is_file() {
                    warn!(path = %file_path.display(), "Admin statik dosya bir dizin");
                    return not_found_response();
                }

                match fs::read(&file_path).await {
                    Ok(contents) => {
                        let content_type = get_content_type(&path);
                        
                        info!(
                            path = %path,
                            size = contents.len(),
                            "Admin statik dosya sunuldu (disk)"
                        );
                        
                        return Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, content_type)
                            .header(header::CACHE_CONTROL, "public, max-age=3600")
                            .body(Body::from(contents))
                            .unwrap_or_else(|_| not_found_response());
                    }
                    Err(e) => {
                        error!(path = %file_path.display(), error = %e, "Admin dosya okunamadı");
                        return not_found_response();
                    }
                }
            }
            Err(e) => {
                warn!(path = %file_path.display(), error = %e, "Admin dosya bulunamadı");
                return not_found_response();
            }
        }
    }

    // Production mode: Serve from embedded files
    #[cfg(not(debug_assertions))]
    {
        match AdminStatic::get(&path) {
            Some(file) => {
                let content_type = get_content_type(&path);
                
                info!(
                    path = %path,
                    size = file.data.len(),
                    "Admin statik dosya sunuldu (embedded)"
                );
                
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, content_type)
                    .header(header::CACHE_CONTROL, "public, max-age=86400")
                    .body(Body::from(file.data.to_vec()))
                    .unwrap_or_else(|_| not_found_response())
            }
            None => {
                warn!(path = %path, "Embedded admin dosya bulunamadı");
                not_found_response()
            }
        }
    }
}

fn not_found_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap()
}

// Media Web Controller - HTML views
use crate::app_state::AppState;
use axum::{
    body::Body,
    extract::Path,
    extract::State,
    http::{Request, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use image::imageops::FilterType;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use tera::Context;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use tower_sessions::Session;

// Helper: Admin check
// Use common RBAC helper
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;

/// Media explorer page
pub async fn media_explorer(State(state): State<AppState>, session: Session) -> Response {
    // Admin check
    if !is_admin(&state, &session).await {
        return Redirect::to("/login").into_response();
    }

    let mut context = Context::new();
    context.insert("title", "Medya Yönetimi");
    context.insert("current_path", "/admin/media");

    // Add user data
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        context.insert("user", &user_data);
    }

    match state.render_template("admin/media/media_explorer.html", &context) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode, otherwise return generic 500
            return crate::middleware::error_handler::handle_template_error(
                &e,
                state.config.is_debug(),
            );
        }
    }
}

pub async fn thumbnail_handler(
    State(state): State<AppState>,
    Path((size, crop, path)): Path<(String, String, String)>,
    req: Request<Body>,
) -> impl IntoResponse {
    use std::time::Duration;

    // Basic size parsing and bounds check
    let (w, h) = match parse_size(&size) {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    if w > 2000 || h > 2000 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // 1) Path sanitization - reject absolute paths or parent dir traversal
    let path_obj = std::path::Path::new(&path);
    if path_obj.is_absolute()
        || path_obj
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // 2) crop validation - allow list
    let allowed_single = ["fit", "center", "fill", "top", "bottom", "left", "right"];
    let allowed = || {
        if allowed_single.contains(&crop.as_str()) {
            true
        } else if crop.contains('-') {
            // allow combinations like "top-left" etc. Validate parts
            crop.split('-').all(|part| allowed_single.contains(&part))
        } else {
            false
        }
    };

    if !allowed() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Orijinal dosya yolu: media/uploads/path
    let original = PathBuf::from(state.config.media_upload_root()).join(&path);
    if !original.exists() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let cached_filename = path.clone();

    // Cache yolu: media/cache/WxH/crop/path
    let cached = PathBuf::from(state.config.media_dir())
        .join("cache")
        .join(format!("{}x{}", w, h))
        .join(&crop)
        .join(&cached_filename);

    if !cached.exists() {
        // Request Coalescing
        let lock = state
            .media_locks
            .entry(cached.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .value()
            .clone();

        let _guard = lock.lock().await;

        if !cached.exists() {
            if let Some(parent) = cached.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let original_clone = original.clone();
            let cached_clone = cached.clone();
            let crop_clone = crop.clone();

            // 5) Acquire a permit from semaphore with short timeout
            let sem = state.thumbnail_semaphore.clone();
            let permit =
                match tokio::time::timeout(Duration::from_secs(40), sem.acquire_owned()).await {
                    Ok(perm) => match perm {
                        Ok(p) => p,
                        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
                    },
                    Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
                };

            // 6) Spawn blocking inside a timeout so long-running conversions are killed
            let spawn_handle = tokio::task::spawn_blocking(move || {
                create_thumbnail(&original_clone, &cached_clone, w, h, &crop_clone)
            });

            match tokio::time::timeout(Duration::from_secs(40), spawn_handle).await {
                Ok(join_res) => {
                    match join_res {
                        Ok(Ok(_)) => {
                            // success
                        }
                        Ok(Err(e)) => {
                            eprintln!("Thumbnail creation error: {e}");
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                        Err(e) => {
                            eprintln!("Thumbnail thread error: {e}");
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    }
                }
                Err(_) => {
                    eprintln!("Thumbnail creation timed out");
                    return StatusCode::GATEWAY_TIMEOUT.into_response();
                }
            }

            // permit dropped here, releasing the semaphore
            drop(permit);
        }
    }

    // ServeFile using the request to handle headers correctly
    match ServeFile::new(cached).oneshot(req).await {
        Ok(res) => res.into_response(),
        Err(e) => {
            eprintln!("ServeFile error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn create_thumbnail(
    input: &StdPath,
    output: &StdPath,
    w: u32,
    h: u32,
    crop: &str,
) -> anyhow::Result<()> {
    use image::GenericImageView;

    // 1. Read EXIF Orientation
    let orientation = if let Ok(file) = std::fs::File::open(input) {
        let mut bufreader = std::io::BufReader::new(file);
        let exifreader = exif::Reader::new();
        exifreader
            .read_from_container(&mut bufreader)
            .ok()
            .and_then(|exif| {
                exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                    .and_then(|field| field.value.get_uint(0))
            })
    } else {
        None
    };

    let mut img = image::open(input)?;

    // 2. Apply Orientation
    if let Some(orient) = orientation {
        match orient {
            2 => img = img.fliph(),
            3 => img = img.rotate180(),
            4 => img = img.flipv(),
            5 => {
                img = img.fliph();
                img = img.rotate270();
            }
            6 => img = img.rotate90(),
            7 => {
                img = img.fliph();
                img = img.rotate90();
            }
            8 => img = img.rotate270(),
            _ => {} // 1 or others: normal
        }
    }

    // 3. Calculate target dimensions for auto-scale (0 values)
    let (orig_w, orig_h) = img.dimensions();
    let (target_w, target_h) = match (w, h) {
        (0, 0) => (orig_w, orig_h), // Should not happen due to parse_size + handler check
        (w, 0) => {
            let h = (orig_h as f32 * (w as f32 / orig_w as f32)).round() as u32;
            (w, h)
        }
        (0, h) => {
            let w = (orig_w as f32 * (h as f32 / orig_h as f32)).round() as u32;
            (w, h)
        }
        (w, h) => (w, h),
    };

    match crop {
        "fit" => {
            img = img.resize(target_w, target_h, FilterType::Lanczos3);
        }
        "fill" | "center" => {
            img = img.resize_to_fill(target_w, target_h, FilterType::Lanczos3);
        }
        _ if crop.contains('-') || ["top", "bottom", "left", "right"].contains(&crop) => {
            let (cur_w, cur_h) = img.dimensions();
            let ratio_w = target_w as f32 / cur_w as f32;
            let ratio_h = target_h as f32 / cur_h as f32;
            let ratio = ratio_w.max(ratio_h);

            let new_w = (cur_w as f32 * ratio).round() as u32;
            let new_h = (cur_h as f32 * ratio).round() as u32;

            img = img.resize(new_w, new_h, FilterType::Lanczos3);

            let x = match crop {
                c if c.contains("left") || c == "left" => 0,
                c if c.contains("right") || c == "right" => new_w - target_w,
                _ => (new_w - target_w) / 2,
            };

            let y = match crop {
                c if c.contains("top") || c == "top" => 0,
                c if c.contains("bottom") || c == "bottom" => new_h - target_h,
                _ => (new_h - target_h) / 2,
            };

            img = img.crop_imm(x, y, target_w, target_h);
        }
        _ => {
            img = img.resize_to_fill(target_w, target_h, FilterType::Lanczos3);
        }
    }

    // Save as original format (inferred from extension)
    img.save(output)?;

    Ok(())
}

fn parse_size(size: &str) -> Result<(u32, u32), ()> {
    let parts: Vec<&str> = size.split('x').collect();
    if parts.len() != 2 {
        return Err(());
    }

    let w = if parts[0].is_empty() {
        0
    } else {
        parts[0].parse().map_err(|_| ())?
    };

    let h = if parts[1].is_empty() {
        0
    } else {
        parts[1].parse().map_err(|_| ())?
    };

    // At least one dimension must be specified
    if w == 0 && h == 0 {
        return Err(());
    }

    Ok((w, h))
}

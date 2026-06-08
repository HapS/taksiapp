use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use axum_extra::extract::Multipart;
use sea_orm::*;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::middleware::auth::MaybeAuthenticatedUser;
use crate::modules::form::models::FormSubmissionActiveModel;
use crate::modules::utils::ip_helper::get_client_ip_from_headers;

#[derive(Serialize)]
pub struct SubmitFormResponse {
    pub success: bool,
    pub message: String,
    pub submission_id: Option<i64>,
}

/// POST /api/form - Submit a form (Multipart)
pub async fn submit_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    MaybeAuthenticatedUser { id: user_id }: MaybeAuthenticatedUser,
    mut multipart: Multipart,
) -> Response {
    // Get client IP from headers
    let client_ip = get_client_ip_from_headers(&headers);

    let mut form_id: Option<i64> = None;
    let mut form_data = serde_json::Map::new();

    // Process multipart fields
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("unknown").to_string();

        if name == "form_id" {
            if let Ok(val) = field.text().await {
                if let Ok(parsed_id) = val.parse::<i64>() {
                    form_id = Some(parsed_id);
                }
            }
        } else if let Some(filename) = field.file_name() {
            // It's a file
            let filename = filename.to_string();
            // Get content type before consuming the field
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            // Read file data
            if let Ok(bytes) = field.bytes().await {
                // Determine upload directory: media/uploads/formfiles/{form_id}/
                let fid = form_id.unwrap_or(0); // If form_id comes after file, we might have an issue.
                                                // Ideally frontend sends form_id first.

                // Backup for if form_id is not yet parsed (use temp or date based)
                let upload_dir = format!(
                    "media/uploads/formfiles/{}",
                    if fid > 0 {
                        fid.to_string()
                    } else {
                        "temp".to_string()
                    }
                );

                // Create directory if not exists
                if let Err(e) = fs::create_dir_all(&upload_dir).await {
                    eprintln!("Failed to create upload dir: {}", e);
                    continue;
                }

                // Generate unique filename
                let unique_name = format!("{}_{}", Uuid::new_v4(), filename);
                let file_path = Path::new(&upload_dir).join(&unique_name);

                // Save file
                if let Err(e) = fs::write(&file_path, bytes).await {
                    eprintln!("Failed to write file: {}", e);
                    continue;
                }

                // Create public URL
                let public_url = format!(
                    "/media/uploads/formfiles/{}/{}",
                    if fid > 0 {
                        fid.to_string()
                    } else {
                        "temp".to_string()
                    },
                    unique_name
                );

                // Add to form data as object
                let file_info = json!({
                    "label": name, // For files, label might need to be passed differently or we use name
                    "value": public_url,
                    "type": "file",
                    "filename": filename,
                    "mime_type": content_type
                });

                form_data.insert(name, file_info);
            }
        } else {
            // It's a text field
            if let Ok(val) = field.text().await {
                // Try to parse if it's a JSON string (for structured data like {label:..., value:...})
                if let Ok(json_val) = serde_json::from_str::<Value>(&val) {
                    form_data.insert(name, json_val);
                } else {
                    // Metadata for simple text field, convert to object structure
                    let field_obj = json!({
                        "label": name, // Default label to name if not provided
                        "value": val
                    });
                    form_data.insert(name, field_obj);
                }
            }
        }
    }

    // Create submission if form_id exists
    if let Some(fid) = form_id {
        // Verify that the form exists
        let form_exists = crate::modules::content::models::Content::find_by_id(fid)
            .filter(crate::modules::content::models::content::Column::ContentType.eq("form"))
            .filter(crate::modules::content::models::content::Column::DeletedAt.is_null())
            .one(&state.db)
            .await;

        match form_exists {
            Ok(Some(_)) => {
                // Form exists, create submission
                let submission = FormSubmissionActiveModel {
                    form_id: Set(fid),
                    data: Set(Value::Object(form_data)),
                    ip: Set(client_ip),
                    user_id: Set(user_id),
                    created_at: Set(Some(chrono::Utc::now().into())),
                    updated_at: Set(Some(chrono::Utc::now().into())),
                    ..Default::default()
                };

                match submission.insert(&state.db).await {
                    Ok(result) => (
                        StatusCode::CREATED,
                        Json(SubmitFormResponse {
                            success: true,
                            message: "Form başarıyla gönderildi.".to_string(),
                            submission_id: Some(result.id),
                        }),
                    )
                        .into_response(),
                    Err(e) => {
                        eprintln!("Error saving form submission: {:?}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({
                                "success": false,
                                "message": "Form kaydedilirken bir hata oluştu."
                            })),
                        )
                            .into_response()
                    }
                }
            }
            Ok(None) => (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "message": "Form bulunamadı."
                })),
            )
                .into_response(),
            Err(e) => {
                eprintln!("Error checking form existence: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "message": "Form kontrol edilirken bir hata oluştu."
                    })),
                )
                    .into_response()
            }
        }
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": "Form ID eksik."
            })),
        )
            .into_response()
    }
}

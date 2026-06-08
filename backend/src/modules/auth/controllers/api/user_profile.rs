use crate::app_state::AppState;
use crate::middleware::auth::AuthenticatedUser;
use crate::modules::auth::services::auth_service;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use rust_i18n::t;
use serde::{Deserialize, Serialize};
// profil güncelleme dto
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub bio: Option<String>,
    pub phone: Option<String>,
    pub phone_country_code: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
    pub birth_date: Option<String>, // yyyy-mm-dd formatında
}

// profil response dto
#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub id: i64,
    pub username: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: String,
    pub birth_date: Option<String>,
    pub phone_number: Option<String>,
    pub phone_country_code: Option<String>,
    pub profile: Option<ProfileData>,
    pub user_type: Option<String>,
    pub created_at: Option<String>,
    pub has_password: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileData {
    pub bio: Option<String>,
    pub website: Option<String>,
    pub location: Option<String>,
}

// api response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T, message: &str) -> Self {
        Self {
            success: true,
            message: message.to_string(),
            data: Some(data),
        }
    }

    pub fn error(message: &str) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            message: message.to_string(),
            data: None,
        }
    }
}

/// kullanıcı profilini getir (hem web hem mobil için)
pub async fn get_profile(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> impl IntoResponse {
    let user = match auth_service::get_user_by_id(&state.db, auth_user.id).await {
        Ok(user) => user,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(&t!(
                    "user_not_found",
                    locale = &state.config.default_language
                ))),
            )
                .into_response()
        }
    };

    // profil verisini parse et
    let profile_data = if let Some(profile_json) = &user.profile {
        serde_json::from_value::<ProfileData>(profile_json.clone()).ok()
    } else {
        None
    };

    // doğum tarihini string'e çevir
    let birth_date = user.birth_date.map(|dt| dt.format("%Y-%m-%d").to_string());

    let response = ProfileResponse {
        id: user.id,
        username: user.username.clone(),
        first_name: user.first_name.clone(),
        last_name: user.last_name.clone(),
        email: user.email.clone(),
        birth_date,
        phone_number: user.phone_number.clone(),
        phone_country_code: user.phone_country_code.clone(),
        profile: profile_data,
        user_type: user.user_type.clone(),
        created_at: user
            .created_at
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        has_password: user.password.is_some(),
    };

    (
        StatusCode::OK,
        Json(ApiResponse::success(
            response,
            &t!("profile_success", locale = &state.config.default_language),
        )),
    )
        .into_response()
}

/// kullanıcı profilini güncelle (hem web hem mobil için)
pub async fn update_profile(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<UpdateProfileRequest>,
) -> impl IntoResponse {
    // kullanıcıyı veritabanından al
    let user = match auth_service::get_user_by_id(&state.db, auth_user.id).await {
        Ok(user) => user,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(&t!(
                    "user_not_found",
                    locale = &state.config.default_language
                ))),
            )
                .into_response()
        }
    };

    // email benzersizlik kontrolü (eğer değiştirilmişse)
    if user.email != request.email {
        if let Ok(existing_user) = auth_service::get_user_by_email(&state.db, &request.email).await
        {
            if existing_user.id != user.id {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()>::error(&t!(
                        "email_in_use",
                        locale = &state.config.default_language
                    ))),
                )
                    .into_response();
            }
        }
    }

    // profil json'ını hazırla (telefon artık users tablosunda)
    let mut profile_data = serde_json::Map::new();

    if let Some(bio) = request.bio {
        if !bio.trim().is_empty() {
            profile_data.insert("bio".to_string(), serde_json::Value::String(bio));
        }
    }

    if let Some(website) = request.website {
        if !website.trim().is_empty() {
            profile_data.insert("website".to_string(), serde_json::Value::String(website));
        }
    }

    if let Some(location) = request.location {
        if !location.trim().is_empty() {
            profile_data.insert("location".to_string(), serde_json::Value::String(location));
        }
    }

    // doğum tarihi parse et
    let birth_date = if let Some(bd) = request.birth_date {
        if !bd.trim().is_empty() {
            match chrono::NaiveDate::parse_from_str(&bd, "%Y-%m-%d") {
                Ok(date) => {
                    let datetime = date.and_hms_opt(0, 0, 0).unwrap();
                    Some(
                        datetime
                            .and_utc()
                            .with_timezone(&chrono::FixedOffset::east_opt(0).unwrap()),
                    )
                }
                Err(_) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ApiResponse::<()>::error(&t!(
                            "invalid_birth_date_format",
                            locale = &state.config.default_language
                        ))),
                    )
                        .into_response();
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // kullanıcıyı güncelle
    use crate::modules::auth::models::user::ActiveModel;
    use sea_orm::{ActiveModelTrait, Set};

    let mut user_update: ActiveModel = user.into();
    user_update.first_name = Set(Some(request.first_name));
    user_update.last_name = Set(Some(request.last_name));
    user_update.email = Set(request.email);
    user_update.birth_date = Set(birth_date);
    user_update.phone_number = Set(request.phone.filter(|p| !p.trim().is_empty()));
    user_update.phone_country_code =
        Set(request.phone_country_code.filter(|p| !p.trim().is_empty()));
    user_update.profile = Set(Some(serde_json::Value::Object(profile_data)));
    user_update.updated_at = Set(Some(
        chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(0).unwrap()),
    ));

    match user_update.update(&state.db).await {
        Ok(updated_user) => {
            // güncellenmiş profil verisini hazırla
            let profile_data = if let Some(profile_json) = &updated_user.profile {
                serde_json::from_value::<ProfileData>(profile_json.clone()).ok()
            } else {
                None
            };

            let birth_date = updated_user
                .birth_date
                .map(|dt| dt.format("%Y-%m-%d").to_string());

            let response = ProfileResponse {
                id: updated_user.id,
                username: updated_user.username,
                first_name: updated_user.first_name,
                last_name: updated_user.last_name,
                email: updated_user.email,
                birth_date,
                phone_number: updated_user.phone_number,
                phone_country_code: updated_user.phone_country_code,
                profile: profile_data,
                user_type: updated_user.user_type.clone(),
                created_at: updated_user
                    .created_at
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                has_password: updated_user.password.is_some(),
            };

            (
                StatusCode::OK,
                Json(ApiResponse::success(
                    response,
                    &t!("profile_updated", locale = &state.config.default_language),
                )),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("profil güncelleme hatası: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(&t!(
                    "profile_updated_failed",
                    locale = &state.config.default_language
                ))),
            )
                .into_response()
        }
    }
}

/// şifre değiştirme
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: Option<String>,
    pub new_password: String,
    pub confirm_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(request): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    // şifre doğrulama
    if request.new_password != request.confirm_password {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(&t!(
                "passwords_do_not_match",
                locale = &state.config.default_language
            ))),
        )
            .into_response();
    }

    if request.new_password.len() < 6 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(&t!(
                "password_too_short",
                locale = &state.config.default_language
            ))),
        )
            .into_response();
    }

    // kullanıcıyı al
    let user = match auth_service::get_user_by_id(&state.db, auth_user.id).await {
        Ok(user) => user,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<()>::error(&t!(
                    "user_not_found",
                    locale = &state.config.default_language
                ))),
            )
                .into_response()
        }
    };

    // mevcut şifreyi kontrol et (sadece kullanıcının zaten şifresi varsa)
    //sosyal medya ile kayıt olmuş kullanıcıların şifresi olmayabilir kesin olmaz
    if let Some(existing_password) = user.password.as_ref() {
        // Kullanıcının mevcut şifresi var, current_password zorunlu
        let current_password = match request.current_password.as_ref() {
            Some(p) if !p.is_empty() => p,
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()>::error(&t!(
                        "current_password_required",
                        locale = &state.config.default_language
                    ))),
                )
                    .into_response();
            }
        };

        if !bcrypt::verify(current_password, existing_password).unwrap_or(false) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(&t!(
                    "current_password_incorrect",
                    locale = &state.config.default_language
                ))),
            )
                .into_response();
        }
    }
    // Kullanıcının şifresi yoksa (OAuth ile kayıt olmuş), current_password kontrolü yapma

    // yeni şifreyi hash'le
    let hashed_password = match bcrypt::hash(&request.new_password, bcrypt::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(&t!(
                    "password_hashing_failed",
                    locale = &state.config.default_language
                ))),
            )
                .into_response();
        }
    };

    // şifreyi güncelle
    use crate::modules::auth::models::user::ActiveModel;
    use sea_orm::{ActiveModelTrait, Set};

    let mut user_update: ActiveModel = user.into();
    user_update.password = Set(Some(hashed_password));
    user_update.updated_at = Set(Some(
        chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(0).unwrap()),
    ));

    match user_update.update(&state.db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::success(
                (),
                &t!(
                    "password_changed_successfully",
                    locale = &state.config.default_language
                ),
            )),
        )
            .into_response(),
        Err(e) => {
            eprintln!("şifre güncellenirken hata oluştu: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(&t!(
                    "password_update_failed",
                    locale = &state.config.default_language
                ))),
            )
                .into_response()
        }
    }
}

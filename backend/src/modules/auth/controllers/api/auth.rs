// Mobile Auth API Controllers - JWT-based REST endpoints for mobile apps
use crate::app_state::AppState;
use crate::config::get_config;
use crate::modules::auth::helpers::jwt::{
    generate_token_pair, validate_refresh_token, JwtConfig, TokenPair,
};
use crate::modules::auth::services::auth_service;
use crate::modules::utils::ip_helper::get_client_ip_from_headers;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};

// Request/Response Models
#[derive(Debug, Deserialize)]
pub struct MobileLoginRequest {
    pub username: String,
    pub password: String,
    pub guest_user_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MobileRegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub guest_user_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct MobileUserResponse {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub profile: Option<serde_json::Value>,
    pub user_type: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct MobileAuthResponse {
    pub success: bool,
    pub message: String,
    pub user: Option<MobileUserResponse>,
    pub tokens: Option<TokenPair>,
}

/// Helper: Get JWT config from app config
fn get_jwt_config() -> JwtConfig {
    let config = get_config();
    JwtConfig {
        secret: config.jwt_secret().to_string(),
        access_token_expiry: config.jwt_access_token_expiry(),
        refresh_token_expiry: config.jwt_refresh_token_expiry(),
    }
}

// ============ MOBILE API ENDPOINTS ============

/// Mobile API: Login with JWT
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(json): Json<MobileLoginRequest>,
) -> Response {
    match auth_service::login(
        &state.db,
        &json.username,
        &json.password,
        json.guest_user_id,
    )
    .await
    {
        Ok(session_data) => {
            // IP adresini güncelle
            let client_ip = get_client_ip_from_headers(&headers);
            if let Err(e) =
                auth_service::update_user_ip(&state.db, session_data.user_id, client_ip).await
            {
                eprintln!("IP update error: {}", e);
            }

            // Get user details
            match auth_service::get_user_by_id(&state.db, session_data.user_id).await {
                Ok(user) => {
                    // Generate JWT tokens
                    let jwt_config = get_jwt_config();
                    match generate_token_pair(user.id, &user.username, &user.email, &jwt_config) {
                        Ok(tokens) => {
                            let user_response = MobileUserResponse {
                                id: user.id,
                                username: user.username,
                                email: user.email,
                                first_name: user.first_name,
                                last_name: user.last_name,
                                profile: user.profile,
                                user_type: user.user_type.clone(),
                                created_at: user.created_at.map(|dt| dt.naive_utc().and_utc()),
                            };

                            let response = MobileAuthResponse {
                                success: true,
                                message: "Login successful".to_string(),
                                user: Some(user_response),
                                tokens: Some(tokens),
                            };
                            (StatusCode::OK, Json(response)).into_response()
                        }
                        Err(e) => {
                            let response = MobileAuthResponse {
                                success: false,
                                message: format!("Token generation failed: {}", e),
                                user: None,
                                tokens: None,
                            };
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
                        }
                    }
                }
                Err(e) => {
                    let response = MobileAuthResponse {
                        success: false,
                        message: e.to_string(),
                        user: None,
                        tokens: None,
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
                }
            }
        }
        Err(e) => {
            let response = MobileAuthResponse {
                success: false,
                message: e.to_string(),
                user: None,
                tokens: None,
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// Mobile API: Register with JWT
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(json): Json<MobileRegisterRequest>,
) -> Response {
    let first_name = json.first_name.clone().unwrap_or_default();
    let last_name = json.last_name.clone().unwrap_or_default();

    // Misafir kullanıcıyı gerçek kullanıcıya dönüştür veya yeni kullanıcı oluştur
    let user_result = if let Some(guest_id) = json.guest_user_id {
        // Guest user'ı gerçek kullanıcıya dönüştür
        auth_service::register_guest_user(
            &state.db,
            guest_id,
            &json.username,
            &json.email,
            &json.password,
            Some(first_name),
            Some(last_name),
        )
        .await
    } else {
        // Yeni kullanıcı oluştur
        auth_service::register(
            &state.db,
            &json.username,
            &json.email,
            &json.password,
            Some(first_name),
            Some(last_name),
        )
        .await
    };

    match user_result {
        Ok(user) => {
            // IP adresini güncelle
            let client_ip = get_client_ip_from_headers(&headers);
            if let Err(e) = auth_service::update_user_ip(&state.db, user.id, client_ip).await {
                eprintln!("IP update error: {}", e);
            }

            // Generate JWT tokens for newly registered user
            let jwt_config = get_jwt_config();
            match generate_token_pair(user.id, &user.username, &user.email, &jwt_config) {
                Ok(tokens) => {
                    let user_response = MobileUserResponse {
                        id: user.id,
                        username: user.username,
                        email: user.email,
                        first_name: user.first_name,
                        last_name: user.last_name,
                        profile: user.profile,
                        user_type: user.user_type.clone(),
                        created_at: user.created_at.map(|dt| dt.naive_utc().and_utc()),
                    };

                    let response = MobileAuthResponse {
                        success: true,
                        message: "Kayıt başarılı".to_string(),
                        user: Some(user_response),
                        tokens: Some(tokens),
                    };
                    (StatusCode::CREATED, Json(response)).into_response()
                }
                Err(e) => {
                    let response = MobileAuthResponse {
                        success: false,
                        message: format!("Token oluşturma hatası: {}", e),
                        user: None,
                        tokens: None,
                    };
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
                }
            }
        }
        Err(e) => {
            let response = MobileAuthResponse {
                success: false,
                message: e.to_string(),
                user: None,
                tokens: None,
            };
            (StatusCode::BAD_REQUEST, Json(response)).into_response()
        }
    }
}

/// Mobile API: Refresh access token
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(json): Json<RefreshTokenRequest>,
) -> Response {
    let jwt_config = get_jwt_config();

    // Validate the refresh token
    match validate_refresh_token(&json.refresh_token, &jwt_config) {
        Ok(claims) => {
            // Get fresh user data
            match auth_service::get_user_by_id(&state.db, claims.sub).await {
                Ok(user) => {
                    // Generate new token pair
                    match generate_token_pair(user.id, &user.username, &user.email, &jwt_config) {
                        Ok(tokens) => {
                            let user_response = MobileUserResponse {
                                id: user.id,
                                username: user.username,
                                email: user.email,
                                first_name: user.first_name,
                                last_name: user.last_name,
                                profile: user.profile,
                                user_type: user.user_type.clone(),
                                created_at: user.created_at.map(|dt| dt.naive_utc().and_utc()),
                            };

                            let response = MobileAuthResponse {
                                success: true,
                                message: "Token refreshed successfully".to_string(),
                                user: Some(user_response),
                                tokens: Some(tokens),
                            };
                            (StatusCode::OK, Json(response)).into_response()
                        }
                        Err(e) => {
                            let response = MobileAuthResponse {
                                success: false,
                                message: format!("Token generation failed: {}", e),
                                user: None,
                                tokens: None,
                            };
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(response)).into_response()
                        }
                    }
                }
                Err(e) => {
                    let response = MobileAuthResponse {
                        success: false,
                        message: format!("User not found: {}", e),
                        user: None,
                        tokens: None,
                    };
                    (StatusCode::NOT_FOUND, Json(response)).into_response()
                }
            }
        }
        Err(e) => {
            let response = MobileAuthResponse {
                success: false,
                message: format!("Invalid refresh token: {}", e),
                user: None,
                tokens: None,
            };
            (StatusCode::UNAUTHORIZED, Json(response)).into_response()
        }
    }
}

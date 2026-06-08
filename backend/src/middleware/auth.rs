use crate::config::get_config;
use crate::modules::auth::helpers::jwt::{extract_bearer_token, validate_access_token, JwtConfig};
// use crate::modules::auth::models::UserModel;
// use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{header::HeaderMap, request::Parts, StatusCode},
    response::{IntoResponse, Json, Response},
};
// use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_sessions::Session;
/// Yardımcı: Kimlik doğrulama doğrulaması (JWT & Oturum)
/// Hem kimliği doğrulanmış hem de misafir kullanıcılar için user_id döndürür
pub async fn verify_auth(headers: &HeaderMap, session: &Session) -> Option<i64> {
    // 1. JWT'yi dene (sadece kimliği doğrulanmış kullanıcılar için)
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = extract_bearer_token(auth_str) {
                let config = get_config();
                let jwt_config = JwtConfig {
                    secret: config.jwt_secret().to_string(),
                    access_token_expiry: config.jwt_access_token_expiry(),
                    refresh_token_expiry: config.jwt_refresh_token_expiry(),
                };

                if let Ok(claims) = validate_access_token(token, &jwt_config) {
                    return Some(claims.sub);
                }
            }
        }
    }

    // 2. Oturum Kontrolü (hem kimliği doğrulanmış hem de misafir kullanıcılar için çalışır)
    if let Ok(Some(user_id)) = session.get::<i64>("user_id").await {
        return Some(user_id);
    }

    None
}

/// Çıkarıcı: Kimliği Doğrulanmış Kullanıcı (Gerekli)
/// Kimlik doğrulaması yoksa, otomatik olarak misafir kullanıcı oluşturur
pub struct AuthenticatedUser {
    pub id: i64,
}

// #[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
    crate::app_state::AppState: axum::extract::FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Session Error").into_response())?;

        match verify_auth(&parts.headers, &session).await {
            Some(id) => Ok(AuthenticatedUser { id }),
            None => {
                // Otomatik misafir kullanıcı oluştur
                let app_state =
                    <crate::app_state::AppState as axum::extract::FromRef<S>>::from_ref(state);
                let client_ip =
                    crate::modules::utils::ip_helper::get_client_ip_from_headers(&parts.headers);

                match crate::modules::auth::helpers::session::ensure_guest_session(
                    &app_state.db,
                    &session,
                    client_ip,
                )
                .await
                {
                    Ok(id) => Ok(AuthenticatedUser { id }),
                    Err(e) => {
                        eprintln!("Guest creation error: {}", e);
                        let error_response = json!({
                            "status": "error",
                            "message": "Authentication failed and guest creation failed"
                        });
                        Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response))
                            .into_response())
                    }
                }
            }
        }
    }
}

/// Çıkarıcı: İsteğe Bağlı Kullanıcı (Misafir veya Kimliği Doğrulanmış)
#[allow(dead_code)]
pub struct MaybeAuthenticatedUser {
    pub id: Option<i64>,
}

/// Çıkarıcı: Misafir/kimliği doğrulanmış ayrımı olan kullanıcı
#[allow(dead_code)]
pub struct UserWithGuestInfo {
    pub id: i64,
    pub is_guest: bool,
}

// #[async_trait]
impl<S> FromRequestParts<S> for MaybeAuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = match Session::from_request_parts(parts, state).await {
            Ok(s) => s,
            Err(_) => return Ok(MaybeAuthenticatedUser { id: None }),
        };

        Ok(MaybeAuthenticatedUser {
            id: verify_auth(&parts.headers, &session).await,
        })
    }
}

// #[async_trait]
impl<S> FromRequestParts<S> for UserWithGuestInfo
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Session Error").into_response())?;

        match verify_auth(&parts.headers, &session).await {
            Some(id) => {
                // is_guest bayrağına bakarak bunun misafir kullanıcı olup olmadığını kontrol et
                // Şimdilik, oturumdan gelen tüm kullanıcıların potansiyel olarak misafir olduğunu varsayacağız
                // Bu, gerektiğinde servis katmanı tarafından belirlenecektir
                Ok(UserWithGuestInfo {
                    id,
                    is_guest: false,
                }) // Servisler tarafından güncellenecek
            }
            None => {
                let error_response = json!({
                    "status": "error",
                    "message": "Unauthorized"
                });
                Err((StatusCode::UNAUTHORIZED, Json(error_response)).into_response())
            }
        }
    }
}

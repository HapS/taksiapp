// JWT Authentication Middleware for mobile apps
use crate::config::get_config;
use crate::modules::auth::helpers::jwt::{extract_bearer_token, validate_access_token, JwtConfig};
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde::Serialize;
use std::future::Future;

/// JWT Claims - Authorization header'dan çıkarılan kullanıcı bilgileri
/// 
/// Bu struct'ı handler'larda extractor olarak kullanarak JWT doğrulaması yapabilirsin.
/// Token geçersiz veya eksikse otomatik olarak 401 Unauthorized döner.
/// 
/// # Kullanım Örneği
/// ```rust
/// pub async fn protected_endpoint(
///     State(state): State<AppState>,
///     claims: JwtClaims,  // <-- Token yoksa veya geçersizse 401 döner
/// ) -> Response {
///     // claims.user_id ile kullanıcıya erişebilirsin
///     let user_id = claims.user_id;
///     // ...
/// }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct JwtClaims {
    pub user_id: i64,
    pub username: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
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

/// Helper function to create unauthorized response
fn unauthorized_response(message: &str) -> Response {
    let error = ErrorResponse {
        success: false,
        error: message.to_string(),
    };
    (StatusCode::UNAUTHORIZED, Json(error)).into_response()
}

/// Axum extractor for JWT claims
/// Use this in handlers that require JWT authentication
impl<S> FromRequestParts<S> for JwtClaims
where
    S: Send + Sync,
{
    type Rejection = Response;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // Extract values we need before the async block
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        async move {
            let auth_header = match auth_header {
                Some(h) => h,
                None => {
                    return Err(unauthorized_response("Missing Authorization header"));
                }
            };

            // Extract Bearer token
            let token = match extract_bearer_token(&auth_header) {
                Some(t) => t,
                None => {
                    return Err(unauthorized_response("Invalid Authorization header format"));
                }
            };

            // Validate token
            let jwt_config = get_jwt_config();
            match validate_access_token(token, &jwt_config) {
                Ok(claims) => Ok(JwtClaims {
                    user_id: claims.sub,
                    username: claims.username,
                    email: claims.email,
                }),
                Err(e) => Err(unauthorized_response(&format!("Invalid token: {}", e))),
            }
        }
    }
}

/// JWT Authentication Middleware - Birden fazla route'u korumak için
/// 
/// Bu middleware'i Router::layer() ile kullanarak tüm route grubunu
/// JWT doğrulaması ile koruyabilirsin. Token geçersizse 401 döner.
/// 
/// # Kullanım Örneği
/// ```rust
/// use axum::middleware;
/// use crate::middleware::jwt::jwt_auth_middleware;
/// 
/// // Protected routes - HEPSİ JWT gerektirir
/// let protected = Router::new()
///     .route("/api/mobile/orders", get(list_orders))
///     .route("/api/mobile/cart", post(add_to_cart))
///     .route("/api/mobile/profile", get(get_profile))
///     .layer(middleware::from_fn(jwt_auth_middleware)); // <-- Tümüne uygulanır
/// ```
#[allow(dead_code)]
pub async fn jwt_auth_middleware(request: Request<Body>, next: Next) -> Response {
    // Get Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let auth_header = match auth_header {
        Some(h) => h,
        None => {
            return unauthorized_response("Missing Authorization header");
        }
    };

    // Extract Bearer token
    let token = match extract_bearer_token(auth_header) {
        Some(t) => t,
        None => {
            return unauthorized_response("Invalid Authorization header format");
        }
    };

    // Validate token
    let jwt_config = get_jwt_config();
    if let Err(e) = validate_access_token(token, &jwt_config) {
        return unauthorized_response(&format!("Invalid token: {}", e));
    }

    // Token is valid, proceed with the request
    next.run(request).await
}

/// Optional JWT Claims - Opsiyonel authentication için
/// 
/// Token yoksa veya geçersizse None döner, 401 hatası VERMEZ.
/// Hem giriş yapmış hem de misafir kullanıcıları destekleyen endpoint'ler için ideal.
/// 
/// # Kullanım Örneği
/// ```rust
/// pub async fn product_list(
///     State(state): State<AppState>,
///     OptionalJwtClaims(maybe_claims): OptionalJwtClaims,
/// ) -> Response {
///     match maybe_claims {
///         Some(claims) => {
///             // Giriş yapmış kullanıcı - kişiselleştirilmiş içerik
///             get_personalized_products(claims.user_id)
///         }
///         None => {
///             // Misafir kullanıcı - genel içerik
///             get_public_products()
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OptionalJwtClaims(pub Option<JwtClaims>);

impl<S> FromRequestParts<S> for OptionalJwtClaims
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // Extract values we need before the async block
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        async move {
            let auth_header = match auth_header {
                Some(h) => h,
                None => return Ok(OptionalJwtClaims(None)),
            };

            // Extract Bearer token
            let token = match extract_bearer_token(&auth_header) {
                Some(t) => t,
                None => return Ok(OptionalJwtClaims(None)),
            };

            // Validate token
            let jwt_config = get_jwt_config();
            match validate_access_token(token, &jwt_config) {
                Ok(claims) => Ok(OptionalJwtClaims(Some(JwtClaims {
                    user_id: claims.sub,
                    username: claims.username,
                    email: claims.email,
                }))),
                Err(_) => Ok(OptionalJwtClaims(None)),
            }
        }
    }
}

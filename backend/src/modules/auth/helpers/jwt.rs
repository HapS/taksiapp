// JWT Helper - Token generation and validation for mobile apps
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user_id)
    pub sub: i64,
    /// Username
    pub username: String,
    /// Email
    pub email: String,
    /// Expiration time (Unix timestamp)
    pub exp: u64,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Token type: "access" or "refresh"
    pub token_type: String,
}

/// JWT Token pair response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

/// JWT Error types
#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("Token creation failed: {0}")]
    TokenCreation(String),
    
    #[error("Token validation failed: {0}")]
    TokenValidation(String),
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Invalid token type")]
    InvalidTokenType,
}

/// JWT configuration
pub struct JwtConfig {
    pub secret: String,
    pub access_token_expiry: u64,  // seconds
    pub refresh_token_expiry: u64, // seconds
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-super-secret-jwt-key-change-in-production".to_string()),
            access_token_expiry: 3600,       // 1 hour
            refresh_token_expiry: 2592000,   // 30 days
        }
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Generate access token
pub fn generate_access_token(
    user_id: i64,
    username: &str,
    email: &str,
    config: &JwtConfig,
) -> Result<String, JwtError> {
    let now = current_timestamp();
    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        email: email.to_string(),
        exp: now + config.access_token_expiry,
        iat: now,
        token_type: "access".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(|e| JwtError::TokenCreation(e.to_string()))
}

/// Generate refresh token
pub fn generate_refresh_token(
    user_id: i64,
    username: &str,
    email: &str,
    config: &JwtConfig,
) -> Result<String, JwtError> {
    let now = current_timestamp();
    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        email: email.to_string(),
        exp: now + config.refresh_token_expiry,
        iat: now,
        token_type: "refresh".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(|e| JwtError::TokenCreation(e.to_string()))
}

/// Generate both access and refresh tokens
pub fn generate_token_pair(
    user_id: i64,
    username: &str,
    email: &str,
    config: &JwtConfig,
) -> Result<TokenPair, JwtError> {
    let access_token = generate_access_token(user_id, username, email, config)?;
    let refresh_token = generate_refresh_token(user_id, username, email, config)?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: config.access_token_expiry,
    })
}

/// Validate and decode a token
pub fn validate_token(token: &str, config: &JwtConfig) -> Result<TokenData<Claims>, JwtError> {
    let mut validation = Validation::default();
    validation.validate_exp = true;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        if e.kind() == &jsonwebtoken::errors::ErrorKind::ExpiredSignature {
            JwtError::TokenExpired
        } else {
            JwtError::TokenValidation(e.to_string())
        }
    })
}

/// Validate access token specifically
pub fn validate_access_token(token: &str, config: &JwtConfig) -> Result<Claims, JwtError> {
    let token_data = validate_token(token, config)?;
    
    if token_data.claims.token_type != "access" {
        return Err(JwtError::InvalidTokenType);
    }
    
    Ok(token_data.claims)
}

/// Validate refresh token specifically
pub fn validate_refresh_token(token: &str, config: &JwtConfig) -> Result<Claims, JwtError> {
    let token_data = validate_token(token, config)?;
    
    if token_data.claims.token_type != "refresh" {
        return Err(JwtError::InvalidTokenType);
    }
    
    Ok(token_data.claims)
}

/// Extract token from Authorization header
/// Supports: "Bearer <token>" format
pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_validation() {
        let config = JwtConfig::default();
        
        let token_pair = generate_token_pair(1, "testuser", "test@example.com", &config).unwrap();
        
        assert!(!token_pair.access_token.is_empty());
        assert!(!token_pair.refresh_token.is_empty());
        assert_eq!(token_pair.token_type, "Bearer");
        
        // Validate access token
        let claims = validate_access_token(&token_pair.access_token, &config).unwrap();
        assert_eq!(claims.sub, 1);
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.token_type, "access");
        
        // Validate refresh token
        let claims = validate_refresh_token(&token_pair.refresh_token, &config).unwrap();
        assert_eq!(claims.sub, 1);
        assert_eq!(claims.token_type, "refresh");
    }

    #[test]
    fn test_extract_bearer_token() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer abc123"), None);
        assert_eq!(extract_bearer_token("abc123"), None);
    }
}

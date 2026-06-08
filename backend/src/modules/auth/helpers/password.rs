// Password Helper - bcrypt operations
use anyhow::Result;
use bcrypt::{hash, verify, DEFAULT_COST};

/// Hash a password using bcrypt
pub fn hash_password(password: &str) -> Result<String> {
    hash(password, DEFAULT_COST).map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    verify(password, hash).map_err(|e| anyhow::anyhow!("Failed to verify password: {}", e))
}

/// Validate password strength
pub fn validate_password_strength(password: &str) -> Result<()> {
    if password.len() < 6 {
        return Err(anyhow::anyhow!(
            "Password must be at least 6 characters long"
        ));
    }
    Ok(())
}

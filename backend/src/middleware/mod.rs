// Middleware modules
pub mod auth;
pub mod error_handler;
pub mod global_context;
pub mod jwt;
pub mod logger;
pub mod permission;

// Re-export JWT claims for easy access
#[allow(unused_imports)]
pub use jwt::{JwtClaims, OptionalJwtClaims};

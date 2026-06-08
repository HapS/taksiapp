// Data Transfer Objects (DTOs) for Admin Controllers
// Shared models between web and API controllers

pub mod content;

// Re-exports for convenience
pub use content::{CreateContentRequest, ContentExtensions, UpdateContentRequest};

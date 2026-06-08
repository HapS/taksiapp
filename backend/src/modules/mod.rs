// Modern modular architecture - each module in its own directory
pub mod admin; // Admin interface
pub mod auth; // Authentication & user management
pub mod b2b; // B2B - business to business
pub mod background_tasks; // Background tasks - periodic jobs
pub mod bookmarks;
pub mod comment;
pub mod content; // Base content model + frontend pages
pub mod currency; // Currency exchange rates - TCMB integration
pub mod ecommerce; // E-commerce - cart, orders
pub mod form; // Form submissions - contact, HR, etc.
pub mod iot; // IoT - ESP32 device communication
pub mod mailer; // Mail system - templates & queue
pub mod media; // Media management - file uploads
pub mod payment_provider; // Payment providers - iyzico, garanti, etc.
pub mod ride;
pub mod location;
pub mod search;
pub mod static_files; // Static files - theme-aware static file serving
pub mod taxonomy; // Taxonomy system - vocabularies & terms
pub mod timeline; // Timeline & activity tracking
pub mod utils; // Utility functions // Search - searching functionality // Ride - taksi uygulaması modülü

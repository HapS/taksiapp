use serde::Serialize;

#[derive(Serialize)]
pub struct AppVersion {
    pub version: &'static str,
    pub name: &'static str,
}

impl AppVersion {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            name: env!("CARGO_PKG_NAME"),
        }
    }
}

impl Default for AppVersion {
    fn default() -> Self {
        Self::new()
    }
}

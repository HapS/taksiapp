use axum::{
    extract::{ConnectInfo, Request},
    response::Response,
};
use colored::*;
use futures_util::future::BoxFuture;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use std::time::Instant;
use tower::{Layer, Service};

#[derive(Clone)]
pub struct LoggerLayer;

impl LoggerLayer {
    pub fn new() -> Self {
        // Renklerin her zaman çalışmasını garantile (bacon için)
        colored::control::set_override(true);
        Self
    }
}

impl<S> Layer<S> for LoggerLayer {
    type Service = LoggerMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LoggerMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct LoggerMiddleware<S> {
    inner: S,
}

impl<S> Service<Request> for LoggerMiddleware<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let start = Instant::now();

        // Get client IP from multiple sources
        let client_ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                req.headers()
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
            })
            .or_else(|| {
                // Try to get from connection info
                req.extensions()
                    .get::<ConnectInfo<SocketAddr>>()
                    .map(|ConnectInfo(addr)| addr.ip().to_string())
            })
            .unwrap_or_else(|| "127.0.0.1".to_string());

        let future = self.inner.call(req);

        Box::pin(async move {
            let res = future.await?;
            let status = res.status().as_u16();
            let duration = start.elapsed();

            // Format duration with color
            let duration_ms = duration.as_millis();
            let duration_str = if duration_ms > 0 {
                let color_duration = match duration_ms {
                    0..=10 => format!("{}ms", duration_ms).bright_green(),
                    11..=50 => format!("{}ms", duration_ms).green(),
                    51..=100 => format!("{}ms", duration_ms).yellow(),
                    101..=500 => format!("{}ms", duration_ms).bright_yellow(),
                    _ => format!("{}ms", duration_ms).red(),
                };
                color_duration.to_string()
            } else {
                format!("{}µs", duration.as_micros())
                    .bright_cyan()
                    .to_string()
            };

            // Get current time
            let now = chrono::Local::now();
            let timestamp = now.format("%H:%M:%S").to_string().bright_black();

            // Config'den ignore path'leri al
            let config = crate::config::get_config();
            let should_log = config.should_log_path(&path);

            // HTTP request log'ları her zaman çalışsın
            if should_log {
                // Status ile renk ve emoji
                let (status_str, status_emoji) = match status {
                    200..=299 => (format!("{}", status).bright_green(), "✓".bright_green()),
                    300..=399 => (format!("{}", status).bright_cyan(), "↻".bright_cyan()),
                    400..=499 => (format!("{}", status).bright_yellow(), "⚠".bright_yellow()),
                    500..=599 => (format!("{}", status).bright_red(), "✗".bright_red()),
                    _ => (format!("{}", status).white(), "•".white()),
                };

                // Method renklendirme
                let method_str = match method.as_str() {
                    "GET" => {
                        if path.contains("/api/") {
                            method.bright_cyan()
                        } else {
                            method.bright_green()
                        }
                    }
                    "POST" => {
                        if path.contains("/api/") {
                            method.bright_blue()
                        } else {
                            method.bright_green()
                        }
                    }
                    "PUT" => {
                        if path.contains("/api/") {
                            method.bright_yellow()
                        } else {
                            method.bright_green()
                        }
                    }
                    "DELETE" => {
                        if path.contains("/api/") {
                            method.bright_red()
                        } else {
                            method.bright_green()
                        }
                    }
                    "PATCH" => {
                        if path.contains("/api/") {
                            method.bright_magenta()
                        } else {
                            method.bright_green()
                        }
                    }
                    _ => method.white(),
                };

                // Path renklendirme
                let path_str = if path.starts_with("/admin") {
                    path.bright_magenta()
                } else if path.starts_with("/api") {
                    path.bright_cyan()
                } else {
                    path.yellow()
                };

                // IP renklendirme
                let ip_str = client_ip.bright_black();

                // Modern log formatı
                println!(
                    "{} {} {} {} -> {} | {} │ {}",
                    timestamp, status_emoji, status_str, method_str, path_str, duration_str, ip_str
                );
            }

            Ok(res)
        })
    }
}

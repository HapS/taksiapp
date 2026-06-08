use axum::http::HeaderMap;
use std::net::SocketAddr;

/// Extracts the client IP address from request headers or connection info.
///
/// Priority order:
/// 1. X-Forwarded-For header (first IP in the list)
/// 2. X-Real-IP header
/// 3. CF-Connecting-IP header (Cloudflare)
/// 4. Connection socket address
///
/// Returns `Some(String)` with the IP address, or `None` if unable to determine.
pub fn get_client_ip(headers: &HeaderMap, conn_info: Option<SocketAddr>) -> Option<String> {
    // 1. X-Forwarded-For header (proxy/load balancer)
    if let Some(forwarded_for) = headers.get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            // X-Forwarded-For can contain multiple IPs: "client, proxy1, proxy2"
            // The first one is the original client IP
            if let Some(first_ip) = value.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
    }

    // 2. X-Real-IP header (nginx)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            let ip = value.trim();
            if !ip.is_empty() {
                return Some(ip.to_string());
            }
        }
    }

    // 3. CF-Connecting-IP header (Cloudflare)
    if let Some(cf_ip) = headers.get("cf-connecting-ip") {
        if let Ok(value) = cf_ip.to_str() {
            let ip = value.trim();
            if !ip.is_empty() {
                return Some(ip.to_string());
            }
        }
    }

    // 4. Fallback to connection socket address
    if let Some(addr) = conn_info {
        return Some(addr.ip().to_string());
    }

    None
}

/// Simplified version that only uses headers (for use in handlers without ConnectInfo)
pub fn get_client_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    get_client_ip(headers, None)
}

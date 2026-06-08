use serde_json;
use tera::{self};

pub fn register_filters(tera: &mut tera::Tera) {
    // pretty_json
    tera.register_filter(
        "pretty_json",
        |value: &serde_json::Value,
         _args: &std::collections::HashMap<String, serde_json::Value>| {
            match serde_json::to_string_pretty(value) {
                Ok(pretty) => Ok(serde_json::Value::String(pretty)),
                Err(_) => Ok(value.clone()),
            }
        },
    );

    // format_price
    tera.register_filter(
        "format_price",
        |value: &serde_json::Value, _args: &std::collections::HashMap<String, serde_json::Value>| {
            let price = value.as_f64().unwrap_or(0.0);
            let formatted = crate::modules::utils::format_price::format_price(price, "TRY");
            Ok(serde_json::Value::String(formatted))
        },
    );

    // format_price_no_symbol
    tera.register_filter(
        "format_price_no_symbol",
        |value: &serde_json::Value, _args: &std::collections::HashMap<String, serde_json::Value>| {
            let price = value.as_f64().unwrap_or(0.0);
            let formatted =
                crate::modules::utils::format_price::format_price_no_symbol(price, "TRY");
            Ok(serde_json::Value::String(formatted))
        },
    );

    // slugify
    tera.register_filter(
        "slugify",
        |value: &serde_json::Value,
         _args: &std::collections::HashMap<String, serde_json::Value>| {
            let text = value.as_str().unwrap_or("");

            let slug = text
                .to_lowercase()
                .replace('ı', "i")
                .replace('ğ', "g")
                .replace('ü', "u")
                .replace('ş', "s")
                .replace('ö', "o")
                .replace('ç', "c")
                .replace('İ', "i")
                .replace('Ğ', "g")
                .replace('Ü', "u")
                .replace('Ş', "s")
                .replace('Ö', "o")
                .replace('Ç', "c")
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .split('-')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("-");

            Ok(serde_json::Value::String(slug))
        },
    );

    // thumb
    tera.register_filter(
        "thumb",
        |value: &serde_json::Value, args: &std::collections::HashMap<String, serde_json::Value>| {
            let path = value.as_str().unwrap_or("");
            if path.is_empty() {
                return Ok(serde_json::Value::String(
                    "/static/no_image.png".to_string(),
                ));
            }

            let size = args
                .get("size")
                .and_then(|v| v.as_str())
                .unwrap_or("200x200");

            let crop = args.get("crop").and_then(|v| v.as_str()).unwrap_or("fit");

            // Strip common prefixes if they exist
            let clean_path = if let Some(stripped) = path.strip_prefix("/media/uploads/") {
                stripped
            } else if let Some(stripped) = path.strip_prefix("media/uploads/") {
                stripped
            } else if let Some(stripped) = path.strip_prefix("/media/") {
                stripped
            } else {
                path
            };

            Ok(serde_json::Value::String(format!(
                "/media/thumb/{}/{}/{}",
                size, crop, clean_path
            )))
        },
    );
}

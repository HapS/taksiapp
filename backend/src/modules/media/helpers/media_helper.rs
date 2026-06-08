use crate::config::get_config;
use serde_json::Value;
//TEŞEKKÜRLER GPT GEMINI

/// Resolve media fallbacks for a specific language
/// If media (cover, gallery, etc.) is empty in target_lang, it tries to pull from fallback_lang
pub fn resolve_media_fallbacks(data: &mut Value, target_lang: &str) {
    let config = get_config();
    let default_lang = &config.default_language;

    // 1. Get fallback candidates list safely
    let mut fallback_candidates = vec![default_lang.to_string()];
    if let Some(langs) = data.get("langs").and_then(|l| l.as_object()) {
        for lang in langs.keys() {
            if lang != target_lang && lang != default_lang {
                fallback_candidates.push(lang.to_string());
            }
        }
    }

    let media_types = ["cover", "gallery", "video", "icon", "document"];
    let mut found_fallbacks = std::collections::HashMap::new();

    // 2. Identify missing media and find fallbacks using immutable access
    if let Some(langs_obj) = data.get("langs").and_then(|l| l.as_object()) {
        if let Some(target_data) = langs_obj.get(target_lang).and_then(|t| t.as_object()) {
            let target_media = target_data.get("media");

            for m_type in media_types {
                let is_empty = match target_media.and_then(|m| m.get(m_type)) {
                    Some(v) => v.as_array().map_or(true, |a| a.is_empty()),
                    None => true,
                };

                if is_empty {
                    // Try to find in candidates
                    for fallback_lang in &fallback_candidates {
                        if fallback_lang == target_lang {
                            continue;
                        }

                        if let Some(fallback_data) = langs_obj.get(fallback_lang) {
                            if let Some(fb_media_val) =
                                fallback_data.get("media").and_then(|m| m.get(m_type))
                            {
                                if !fb_media_val.as_array().map_or(true, |a| a.is_empty()) {
                                    found_fallbacks
                                        .insert(m_type.to_string(), fb_media_val.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Apply found fallbacks using mutable access
    if !found_fallbacks.is_empty() {
        if let Some(langs_obj) = data.get_mut("langs").and_then(|l| l.as_object_mut()) {
            if let Some(target_data) = langs_obj
                .get_mut(target_lang)
                .and_then(|t| t.as_object_mut())
            {
                let target_media_obj = target_data
                    .entry("media")
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut();
                if let Some(target_media_obj) = target_media_obj {
                    for (m_type, fb_val) in found_fallbacks {
                        target_media_obj.insert(m_type, fb_val);
                    }
                }
            }
        }
    }
}
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yml;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LocaleData {
    pub keys: IndexMap<String, HashMap<String, String>>,
    pub languages: Vec<String>,
}

pub fn get_locales(
    supported_langs: &[String],
    theme: &str,
) -> Result<LocaleData, Box<dyn std::error::Error>> {
    let locales_dir = get_locales_path(theme);
    let mut all_keys: IndexMap<String, HashMap<String, String>> = IndexMap::new();

    // Önce desteklenen diller için dosyaları tara
    for lang in supported_langs {
        let file_path = locales_dir.join(format!("{}.yml", lang));
        if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let translations: HashMap<String, String> = serde_yml::from_str(&content)?;

            for (key, value) in translations {
                all_keys
                    .entry(key)
                    .or_insert_with(HashMap::new)
                    .insert(lang.clone(), value);
            }
        }
    }

    // Eksik anahtarları doldur (senkronizasyon için)
    for lang in supported_langs {
        for values in all_keys.values_mut() {
            if !values.contains_key(lang) {
                values.insert(lang.clone(), "".to_string());
            }
        }
    }

    Ok(LocaleData {
        keys: all_keys,
        languages: supported_langs.to_vec(),
    })
}

fn get_locales_path(theme: &str) -> std::path::PathBuf {
    if theme.is_empty() || theme == "default" {
        Path::new("locales").to_path_buf()
    } else {
        Path::new(&format!("templates/{}/locales", theme)).to_path_buf()
    }
}

pub fn save_locales(data: &LocaleData, theme: &str) -> Result<(), Box<dyn std::error::Error>> {
    let locales_dir = get_locales_path(theme);

    // Klasör yoksa oluştur
    if !locales_dir.exists() {
        fs::create_dir_all(&locales_dir)?;
    }

    // Her dil için ayrı dosya oluştur
    for lang in &data.languages {
        let mut lang_translations = IndexMap::new();

        for (key, values) in &data.keys {
            if let Some(value) = values.get(lang) {
                lang_translations.insert(key.clone(), value.clone());
            } else {
                lang_translations.insert(key.clone(), "".to_string());
            }
        }

        let content = serde_yml::to_string(&lang_translations)?;
        let file_path = locales_dir.join(format!("{}.yml", lang));
        fs::write(file_path, content)?;
    }

    Ok(())
}

pub fn update_key(
    key: String,
    translations: HashMap<String, String>,
    supported_langs: &[String],
    theme: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut data = get_locales(supported_langs, theme)?;
    data.keys.insert(key, translations);
    save_locales(&data, theme)
}

pub fn delete_key(
    key: &str,
    supported_langs: &[String],
    theme: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut data = get_locales(supported_langs, theme)?;
    data.keys.shift_remove(key);
    save_locales(&data, theme)
}

// Runtime i18n - YAML dosyalarını uygulama başlatıldığında yükler
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

pub type Translations = HashMap<String, HashMap<String, String>>; // lang -> key -> value

lazy_static::lazy_static! {
    static ref FILE_WRITE_LOCK: StdMutex<()> = StdMutex::new(());
}

thread_local! {
    static CURRENT_THEME_I18N: std::cell::RefCell<Option<I18n>> = const { std::cell::RefCell::new(None) };
}

pub fn set_current_theme_i18n(i18n: I18n) {
    CURRENT_THEME_I18N.with(|cell| {
        *cell.borrow_mut() = Some(i18n);
    });
}

pub fn get_current_theme_i18n() -> Option<I18n> {
    CURRENT_THEME_I18N.with(|cell| cell.borrow().clone())
}

#[derive(Clone)]
pub struct I18n {
    translations: Arc<RwLock<Translations>>,
    pub theme: String,
}

impl I18n {
    // Yeni i18n instance oluştur - boş çevirilerle
    pub fn new() -> Self {
        Self {
            translations: Arc::new(RwLock::new(HashMap::new())),
            theme: "default".to_string(),
        }
    }

    // Belirli bir theme için i18n instance oluştur
    pub fn with_theme(theme: &str) -> Self {
        // Default veya boş theme için boş i18n döndür (root locales kullanma)
        if theme == "default" || theme.is_empty() {
            return Self::new();
        }

        let mut translations = HashMap::new();
        let locales_path = format!("templates/{}/locales", theme);

        // Theme locales klasörü yoksa oluştur
        let locales_dir = std::path::Path::new(&locales_path);
        if !locales_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(locales_dir) {
                eprintln!("⚠️  Locales klasörü oluşturulamadı: {}", e);
            } else {
                eprintln!("📁 Locales klasörü oluşturuldu: {}", locales_path);
            }
        }

        // Varsayılan dil dosyaları yoksa oluştur
        let tr_file = locales_dir.join("tr.yml");
        let en_file = locales_dir.join("en.yml");

        if !tr_file.exists() {
            if let Err(e) = std::fs::write(&tr_file, "# Türkçe Çeviriler\n") {
                eprintln!("⚠️  tr.yml oluşturulamadı: {}", e);
            } else {
                eprintln!("📝 tr.yml oluşturuldu: {}", tr_file.display());
            }
        }

        if !en_file.exists() {
            if let Err(e) = std::fs::write(&en_file, "# English Translations\n") {
                eprintln!("⚠️  en.yml oluşturulamadı: {}", e);
            } else {
                eprintln!("📝 en.yml oluşturuldu: {}", en_file.display());
            }
        }

        // Theme locales klasöründeki dosyaları yükle
        if let Ok(entries) = std::fs::read_dir(&locales_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .map(|e| e == "yml" || e == "yaml")
                    .unwrap_or(false)
                {
                    if let Some(lang) = path.file_stem().and_then(|s| s.to_str()) {
                        match Self::load_yaml_file(&path) {
                            Ok(lang_translations) => {
                                println!(
                                    "🌍 [{}] Dil yüklendi: {} ({} anahtar)",
                                    theme,
                                    lang,
                                    lang_translations.len()
                                );
                                translations.insert(lang.to_string(), lang_translations);
                            }
                            Err(e) => {
                                eprintln!("⚠️  [{}] Dil yüklenemedi {}: {}", theme, lang, e);
                            }
                        }
                    }
                }
            }
        }

        Self {
            translations: Arc::new(RwLock::new(translations)),
            theme: theme.to_string(),
        }
    }

    // YAML dosyasını oku ve HashMap'e dönüştür
    fn load_yaml_file(path: &std::path::Path) -> Result<HashMap<String, String>, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Dosya okuma hatası: {}", e))?;

        let yaml: serde_yml::Value =
            serde_yml::from_str(&content).map_err(|e| format!("YAML ayrıştırma hatası: {}", e))?;

        let mut result = HashMap::new();

        if let serde_yml::Value::Mapping(map) = yaml {
            for (key, value) in map {
                if let (serde_yml::Value::String(k), serde_yml::Value::String(v)) = (key, value) {
                    result.insert(k, v);
                }
            }
        }

        Ok(result)
    }

    // Çeviri al - default parametresi ile
    pub fn t_with_default(&self, key: &str, lang: &str, default: Option<&str>) -> String {
        // Önce cache'den kontrol et
        {
            let translations = self.translations.read();

            // Önce istenen dilde ara
            if let Some(lang_map) = translations.get(lang) {
                if let Some(value) = lang_map.get(key) {
                    return value.clone();
                }
            }

            // Bulunamazsa default dilde (tr) ara
            if lang != "tr" {
                if let Some(lang_map) = translations.get("tr") {
                    if let Some(value) = lang_map.get(key) {
                        return value.clone();
                    }
                }
            }
        } // read lock burada drop oluyor

        // Hiç bulunamazsa ve default varsa
        if let Some(default_value) = default {
            // Key'i YAML dosyasına ekle (artık read lock yok)
            self.add_missing_key(key, default_value, lang);
            return default_value.to_string();
        }

        // Default yoksa key'i döndür
        key.to_string()
    }

    // Eksik key'i YAML dosyasına ekle (thread-safe)
    fn add_missing_key(&self, key: &str, value: &str, lang: &str) {
        // Default theme için dosyaya yazma (artık root locales kullanılmıyor)
        if self.theme == "default" || self.theme.is_empty() {
            tracing::warn!("Default theme için i18n key'i yazılmadı: {}", key);
            return;
        }

        // Global lock - aynı anda sadece bir thread dosyaya yazabilir
        let _lock = match FILE_WRITE_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("⚠️  i18n lock poisoned, recovering...");
                poisoned.into_inner()
            }
        };

        // Önce cache'i kontrol et - başka thread zaten eklemiş olabilir
        {
            let translations = self.translations.read();
            if let Some(lang_map) = translations.get(lang) {
                if lang_map.contains_key(key) {
                    return; // Zaten eklenmiş
                }
            }
        }

        let yaml_path = format!("templates/{}/locales/{}.yml", self.theme, lang);

        // Klasör yoksa oluştur
        if let Some(parent) = std::path::Path::new(&yaml_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Escape double quotes in value
        let escaped_value = value.replace("\"", "\\\"");

        match std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&yaml_path)
        {
            Ok(mut file) => {
                use std::io::Write;
                match writeln!(file, "{}: \"{}\"", key, escaped_value) {
                    Ok(_) => {
                        eprintln!(
                            "📝 i18n [{}]: '{}' eklendi -> {} ({})",
                            self.theme, key, value, lang
                        );

                        // Cache'i güncelle
                        let mut translations = self.translations.write();
                        translations
                            .entry(lang.to_string())
                            .or_insert_with(HashMap::new)
                            .insert(key.to_string(), value.to_string());
                    }
                    Err(e) => {
                        eprintln!("❌ i18n [{}]: '{}' yazılamadı: {}", self.theme, key, e);
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "❌ i18n [{}]: {} dosyası açılamadı: {}",
                    self.theme, yaml_path, e
                );
            }
        }
    }

    // Çeviri al
    pub fn t(&self, key: &str, lang: &str) -> String {
        self.t_with_default(key, lang, None)
    }

    // Mevcut dilleri listele
    pub fn available_locales(&self) -> Vec<String> {
        self.translations.read().keys().cloned().collect()
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

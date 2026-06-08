/// Mailer modülü için seed işlemleri
/// Artık template'ler HTML dosyalarından okunduğu için seed'e gerek yok
pub struct SeedService;

impl SeedService {
    /// Mevcut template'leri listele (tema sisteminden)
    pub fn list_available_templates() -> Vec<String> {
        let mut templates = Vec::new();

        // Base tema'dan mail template'lerini listele (referans olarak)
        let template_dir = "templates/base/mailer";

        if let Ok(entries) = std::fs::read_dir(template_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.ends_with(".html") && file_name != "base.html" {
                            let template_name = file_name.replace(".html", "");
                            templates.push(template_name);
                        }
                    }
                }
            }
        }

        templates.sort();
        templates
    }

    /// Template'lerin mevcut olup olmadığını kontrol et (tema sisteminde)
    pub fn check_templates() -> Result<(), String> {
        let templates = Self::list_available_templates();

        if templates.is_empty() {
            return Err("Base tema'da hiç mail template'i bulunamadı!".to_string());
        }

        println!("✅ Mevcut mail template'leri (base tema):");
        for template in &templates {
            println!("   - {}.html", template);
        }

        // Mevcut temaları da kontrol et
        if let Ok(entries) = std::fs::read_dir("templates") {
            let mut theme_count = 0;
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Some(dir_name) = entry.file_name().to_str() {
                            if dir_name != "admin" {
                                let mailer_path = format!("templates/{}/mailer", dir_name);
                                if std::path::Path::new(&mailer_path).exists() {
                                    theme_count += 1;
                                }
                            }
                        }
                    }
                }
            }
            println!(
                "✅ İçinde mailer template'leri bulunan tema sayısı: {}",
                theme_count
            );
        }

        Ok(())
    }
}

use serde_json;

/// Tera template engine için i18n çeviri fonksiyonlarını kaydeder
///
/// Bu fonksiyon, Tera template'lerinde `t` fonksiyonunu kullanarak
/// çok dilli içerik yönetimi sağlar.
///
/// Thread-local'dan theme'e özgü i18n'i alır (render sırasında ayarlanır).
pub fn register_functions(tera: &mut tera::Tera, i18n: crate::i18n::I18n) {
    // Varsayılan i18n (fallback için)
    let default_i18n = i18n.clone();

    // "t" fonksiyonunu Tera'ya kaydet
    // Kullanım: {{ t(key="hello", lang="tr", default="Merhaba") }}
    tera.register_function(
        "t",
        move |args: &std::collections::HashMap<String, serde_json::Value>| {
            // Çeviri anahtarını al
            let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");

            // Hedef dil kodunu al
            let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("tr");

            // Varsayılan değer varsa al
            let default = args.get("default").and_then(|v| v.as_str());

            // Önce thread-local'dan theme i18n'i dene, yoksa default kullan
            let i18n = match crate::i18n::get_current_theme_i18n() {
                Some(theme_i18n) => theme_i18n,
                None => default_i18n.clone(),
            };

            // Çeviri işlemini gerçekleştir
            let translated = i18n.t_with_default(key, lang, default);

            Ok(serde_json::Value::String(translated))
        },
    );
}

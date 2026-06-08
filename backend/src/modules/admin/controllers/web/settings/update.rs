// Güvenlik Ayarları Denetleyicisi
use crate::app_state::AppState;
// use crate::modules::admin::services::settings_service;
// use crate::modules::admin::models::settings::SettingsData;
use axum::{
    extract::{State, Query},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::Multipart;
use tera::Context;
use tower_sessions::Session;
use std::path::PathBuf;
use std::fs::File;
use tokio::task;
use tokio::process::Command;
use std::os::unix::fs::PermissionsExt;
use zip::ZipArchive; 

// Ortak RBAC yardımcı fonksiyonunu kullan
use crate::modules::auth::helpers::rbac::has_admin_access as is_admin;


// bu şekilde update yapmaktan vazgeçiyorum ama code dursun
#[allow(dead_code)]
pub async fn system_update(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    // Mevcut ayarları al
    // let current_settings = match settings_service::get_settings(&state.db).await {
    //     Ok(settings) => settings,
    //     Err(e) => {
    //         eprintln!("Update error: {:?}", e);
    //         SettingsData::default()
    //     }
    // };

    let mut context = Context::new();
    context.insert("version", &"2.3.1");

    // Başarı/hata bayrakları için URL sorgu parametrelerini ayrıştır (kısa)
    context.insert(
        "upload_success",
        &query
            .get("success")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    );
    context.insert(
        "upload_error",
        &query
            .get("error")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    );

    match super::render_settings_page(
        &state,
        "update",
        "Güncelleme",
        "admin/settings/sections/update.html",
        context,
        None,
    ).await {
        Ok(html) => html.into_response(),
        Err(response) => response,
    }
}

/// Güvenlik ayarları sayfası - POST
#[allow(dead_code)]
pub async fn system_update_zip_file(
    State(state): State<AppState>,
    session: Session,
    mut multipart: Multipart,
) -> Response {
    if !is_admin(&state, &session).await {
        return Redirect::to("/admin/login").into_response();
    }

    let mut form_data = std::collections::HashMap::new();
    let mut processed_fields = std::collections::HashSet::new();
    // En az bir dosyanın yüklendiğini ve kaydedildiğini göstermek için bayrak
    let mut uploaded_any = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        processed_fields.insert(name.clone());


        if let Some(filename) = field.file_name().map(|s| s.to_string()) {
            let raw = filename
                .rsplit(|c| c == '/' || c == '\\')
                .next()
                .unwrap_or(&filename)
                .to_string();
            let safe_name: String = raw
                .chars()
                .filter(|c| !c.is_control())
                .map(|c| if c.is_whitespace() { '_' } else { c })
                .collect();

            let unique_name = format!("{}_{}", chrono::Utc::now().timestamp(), safe_name);

            let updates_dir = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("updates");

            if let Err(e) = tokio::fs::create_dir_all(&updates_dir).await {
                eprintln!("Güncellemeler dizini oluşturulamadı {:?}: {}", updates_dir, e);
                return Redirect::to("/admin/settings/update?upload_error=1").into_response();
            }

            let file_path = updates_dir.join(&unique_name);

            match field.bytes().await {
                Ok(bytes) => {
                    if let Err(e) = tokio::fs::write(&file_path, &bytes).await {
                        eprintln!("Güncelleme dosyası kaydedilemedi {:?}: {}", file_path, e);
                        return Redirect::to("/admin/settings/update?upload_error=1").into_response();
                    }

                    eprintln!("Güncelleme dosyası kaydedildi: {:?}", file_path);

                    // Yüklenen dosya ZIP (PK..) görünüyorsa, proje köküne çıkar
                    let is_zip = bytes.len() >= 4 && &bytes[..4] == b"PK\x03\x04";
                    if is_zip {
                        if let Err(e) = extract_zip_to_root(&file_path).await {
                            eprintln!("Zip çıkarılırken hata {:?}: {}", file_path, e);
                            return Redirect::to("/admin/settings/update?upload_error=1").into_response();
                        }
                        eprintln!("ZIP çıkarıldı: {:?}", file_path);
                    }

                    form_data.insert(name, file_path.to_string_lossy().to_string());
                    uploaded_any = true;
                }
                Err(e) => {
                    eprintln!("Error reading uploaded file {}: {}", filename, e);
                    return Redirect::to("/admin/settings/update?upload_error=1").into_response();
                }
            }
        } else {
            if let Ok(value) = field.text().await {
                form_data.insert(name, value);
            }
        }
    }

    if uploaded_any {
        Redirect::to("/admin/settings/update?success=1").into_response()
    } else {
        Redirect::to("/admin/settings/update?error=1").into_response()
    }
}


// zip dosyasını update.sh açacak, update sh dosyasını root a atacak, orada çalıştıracak
#[allow(dead_code)]
pub async fn extract_zip_to_root(zip_path: &std::path::Path) -> Result<(), String> {
    let zip_path = zip_path.to_path_buf();
    let root = std::env::current_dir().map_err(|e| e.to_string())?;
  
    let zip_path_for_closure = zip_path.clone();
    let root_for_closure = root.clone();

    let script_path_opt = task::spawn_blocking(move || -> Result<Option<std::path::PathBuf>, String> {
        let file = File::open(&zip_path_for_closure).map_err(|e| format!("Could not open zip file {:?}: {}", zip_path_for_closure, e))?;
        let mut archive = ZipArchive::new(file).map_err(|e| format!("Zip error: {}", e))?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("Zip index error: {}", e))?;
            let name = entry.name().to_string();
            if name.ends_with("update.sh") && !entry.name().ends_with('/') {
                let outpath = root_for_closure.join("update.sh");
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p).map_err(|e| format!("Could not create dir {:?}: {}", p, e))?;
                }

                // update.sh'i proje köküne yaz (varsa üzerine yaz)
                let mut outfile = File::create(&outpath).map_err(|e| format!("Could not create file {:?}: {}", outpath, e))?;
                std::io::copy(&mut entry, &mut outfile).map_err(|e| format!("Could not write update.sh {:?}: {}", outpath, e))?;

                return Ok(Some(outpath));
            }
        }

        Ok(None)
    })
    .await
    .map_err(|e| e.to_string())??;

    let script_to_run = if let Some(script) = script_path_opt {
        script
    } else {
        let existing = root.join("update.sh");
        if existing.exists() {
            existing
        } else {
            return Err(format!("No update.sh found in zip {:?} and no persistent update.sh at {:?}", zip_path, root.join("update.sh")));
        }
    };

    // Çalıştırılabilir olduğundan emin ol
    let mut perms = std::fs::metadata(&script_to_run).map_err(|e| format!("Could not stat {:?}: {}", script_to_run, e))?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script_to_run, perms).map_err(|e| format!("Could not set perms {:?}: {}", script_to_run, e))?;

    eprintln!("Güncelleme betiği (arka planda) çalıştırılıyor: {:?} arg zip: {:?}", script_to_run, zip_path);

    // Arka plan çalıştırması için bir log dosyası hazırla
    let logs_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);
    let log_file = logs_dir.join(format!("update_bg_{}.log", chrono::Utc::now().timestamp()));

    // Süreci tamamen ayırmak için nohup ile sudo altında bir shell kullan
    // Child sürecin .status marker'ını nereye yazacağını bilmesi için UPDATE_LOG ortam değişkenini geçir
    let shell_cmd = format!("UPDATE_LOG='{}' nohup '{}' '{}' >> '{}' 2>&1 &", log_file.display(), script_to_run.display(), zip_path.display(), log_file.display());

    let child = Command::new("sudo")
        .arg("sh")
        .arg("-c")
        .arg(&shell_cmd)
        .spawn()
        .map_err(|e| format!("Failed to spawn background update script {:?}: {}", script_to_run, e))?;

    eprintln!("Güncelleme betiği başlatıldı, arka plan PID {:?}, log: {:?}", child.id(), log_file);

    // update.sh'in yazacağı tamamlanma/.status markerını bekle (yavaş kopyalar için zaman aşımı daha uzun)
    let done_file = log_file.with_extension("log.status");
    let timeout_secs = 120u64;
    let mut waited = 0u64;

    while waited < timeout_secs {
        if done_file.exists() {
            // Marker dosyasını oku
            match std::fs::read_to_string(&done_file) {
                Ok(s) => {
                    let s = s.trim();
                    if s == "OK" {
                        eprintln!("Güncelleme betiği başarıyla tamamlandı. Log: {:?}", log_file);
                        return Ok(());
                    } else {
                        eprintln!("Güncelleme betiği hata bildirdi: {}. Log'a bak: {:?}", s, log_file);
                        return Err(format!("update.sh failed: {}. See log: {:?}", s, log_file));
                    }
                }
                Err(e) => {
                    eprintln!("Tamamlanma dosyası okunamadı {:?}: {}", done_file, e);
                    return Err(format!("Güncelleme durum dosyası okunamadı {:?}: {}", done_file, e));
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        waited += 1;
    }

    // Tamamlanma dosyasını beklerken zaman aşımı: systemctl durumunu kontrol et ve raporla
    eprintln!("Güncelleme işaretçisi beklenirken zaman aşımına uğradı ({}s). Servis durumu kontrol ediliyor...", timeout_secs);
    match Command::new("systemctl").arg("is-active").arg("one.web.tr.service").output().await {
        Ok(output) => {
            let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if out == "active" {
                eprintln!("Marker eksik olmasına rağmen servis aktif. Log: {:?}", log_file);
                return Ok(());
            }
        }
        Err(e) => eprintln!("systemctl sorgulanamadı: {}", e),
    }

    Err(format!("Timeout waiting for update to finish. Check update log: {:?}", log_file))
}
// Medya Servisi - Dosya yükleme ve yönetim işlemleri
use crate::modules::media::models::media::{ActiveModel, Column, Entity as Media, Model};
use sea_orm::*;
use std::path::PathBuf;

#[derive(Debug)]
pub enum MediaError {
    DatabaseError(DbErr),
    FileNotFound,
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaError::DatabaseError(e) => write!(f, "Veritabanı hatası: {}", e),
            MediaError::FileNotFound => write!(f, "File not found"),
        }
    }
}

impl std::error::Error for MediaError {}

impl From<DbErr> for MediaError {
    fn from(err: DbErr) -> Self {
        MediaError::DatabaseError(err)
    }
}

/// Medya dosyalarını sayfalandırma ve filtreleme ile listele
pub async fn list_media(
    db: &DatabaseConnection,
    page: u64,
    per_page: u64,
    media_type: Option<&str>,
    user_id: Option<i32>,
    search: Option<&str>,
    content_type: Option<&str>,
    content_id: Option<i64>,
) -> Result<(Vec<Model>, u64), MediaError> {
    let offset = (page - 1) * per_page;

    let mut select = Media::find();

    // Medya tipine göre filtrele
    if let Some(m_type) = media_type {
        select = select.filter(Column::MediaType.eq(m_type));
    }

    // Kullanıcıya göre filtrele
    if let Some(uid) = user_id {
        select = select.filter(Column::UserId.eq(uid));
    }

    // İçerik tipine göre filtrele
    if let Some(c_type) = content_type {
        select = select.filter(Column::ContentType.eq(c_type));
    }

    // İçerik id'sine göre filtrele
    if let Some(c_id) = content_id {
        select = select.filter(Column::ContentId.eq(c_id));
    }

    // Dosya adında ara
    if let Some(search_term) = search {
        if !search_term.is_empty() {
            let search_pattern = format!("%{}%", search_term.to_lowercase());
            select = select.filter(Column::FileName.like(&search_pattern));
        }
    }

    // Toplam sayıyı al
    let total = select.clone().count(db).await?;

    // Medya dosyalarını getir
    let media = select
        .order_by_desc(Column::CreatedAt)
        .offset(offset)
        .limit(per_page)
        .all(db)
        .await?;

    Ok((media, total))
}

/// ID'ye göre medya getir
pub async fn get_media_by_id(db: &DatabaseConnection, media_id: i64) -> Result<Model, MediaError> {
    Media::find_by_id(media_id)
        .one(db)
        .await?
        .ok_or(MediaError::FileNotFound)
}

/// Medya kaydı oluştur
pub async fn create_media(
    db: &DatabaseConnection,
    user_id: i32,
    file_name: String,
    media_type: String,
    mime_type: String,
    file_path: String,
    file_size: i64,
    title: Option<String>,
    description: Option<String>,
    content_type: Option<String>,
    content_id: Option<i64>,
) -> Result<Model, MediaError> {
    let now = chrono::Utc::now();

    // Yolu normalize et ve yapılandırılmış yükleme köküne göre göreli hale getir.
    // Bu, herhangi bir öneki (mutlak yol gibi) temizler; böylece DB'de hep yükleme kökü ile başlayan bir yol saklanır (örn. "media/uploads/...").
    let normalized_path = normalize_file_path(std::path::Path::new(&file_path));
    let upload_root = crate::config::get_config().media_upload_root().to_string();
    let stored_path = relative_path_from_upload_root(&normalized_path, &upload_root);

    let media = ActiveModel {
        user_id: Set(user_id),
        file_name: Set(file_name),
        media_type: Set(media_type),
        mime_type: Set(mime_type),
        file_path: Set(stored_path),
        file_size: Set(file_size),
        title: Set(title),
        description: Set(description),
        content_type: Set(content_type),
        content_id: Set(content_id),
        created_at: Set(Some(now.into())),
        updated_at: Set(Some(now.into())),
        ..Default::default()
    };

    let media = media.insert(db).await?;
    Ok(media)
}

/// Medya meta verisini güncelle ve isteğe bağlı olarak dosyayı değiştir
pub async fn update_media(
    db: &DatabaseConnection,
    media_id: i64,
    title: Option<String>,
    description: Option<String>,
    new_file_path: Option<String>,
    new_file_size: Option<i64>,
    new_file_name: Option<String>,
    new_mime_type: Option<String>,
    new_media_type: Option<String>,
) -> Result<Model, MediaError> {
    let media = Media::find_by_id(media_id)
        .one(db)
        .await?
        .ok_or(MediaError::FileNotFound)?;

    let mut active_model: ActiveModel = media.into();

    active_model.title = Set(title);
    active_model.description = Set(description);

    if let Some(path) = new_file_path {
        // Kaydetmeden önce normalize et ve yapılandırılmış yükleme köküne göre göreli hale getir
        let normalized_path = normalize_file_path(std::path::Path::new(&path));
        let upload_root = crate::config::get_config().media_upload_root().to_string();
        let stored_path = relative_path_from_upload_root(&normalized_path, &upload_root);
        active_model.file_path = Set(stored_path);
    }

    if let Some(size) = new_file_size {
        active_model.file_size = Set(size);
    }

    if let Some(name) = new_file_name {
        active_model.file_name = Set(name);
    }

    if let Some(mime) = new_mime_type {
        active_model.mime_type = Set(mime);
    }

    if let Some(media_type) = new_media_type {
        active_model.media_type = Set(media_type);
    }

    active_model.updated_at = Set(Some(chrono::Utc::now().into()));

    let media = active_model.update(db).await?;
    Ok(media)
}

/// Medya (sadece DB kaydını siler)
pub async fn delete_media(db: &DatabaseConnection, media_id: i64) -> Result<(), MediaError> {
    Media::delete_by_id(media_id).exec(db).await?;
    Ok(())
}

/// Medya dosyasını diskten ve DB'den siler
pub async fn delete_media_and_file(
    db: &DatabaseConnection,
    media_id: i64,
    upload_root: &str,
) -> Result<(), MediaError> {
    // Medya kaydını yüklemeyi dene
    let media = match Media::find_by_id(media_id).one(db).await? {
        Some(m) => m,
        None => {
            return Ok(());
        }
    };

    // Göreli dosya yolunu normalize et ve dosya sistemindeki tam yolu oluştur
    let mut rel = media.file_path.trim_start_matches('/').to_string();

    if let Some(stripped) = rel.strip_prefix("media/uploads/") {
        rel = stripped.to_string();
    } else if let Some(stripped) = rel.strip_prefix("/media/uploads/") {
        rel = stripped.to_string();
    } else if let Some(stripped) = rel.strip_prefix("uploads/") {
        rel = stripped.to_string();
    }

    let fs_path = std::path::Path::new(upload_root).join(rel);

    // Fiziksel dosyayı silmeyi dene - izin hatası dışındaki hataları yoksay
    match tokio::fs::remove_file(&fs_path).await {
        Ok(_) => {
            eprintln!("Deleted media file: {:?}", fs_path);
        }
        Err(e) => {
            // Dosya yoksa uyarı ver ama devam et
            eprintln!("Could not delete media file {:?}: {}", fs_path, e);
        }
    }

    // Son olarak DB kaydını sil
    Media::delete_by_id(media_id).exec(db).await?;
    Ok(())
}

/// Yıl/ay/gün yapısıyla yükleme yolu oluştur
pub fn generate_upload_path(upload_root: &str, filename: &str) -> PathBuf {
    let now = chrono::Local::now();
    let year = now.format("%Y").to_string();
    let month = now.format("%m").to_string();
    let day = now.format("%d").to_string();

    // Benzersiz dosya adı oluştur
    let timestamp = now.timestamp();
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let unique_filename = if extension.is_empty() {
        format!("{}_{}", timestamp, filename)
    } else {
        let name_without_ext = filename.trim_end_matches(&format!(".{}", extension));
        format!("{}_{}.{}", timestamp, name_without_ext, extension)
    };

    PathBuf::from(upload_root)
        .join(year)
        .join(month)
        .join(day)
        .join(unique_filename)
}

/// Dosya yolunu depolama ve web kullanımına uygun hale getirir:
pub fn normalize_file_path<P: AsRef<std::path::Path>>(path: P) -> String {
    let mut s = path.as_ref().to_string_lossy().to_string();

    // Windows ters eğik çizgilerini (\\) öne eğik çizgilere (/) dönüştür
    if s.contains('\\') {
        s = s.replace('\\', "/");
    }

    // Başında Windows sürücü harfi (örn. "C:") varsa kaldır (örn. "C:/yol" -> "/yol")
    if s.len() >= 2 {
        let mut chs = s.chars();
        chs.next(); // skip first char
        if let Some(':') = chs.next() {
            s = s.chars().skip(2).collect::<String>();
        }
    }

    // Baştaki eğik çizgileri kırpar; böylece yol göreli olur (başında '/' olmaz)
    s.trim_start_matches('/').to_string()
}

/// Herhangi bir (muhtemelen mutlak) yolu, yapılandırılmış yükleme köküne göre normalize edip göreli
/// bir yola dönüştürür. Eğer `upload_root` normalize edilmiş yol içinde bulunursa, döndürülen
/// değer `upload_root`'tan başlayan kısımdır; bulunmazsa normalize edilmiş yol olduğu gibi döner.
///
/// Örnekler:
/// - path = "/var/www/app/media/uploads/x.jpg", upload_root = "media/uploads" -> "media/uploads/x.jpg"
/// - path = "C:\projects\app\media\uploads\x.jpg", upload_root = "media/uploads" -> "media/uploads/x.jpg"
pub fn relative_path_from_upload_root(path: &str, upload_root: &str) -> String {
    let normalized = normalize_file_path(std::path::Path::new(path));
    let ur_norm = normalize_file_path(std::path::Path::new(upload_root));
    if let Some(idx) = normalized.find(&ur_norm) {
        normalized[idx..].trim_start_matches('/').to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_normalize_unix_path() {
        let p = PathBuf::from("media/uploads/2026/01/file.jpg");
        assert_eq!(normalize_file_path(p), "media/uploads/2026/01/file.jpg");
    }

    #[test]
    fn test_normalize_windows_backslashes() {
        let p = PathBuf::from(r"media\uploads\2026\01\file.jpg");
        assert_eq!(normalize_file_path(p), "media/uploads/2026/01/file.jpg");
    }

    #[test]
    fn test_normalize_windows_drive() {
        let p = PathBuf::from(r"C:\project\media\uploads\file.jpg");
        assert_eq!(normalize_file_path(p), "project/media/uploads/file.jpg");
    }

    #[test]
    fn test_normalize_leading_slash() {
        let p = PathBuf::from("/media/uploads/file.jpg");
        assert_eq!(normalize_file_path(p), "media/uploads/file.jpg");
    }

    #[test]
    fn test_relative_path_from_upload_root() {
        let p = "/var/www/app/media/uploads/2026/01/file.jpg";
        assert_eq!(
            relative_path_from_upload_root(p, "media/uploads"),
            "media/uploads/2026/01/file.jpg"
        );

        let p2 = r"C:\project\media\uploads\file.jpg";
        assert_eq!(
            relative_path_from_upload_root(p2, "media/uploads"),
            "media/uploads/file.jpg"
        );
    }
}

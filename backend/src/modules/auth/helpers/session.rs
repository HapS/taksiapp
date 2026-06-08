use crate::modules::auth::models::user::{ActiveModel as UserActiveModel, Entity as User};
use sea_orm::*;
use tower_sessions::Session;
use uuid::Uuid;

/// Misafir oturumunu sağla - gerekirse misafir User kaydı oluşturur
/// user_id'yi döndürür (mevcut misafir veya yeni oluşturulan)
pub async fn ensure_guest_session(
    db: &DatabaseConnection,
    session: &Session,
    client_ip: Option<String>,
) -> Result<i64, DbErr> {
    // Oturumda zaten bir user_id var mı kontrol et
    if let Ok(Some(user_id)) = session.get::<i64>("user_id").await {
        // Bu kullanıcının hala var ve geçerli olduğunu doğrula
        if let Some(user) = User::find_by_id(user_id).one(db).await? {
            // IP adresini güncelle (her ziyarette)
            if client_ip.is_some() {
                let mut user_active: UserActiveModel = user.clone().into();
                if let Some(ref ip) = client_ip {
                    if ip.contains(':') {
                        user_active.ip_v6 = Set(Some(ip.clone()));
                    } else {
                        user_active.ip = Set(Some(ip.clone()));
                    }
                }
                user_active.updated_at = Set(Some(chrono::Utc::now().into()));
                let _ = user_active.update(db).await;
            }
            return Ok(user.id);
        }
        // Eğer kullanıcı artık mevcut değilse, oturumu temizle ve yeni misafir oluştur
        let _ = session.remove::<i64>("user_id").await;
    }

    // Oturumda bir guest_session_id var mı kontrol et
    if let Ok(Some(guest_session_id)) = session.get::<String>("guest_session_id").await {
        // Oturum ID'sine göre mevcut misafir kullanıcıyı bulmayı dene
        if let Some(guest_user) = User::find()
            .filter(crate::modules::auth::models::user::Column::IsGuest.eq(true))
            .filter(
                crate::modules::auth::models::user::Column::GuestSessionId.eq(&guest_session_id),
            )
            .one(db)
            .await?
        {
            // IP adresini güncelle
            if client_ip.is_some() {
                let mut user_active: UserActiveModel = guest_user.clone().into();
                if let Some(ref ip) = client_ip {
                    if ip.contains(':') {
                        user_active.ip_v6 = Set(Some(ip.clone()));
                    } else {
                        user_active.ip = Set(Some(ip.clone()));
                    }
                }
                user_active.updated_at = Set(Some(chrono::Utc::now().into()));
                let _ = user_active.update(db).await;
            }

            // Gelecekteki istekler için oturuma user_id kaydet
            let _ = session.insert("user_id", guest_user.id).await;
            return Ok(guest_user.id);
        }
    }

    // Yeni misafir kullanıcı oluştur
    let guest_session_id = Uuid::new_v4().to_string();
    let guest_username = format!("guest_{}", &guest_session_id[..8]);
    let guest_email = format!("guest_{}@guest.local", &guest_session_id[..8]);

    let mut guest_user = UserActiveModel {
        username: Set(guest_username),
        email: Set(guest_email),
        is_guest: Set(true),
        guest_session_id: Set(Some(guest_session_id.clone())),
        password: Set(None), // Misafir kullanıcıların şifresi yoktur
        created_at: Set(Some(chrono::Utc::now().into())),
        updated_at: Set(Some(chrono::Utc::now().into())),
        ..Default::default()
    };

    // IP adresini ekle
    if let Some(ref ip) = client_ip {
        if ip.contains(':') {
            guest_user.ip_v6 = Set(Some(ip.clone()));
        } else {
            guest_user.ip = Set(Some(ip.clone()));
        }
    }

    let result = User::insert(guest_user).exec(db).await?;
    let user_id = result.last_insert_id;

    // Oturuma hem user_id hem de guest_session_id kaydet
    let _ = session.insert("user_id", user_id).await;
    let _ = session.insert("guest_session_id", guest_session_id).await;

    Ok(user_id)
}

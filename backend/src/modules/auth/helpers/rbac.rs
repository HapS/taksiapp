/// RBAC Helper Functions
/// Common functions for checking permissions and admin access
use crate::app_state::AppState;
use axum::response::{IntoResponse, Json, Response};
use tower_sessions::Session;

/// Check if user has admin panel access (HTML sayfaları için)
/// Uses RBAC: checks if user has system.admin_access permission
pub async fn has_admin_access(_state: &AppState, session: &Session) -> bool {
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        return user_data.has_admin_access;
    }
    false
}

//burası düzeltilecek user_type a göre değil permission ve role referans alınacak,
pub async fn has_b2b_access(_state: &AppState, session: &Session) -> bool {
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        return user_data.has_b2b_access;
    }
    false
}

/// API için admin access kontrolü helper fonksiyonu
/// AuthenticatedUser middleware ile kullanım için
pub async fn check_admin_access_api(state: &AppState, user_id: i64) -> Result<(), Response> {
    // User'ı veritabanından al
    use crate::modules::auth::models::User;
    use sea_orm::EntityTrait;

    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Err(Json(serde_json::json!({
                "success": false,
                "error": "Kullanıcı bulunamadı"
            }))
            .into_response());
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return Err(Json(serde_json::json!({
                "success": false,
                "error": "Veritabanı hatası"
            }))
            .into_response());
        }
    };

    // Admin access kontrolü
    match user.has_permission(&state.db, "system.admin_access").await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Json(serde_json::json!({
            "success": false,
            "error": "Admin yetkisi gerekli"
        }))
        .into_response()),
        Err(e) => {
            eprintln!("Yetki kontrolü hatası: {}", e);
            Err(Json(serde_json::json!({
                "success": false,
                "error": "Yetki kontrolü hatası"
            }))
            .into_response())
        }
    }
}

#[allow(dead_code)]
pub async fn check_b2b_user_access_api(state: &AppState, user_id: i64) -> Result<(), Response> {
    // User'ı veritabanından al
    use crate::modules::auth::models::User;
    use sea_orm::EntityTrait;

    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Err(Json(serde_json::json!({
                "success": false,
                "error": "Kullanıcı bulunamadı"
            }))
            .into_response());
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return Err(Json(serde_json::json!({
                "success": false,
                "error": "Veritabanı hatası"
            }))
            .into_response());
        }
    };

    match user.has_permission(&state.db, "system.b2b_access").await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Json(serde_json::json!({
            "success": false,
            "error": "B2B yetkisi gerekli"
        }))
        .into_response()),
        Err(e) => {
            eprintln!("Yetki kontrolü hatası: {}", e);
            Err(Json(serde_json::json!({
                "success": false,
                "error": "B2b Yetki kontrolü hatası"
            }))
            .into_response())
        }
    }

    // b2b access kontrolü
    // if user.user_type.as_deref() == Some("B2B") {
    //     Ok(())
    // } else {
    //     Err(Json(serde_json::json!({
    //         "success": false,
    //         "error": "B2B Kullanıcı yetkisi gerekli"
    //     }))
    //     .into_response())
    // }
}

// ============ İHTİYAÇ DURUMUNDA EKLENEBİLECEK FONKSİYONLAR ============
// Bu fonksiyonlar şu anda kullanılmıyor ama gelecekte ihtiyaç olabilir

/// Check if user has a specific permission (Session tabanlı - hızlı)
/// İhtiyaç durumunda kullanılabilir
#[allow(dead_code)]
pub async fn has_permission(_state: &AppState, session: &Session, permission: &str) -> bool {
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        return user_data.permissions.contains(&permission.to_string());
    }
    false
}

/// Check if user has a specific permission (Database tabanlı - güvenli)
/// İhtiyaç durumunda kullanılabilir
#[allow(dead_code)]
pub async fn has_permission_secure(state: &AppState, session: &Session, permission: &str) -> bool {
    // Session'dan user_id al
    if let Ok(Some(user_id)) = session.get::<i64>("user_id").await {
        // User'ı database'den al
        use crate::modules::auth::models::User;
        use sea_orm::EntityTrait;

        if let Ok(Some(user)) = User::find_by_id(user_id).one(&state.db).await {
            // Database'den permission kontrolü yap
            return user
                .has_permission(&state.db, permission)
                .await
                .unwrap_or(false);
        }
    }
    false
}

/// Check if user has any of the given permissions
/// İhtiyaç durumunda kullanılabilir
#[allow(dead_code)]
pub async fn has_any_permission(
    _state: &AppState,
    session: &Session,
    permissions: &[&str],
) -> bool {
    if let Ok(Some(user_data)) = session
        .get::<crate::modules::auth::models::SessionData>("user_data")
        .await
    {
        return permissions
            .iter()
            .any(|p| user_data.permissions.contains(&p.to_string()));
    }
    false
}

/// API için belirli permission kontrolü
/// İhtiyaç durumunda kullanılabilir
#[allow(dead_code)]
pub async fn check_permission_api(
    state: &AppState,
    user_id: i64,
    permission: &str,
) -> Result<(), Response> {
    // User'ı veritabanından al
    use crate::modules::auth::models::User;
    use sea_orm::EntityTrait;

    let user = match User::find_by_id(user_id).one(&state.db).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Err(Json(serde_json::json!({
                "success": false,
                "error": "Kullanıcı bulunamadı"
            }))
            .into_response());
        }
        Err(e) => {
            eprintln!("Veritabanı hatası: {}", e);
            return Err(Json(serde_json::json!({
                "success": false,
                "error": "Veritabanı hatası"
            }))
            .into_response());
        }
    };

    // Permission kontrolü
    match user.has_permission(&state.db, permission).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Json(serde_json::json!({
            "success": false,
            "error": format!("Bu işlem için '{}' yetkisi gerekli", permission)
        }))
        .into_response()),
        Err(e) => {
            eprintln!("Yetki kontrolü hatası: {}", e);
            Err(Json(serde_json::json!({
                "success": false,
                "error": "Yetki kontrolü hatası"
            }))
            .into_response())
        }
    }
}

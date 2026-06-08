// Auth Web Controllers - HTML sayfaları ve form işlemleri
use crate::app_state::AppState;
use crate::middleware::global_context::ViewContext;
use crate::modules::auth::services::auth_service;
use crate::modules::utils::ip_helper::get_client_ip_from_headers;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use rust_i18n::t;
use sea_orm::EntityTrait;
use serde::Deserialize;
use tower_sessions::Session;

// Form Data Models
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorQuery {
    pub error: Option<String>,
    pub success: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordForm {
    pub email_or_username: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordForm {
    pub password: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordQuery {
    pub token: String,
}

// ============ HTML PAGES ============

/// Login sayfası
pub async fn login_page(
    State(state): State<AppState>,
    mut ctx: ViewContext,
    session: Session,
    Query(params): Query<ErrorQuery>,
) -> Response {
    // Sadece gerçek kullanıcılar için redirect yap, guest kullanıcılar için değil
    if let Ok(Some(user_id)) = session.get::<i64>("user_id").await {
        // Kullanıcının guest olup olmadığını kontrol et
        if let Ok(Some(user)) = crate::modules::auth::models::user::Entity::find_by_id(user_id)
            .one(&state.db)
            .await
        {
            if !user.is_guest {
                // Gerçek kullanıcı - ana sayfaya yönlendir
                return Redirect::to("/").into_response();
            }
        }
    }

    let lang = ctx
        .0
        .get("current_language")
        .and_then(|v| v.as_str())
        .unwrap_or("tr");
    ctx.0.insert("title", &t!("login", locale = lang));
    ctx.0.insert("request_path", "/login");

    // Hata veya başarı mesajlarını ekle
    if let Some(error) = params.error {
        ctx.0.insert("error", &error);
        // Eğer hata varsa input'ları is-invalid yapmak için field_errors ekleyelim (register'daki gibi)
        let field_errors = serde_json::json!({
            "username": "invalid",
            "password": "invalid"
        });
        ctx.0.insert("field_errors", &field_errors);
    }
    if let Some(success) = params.success {
        ctx.0.insert("success", &success);
    }

    // Flash data'dan form verilerini al (username'i tekrar doldurmak için)
    if let Ok(Some(old_input)) = session.get::<serde_json::Value>("login_old_input").await {
        ctx.0.insert("old_input", &old_input);
        let _ = session.remove::<serde_json::Value>("login_old_input").await;
    }

    match state.render_frontend_template("auth/login.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

/// Register sayfası
pub async fn register_page(
    State(state): State<AppState>,
    mut ctx: ViewContext,
    session: Session,
    Query(params): Query<ErrorQuery>,
) -> Response {
    // Sadece gerçek kullanıcılar için redirect yap, guest kullanıcılar için değil
    if let Ok(Some(user_id)) = session.get::<i64>("user_id").await {
        // Kullanıcının guest olup olmadığını kontrol et
        if let Ok(Some(user)) = crate::modules::auth::models::user::Entity::find_by_id(user_id)
            .one(&state.db)
            .await
        {
            if !user.is_guest {
                // Gerçek kullanıcı - ana sayfaya yönlendir
                return Redirect::to("/").into_response();
            }
        }
    }

    let lang = ctx
        .0
        .get("current_language")
        .and_then(|v| v.as_str())
        .unwrap_or("tr");
    ctx.0.insert("title", &t!("register", locale = lang));
    ctx.0.insert("request_path", "/register");

    // Flash data'dan form verilerini al
    if let Ok(Some(old_input)) = session.get::<serde_json::Value>("register_old_input").await {
        ctx.0.insert("old_input", &old_input);
        let _ = session
            .remove::<serde_json::Value>("register_old_input")
            .await;
    }

    // Flash data'dan field errors'ları al
    if let Ok(Some(field_errors)) = session
        .get::<serde_json::Value>("register_field_errors")
        .await
    {
        ctx.0.insert("field_errors", &field_errors);
        let _ = session
            .remove::<serde_json::Value>("register_field_errors")
            .await;
    }

    // Hata mesajlarını ekle
    if let Some(error) = params.error {
        ctx.0.insert("error", &error);
    }
    if let Some(success) = params.success {
        ctx.0.insert("success", &success);
    }

    match state.render_frontend_template("auth/register.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                state.config.is_debug(),
                false,
                Some(&state),
            );
        }
    }
}

// ============ FORM HANDLERS ============

/// Login form işleme
pub async fn login_form(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    // Misafir kullanıcı ID'si varsa al
    let guest_user_id = session.get::<i64>("user_id").await.ok().flatten();

    match auth_service::login(&state.db, &form.username, &form.password, guest_user_id).await {
        Ok(session_data) => {
            // IP adresini güncelle
            let client_ip = get_client_ip_from_headers(&headers);
            if let Err(e) =
                auth_service::update_user_ip(&state.db, session_data.user_id, client_ip).await
            {
                eprintln!("IP update error: {}", e);
            }

            // Yeni session oluştur (güvenlik için)
            if let Err(e) = session.cycle_id().await {
                eprintln!("Session cycle error: {}", e);
            }

            // Session'a kaydet
            if let Err(e) = session.insert("user_id", session_data.user_id).await {
                eprintln!("Session error: {}", e);
                return Redirect::to("/login?error=session").into_response();
            }
            if let Err(e) = session.insert("user_data", session_data).await {
                eprintln!("Session error: {}", e);
                return Redirect::to("/login?error=session").into_response();
            }

            Redirect::to("/").into_response()
        }
        Err(_) => {
            // Form verilerini session'a kaydet (username'i tekrar doldurmak için)
            let old_input = serde_json::json!({
                "username": form.username,
            });
            let _ = session.insert("login_old_input", old_input).await;
            Redirect::to("/login?error=invalid").into_response()
        }
    }
}

/// Register form işleme
pub async fn register_form(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<RegisterForm>,
) -> Response {
    let first_name = form.first_name.clone().unwrap_or_default();
    let last_name = form.last_name.clone().unwrap_or_default();

    //guest olan kullanıcıyı gerçek kullanıcıya dönüştürme
    if let Ok(Some(user_id)) = session.get::<i64>("user_id").await {
        // Kullanıcının guest olup olmadığını kontrol et
        if let Ok(Some(user)) = crate::modules::auth::models::user::Entity::find_by_id(user_id)
            .one(&state.db)
            .await
        {
            if user.is_guest {
                // Gerçek kullanıcı - ana sayfaya yönlendir
                println!("GUEST USER I GERÇEK USER A DÖNÜŞTÜR BUNUN İÇİN HARİKA FİR FONKSYON YAZDIM : ADI register_guest_user");

                match auth_service::register_guest_user(
                    &state.db,
                    user.id,
                    &form.username,
                    &form.email,
                    &form.password,
                    Some(first_name.clone()),
                    Some(last_name.clone()),
                )
                .await
                {
                    Ok(user) => {
                        // IP adresini güncelle
                        let client_ip = get_client_ip_from_headers(&headers);
                        if let Err(e) =
                            auth_service::update_user_ip(&state.db, user.id, client_ip).await
                        {
                            eprintln!("IP update error: {}", e);
                        }

                        Redirect::to("/login?success=registered").into_response()
                    }
                    Err(e) => {
                        // Form verilerini session'a kaydet (şifre hariç, güvenlik için)
                        let old_input = serde_json::json!({
                            "username": form.username,
                            "email": form.email,
                            "first_name": form.first_name,
                            "last_name": form.last_name,
                        });
                        let _ = session.insert("register_old_input", old_input).await;

                        // Field-specific hataları belirle
                        let (field, error_key) = match e {
                            auth_service::AuthError::EmailAlreadyExists => {
                                ("email", "email_exists")
                            }
                            auth_service::AuthError::UserAlreadyExists => {
                                ("username", "username_exists")
                            }
                            auth_service::AuthError::EmailFormatInvalid => {
                                ("email", "email_invalid")
                            }
                            auth_service::AuthError::WeakPassword => ("password", "weak_password"),
                            _ => ("", "failed"),
                        };

                        // Field errors'ı session'a kaydet
                        if !field.is_empty() {
                            let field_errors = serde_json::json!({
                                field: error_key
                            });
                            let _ = session.insert("register_field_errors", field_errors).await;
                        }

                        Redirect::to("/register").into_response()
                    }
                };
            }
        }
    }
    //guest olan kullanıcıyı gerçek kullanıcıya dönüştürme son

    match auth_service::register(
        &state.db,
        &form.username,
        &form.email,
        &form.password,
        Some(first_name),
        Some(last_name),
    )
    .await
    {
        Ok(user) => {
            // IP adresini güncelle
            let client_ip = get_client_ip_from_headers(&headers);
            if let Err(e) = auth_service::update_user_ip(&state.db, user.id, client_ip).await {
                eprintln!("IP update error: {}", e);
            }

            Redirect::to("/login?success=registered").into_response()
        }
        Err(e) => {
            // Form verilerini session'a kaydet (şifre hariç, güvenlik için)
            let old_input = serde_json::json!({
                "username": form.username,
                "email": form.email,
                "first_name": form.first_name,
                "last_name": form.last_name,
            });
            let _ = session.insert("register_old_input", old_input).await;

            // Field-specific hataları belirle
            let (field, error_key) = match e {
                auth_service::AuthError::EmailAlreadyExists => ("email", "email_exists"),
                auth_service::AuthError::UserAlreadyExists => ("username", "username_exists"),
                auth_service::AuthError::EmailFormatInvalid => ("email", "email_invalid"),
                auth_service::AuthError::WeakPassword => ("password", "weak_password"),
                _ => ("", "failed"),
            };

            // Field errors'ı session'a kaydet
            if !field.is_empty() {
                let field_errors = serde_json::json!({
                    field: error_key
                });
                let _ = session.insert("register_field_errors", field_errors).await;
            }

            Redirect::to("/register").into_response()
        }
    }
}
/// Logout işlemi
pub async fn logout(session: Session) -> Response {
    let _ = session.flush().await;
    let _ = session.delete().await;
    Redirect::to("/login?success=logout").into_response()
}

/// Parolamı Unuttum sayfası
pub async fn forgot_password_page(
    State(_state): State<AppState>,
    mut ctx: ViewContext,
    Query(params): Query<ErrorQuery>,
) -> Response {
    let lang = ctx
        .0
        .get("current_language")
        .and_then(|v| v.as_str())
        .unwrap_or("tr");
    ctx.0.insert("title", &t!("forgot_password", locale = lang));
    ctx.0.insert("request_path", "/forgot-password");

    if let Some(error) = params.error {
        ctx.0.insert("error", &error);
    }
    if let Some(success) = params.success {
        ctx.0.insert("success", &success);
    }

    match _state.render_frontend_template("auth/forgot_password.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode (with snippet if possible)
            return crate::middleware::error_handler::handle_template_error_with_context(
                &e,
                _state.config.is_debug(),
                false,
                Some(&_state),
            );
        }
    }
}

/// Parolamı Unuttum form işleme
pub async fn forgot_password_form(
    State(state): State<AppState>,
    crate::middleware::global_context::CurrentLanguage(lang): crate::middleware::global_context::CurrentLanguage,
    Form(form): Form<ForgotPasswordForm>,
) -> Response {
    match auth_service::request_password_reset(&state.db, &form.email_or_username).await {
        Ok((user, token)) => {
            // E-posta gönder
            let base_url = state.config.get_base_url();
            let reset_url = format!("{}/reset-password?token={}", base_url, token);

            if let Err(e) =
                crate::modules::mailer::services::mailer_template_service::MailHelper::send_password_reset(
                    &state,
                    &user.email,
                    &user.username,
                    &reset_url,
                    &lang,
                )
                .await
            {
                eprintln!("Mail sending error: {}", e);
                return Redirect::to("/forgot-password?error=mail_failed").into_response();
            }

            Redirect::to("/forgot-password?success=sent").into_response()
        }
        Err(_) => {
            // Güvenlik için kullanıcı bulunmasa bile başarılıymış gibi davranabiliriz veya hata verebiliriz.
            // Kullanıcı istediği için hata veriyoruz.
            Redirect::to("/forgot-password?error=user_not_found").into_response()
        }
    }
}

/// Parola Sıfırlama sayfası
pub async fn reset_password_page(
    State(state): State<AppState>,
    mut ctx: ViewContext,
    Query(params): Query<ResetPasswordQuery>,
) -> Response {
    // Token geçerli mi kontrol et
    match auth_service::validate_reset_token(&state.db, &params.token).await {
        Ok(_) => {
            ctx.0.insert("token", &params.token);
            let lang = ctx
                .0
                .get("current_language")
                .and_then(|v| v.as_str())
                .unwrap_or("tr");
            ctx.0.insert("title", &t!("reset_password", locale = lang));

            match state.render_frontend_template("auth/reset_password.html", &ctx.0) {
                Ok(html) => Html(html).into_response(),
                Err(e) => {
                    // Show detailed Tera error page in debug mode (with snippet if possible)
                    return crate::middleware::error_handler::handle_template_error_with_context(
                        &e,
                        state.config.is_debug(),
                        false,
                        Some(&state),
                    );
                }
            }
        }
        Err(_) => Redirect::to("/forgot-password?error=invalid_token").into_response(),
    }
}

/// Parola Sıfırlama form işleme
pub async fn reset_password_form(
    State(state): State<AppState>,
    Form(form): Form<ResetPasswordForm>,
) -> Response {
    match auth_service::reset_password(&state.db, &form.token, &form.password).await {
        Ok(_) => Redirect::to("/login?success=password_reset").into_response(),
        Err(e) => {
            let error_key = match e {
                auth_service::AuthError::WeakPassword => "weak_password",
                _ => "reset_failed",
            };
            Redirect::to(&format!(
                "/reset-password?token={}&error={}",
                form.token, error_key
            ))
            .into_response()
        }
    }
}

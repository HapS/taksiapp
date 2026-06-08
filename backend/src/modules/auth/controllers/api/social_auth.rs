use crate::app_state::AppState;
use crate::modules::auth::services::social_auth_service::GoogleAuthService;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use openidconnect::Nonce;
use serde::Deserialize;
use time::Duration;
use tower_sessions::Session;

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: String,
    pub state: String,
}

pub async fn google_login() -> impl IntoResponse {
    match GoogleAuthService::get_authorization_url().await {
        Ok((auth_url, csrf_token, nonce)) => {
            let response = Redirect::to(&auth_url).into_response();
            let jar = CookieJar::new();

            let jar = jar.add(
                Cookie::build(("oauth_state", csrf_token.secret().to_string()))
                    .path("/")
                    .http_only(true)
                    .max_age(Duration::minutes(10))
                    .build(),
            );

            let jar = jar.add(
                Cookie::build(("oauth_nonce", nonce.secret().to_string()))
                    .path("/")
                    .http_only(true)
                    .max_age(Duration::minutes(10))
                    .build(),
            );

            (jar, response)
        }
        Err(e) => {
            tracing::error!("Failed to initialize Google Auth: {}", e);
            (
                CookieJar::new(),
                Redirect::to("/login?error=oauth_init_failed").into_response(),
            )
        }
    }
}

pub async fn google_callback(
    State(state): State<AppState>,
    session: Session,
    jar: CookieJar,
    Query(query): Query<GoogleCallbackQuery>,
) -> impl IntoResponse {
    // Misafir kullanıcı ID'si varsa al
    let guest_user_id = session.get::<i64>("user_id").await.ok().flatten();

    let state_cookie = jar.get("oauth_state").map(|c| c.value().to_string());
    let nonce_cookie = jar.get("oauth_nonce").map(|c| c.value().to_string());

    // OAuth cookie'lerini temizle
    let jar = jar
        .remove(Cookie::build("oauth_state").path("/").build())
        .remove(Cookie::build("oauth_nonce").path("/").build());

    if state_cookie.as_deref() != Some(&query.state) {
        return (
            jar,
            Redirect::to("/login?error=invalid_state").into_response(),
        )
            .into_response();
    }

    let nonce = match nonce_cookie {
        Some(n) => Nonce::new(n),
        None => {
            return (
                jar,
                Redirect::to("/login?error=missing_nonce").into_response(),
            )
                .into_response()
        }
    };

    match GoogleAuthService::authenticate(&state.db, query.code, nonce, guest_user_id).await {
        Ok(user) => {
            // Session verisini oluştur
            match user.to_session_data(&state.db).await {
                Ok(session_data) => {
                    // Session başlat
                    if let Err(e) = session.cycle_id().await {
                        tracing::error!("Session cycle error: {}", e);
                    }

                    if let Err(e) = session.insert("user_id", user.id).await {
                        tracing::error!("Session error user_id: {}", e);
                        return (
                            jar,
                            Redirect::to("/login?error=session_error").into_response(),
                        )
                            .into_response();
                    }

                    if let Err(e) = session.insert("user_data", session_data).await {
                        tracing::error!("Session error user_data: {}", e);
                        return (
                            jar,
                            Redirect::to("/login?error=session_error").into_response(),
                        )
                            .into_response();
                    }

                    (jar, Redirect::to("/").into_response()).into_response()
                }
                Err(e) => {
                    tracing::error!("Failed to prepare session data: {}", e);
                    (
                        jar,
                        Redirect::to("/login?error=session_data_failed").into_response(),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!("Google authentication failed: {}", e);
            (
                jar,
                Redirect::to("/login?error=auth_failed").into_response(),
            )
                .into_response()
        }
    }
}

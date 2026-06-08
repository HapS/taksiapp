use crate::config::get_config;
use crate::modules::auth::services::auth_service::{self, SocialProvider};
use anyhow::{Context, Result};
use openidconnect::core::{CoreClient, CoreIdTokenClaims, CoreProviderMetadata, CoreResponseType};
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl,
    LanguageTag, Nonce, RedirectUrl, Scope, TokenResponse,
};
use sea_orm::DatabaseConnection;

pub struct GoogleAuthService;

fn get_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Failed to build reqwest client")
}

impl GoogleAuthService {
    pub async fn get_authorization_url() -> Result<(String, CsrfToken, Nonce)> {
        let config = get_config();
        let oauth_config = config
            .oauth()
            .google
            .as_ref()
            .context("Google OAuth not configured")?;

        let http_client = get_http_client();

        let provider_metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new("https://accounts.google.com".to_string())?,
            &http_client,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to discover Google provider metadata: {}", e))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(oauth_config.client_id.clone()),
            Some(ClientSecret::new(oauth_config.client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(oauth_config.redirect_url.clone())?);

        let (auth_url, csrf_token, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();

        Ok((auth_url.to_string(), csrf_token, nonce))
    }

    pub async fn authenticate(
        db: &DatabaseConnection,
        code: String,
        nonce: Nonce,
        guest_user_id: Option<i64>,
    ) -> Result<crate::modules::auth::models::UserModel> {
        let config = get_config();
        let oauth_config = config
            .oauth()
            .google
            .as_ref()
            .context("Google OAuth not configured")?;

        let http_client = get_http_client();

        let provider_metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new("https://accounts.google.com".to_string())?,
            &http_client,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to discover Google provider metadata: {}", e))?;

        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(oauth_config.client_id.clone()),
            Some(ClientSecret::new(oauth_config.client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(oauth_config.redirect_url.clone())?);

        let token_response: openidconnect::core::CoreTokenResponse = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|e| anyhow::anyhow!("Failed to create token request: {}", e))?
            .request_async(&http_client)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to exchange authorization code: {}", e))?;

        let id_token = token_response
            .id_token()
            .context("Server did not return an ID token")?;

        let claims: CoreIdTokenClaims = id_token
            .claims(&client.id_token_verifier(), &nonce)
            .map_err(|e| anyhow::anyhow!("Failed to verify ID token claims: {}", e))?
            .clone();

        let email = claims
            .email()
            .context("ID token did not contain an email claim")?
            .to_string();

        let provider_id = claims.subject().to_string();

        let first_name = claims
            .given_name()
            .and_then(|n| n.get(None::<&LanguageTag>))
            .map(|s| s.to_string());

        let last_name = claims
            .family_name()
            .and_then(|n| n.get(None::<&LanguageTag>))
            .map(|s| s.to_string());

        let user = auth_service::find_or_create_social_user(
            db,
            SocialProvider::Google,
            &provider_id,
            &email,
            first_name,
            last_name,
            guest_user_id,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        Ok(user)
    }
}

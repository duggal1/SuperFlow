use super::MicrosoftClient;
use crate::error::IntegrationError;
use crate::oauth::{PkcePair, random_state};
use crate::types::OAuthToken;
use chrono::{Duration, Utc};
use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug)]
pub struct MicrosoftConfig {
    pub client_id: String,
    pub tenant: String,
    pub scopes: Vec<String>,
}

impl MicrosoftConfig {
    pub fn graph(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            tenant: "common".into(),
            scopes: vec![
                "offline_access".into(),
                "Mail.Read".into(),
                "Mail.Send".into(),
                "Calendars.ReadWrite".into(),
                "Files.ReadWrite".into(),
            ],
        }
    }

    pub(crate) fn validate(&self) -> Result<(), IntegrationError> {
        if self.client_id.trim().is_empty() {
            return Err(IntegrationError::Configuration(
                "Microsoft client_id is required".into(),
            ));
        }
        if self.tenant.trim().is_empty() {
            return Err(IntegrationError::Configuration(
                "Microsoft tenant is required".into(),
            ));
        }
        if self.scopes.is_empty() {
            return Err(IntegrationError::Configuration(
                "Microsoft scopes cannot be empty".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    scope: Option<String>,
    token_type: Option<String>,
}

pub async fn connect(client: &MicrosoftClient, account: &str) -> Result<(), IntegrationError> {
    let callback = super::callback::bind().await?;
    let state = random_state();
    let pkce = PkcePair::new();
    let authorize_url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
        client.config.tenant
    );
    let scopes = client.config.scopes.join(" ");
    let mut url = Url::parse(&authorize_url)?;
    url.query_pairs_mut()
        .append_pair("client_id", &client.config.client_id)
        .append_pair("redirect_uri", callback.redirect_uri())
        .append_pair("response_type", "code")
        .append_pair("response_mode", "query")
        .append_pair("scope", &scopes)
        .append_pair("state", &state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256");
    open::that(url.as_str()).map_err(|error| IntegrationError::Browser(error.to_string()))?;
    let redirect_uri = callback.redirect_uri().to_owned();
    let code = callback.wait(&state).await?;
    let token_url = format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        client.config.tenant
    );
    let response = client
        .http
        .post(token_url)
        .form(&[
            ("client_id", client.config.client_id.as_str()),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", pkce.verifier.as_str()),
            ("scope", scopes.as_str()),
        ])
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(IntegrationError::Api {
            status: status.as_u16(),
            message: body,
        });
    }
    let token: TokenResponse = serde_json::from_str(&body)?;
    let previous_refresh_token = client
        .store
        .load("microsoft", account)?
        .and_then(|stored| stored.refresh_token);
    let refresh_token = token
        .refresh_token
        .or(previous_refresh_token)
        .ok_or(IntegrationError::MissingRefreshToken)?;
    client.store.save(
        "microsoft",
        account,
        &OAuthToken {
            access_token: token.access_token,
            refresh_token: Some(refresh_token),
            token_type: token.token_type.unwrap_or_else(|| "Bearer".into()),
            scope: token.scope,
            expires_at: Utc::now() + Duration::seconds(token.expires_in.max(60)),
        },
    )?;
    Ok(())
}

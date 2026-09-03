use super::GoogleClient;
use crate::error::IntegrationError;
use crate::oauth::{PkcePair, random_state};
use crate::types::OAuthToken;
use chrono::{Duration, Utc};
use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
}

impl GoogleConfig {
    pub fn workspace(client_id: impl Into<String>, client_secret: Option<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret,
            scopes: vec![
                "https://www.googleapis.com/auth/gmail.readonly".into(),
                "https://www.googleapis.com/auth/gmail.send".into(),
                "https://www.googleapis.com/auth/calendar.calendarlist.readonly".into(),
                "https://www.googleapis.com/auth/calendar.events".into(),
                "https://www.googleapis.com/auth/drive.file".into(),
            ],
        }
    }

    pub(crate) fn validate(&self) -> Result<(), IntegrationError> {
        if self.client_id.trim().is_empty() {
            return Err(IntegrationError::Configuration(
                "Google client_id is required".into(),
            ));
        }
        if self.scopes.is_empty() {
            return Err(IntegrationError::Configuration(
                "Google scopes cannot be empty".into(),
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

pub async fn connect(client: &GoogleClient, account: &str) -> Result<(), IntegrationError> {
    let callback = super::callback::bind().await?;
    let state = random_state();
    let pkce = PkcePair::new();
    let scopes = client.config.scopes.join(" ");
    let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")?;
    url.query_pairs_mut()
        .append_pair("client_id", &client.config.client_id)
        .append_pair("redirect_uri", callback.redirect_uri())
        .append_pair("response_type", "code")
        .append_pair("scope", &scopes)
        .append_pair("state", &state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    open::that(url.as_str()).map_err(|error| IntegrationError::Browser(error.to_string()))?;
    let code = callback.wait(&state).await?;
    let redirect_uri = url
        .query_pairs()
        .find(|(key, _)| key == "redirect_uri")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| IntegrationError::OAuth("redirect uri missing".into()))?;
    let mut form = vec![
        ("client_id", client.config.client_id.as_str()),
        ("code", code.as_str()),
        ("code_verifier", pkce.verifier.as_str()),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri.as_str()),
    ];
    if let Some(secret) = client.config.client_secret.as_deref() {
        form.push(("client_secret", secret));
    }
    let response = client
        .http
        .post("https://oauth2.googleapis.com/token")
        .form(&form)
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
        .load("google", account)?
        .and_then(|stored| stored.refresh_token);
    let refresh_token = token
        .refresh_token
        .or(previous_refresh_token)
        .ok_or(IntegrationError::MissingRefreshToken)?;
    client.store.save(
        "google",
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

mod auth;
mod callback;
pub mod calendar;
pub mod docs;
pub mod drive;
pub mod gmail;

use crate::error::IntegrationError;
use crate::storage::KeychainTokenStore;
use crate::types::{ConnectionStatus, OAuthToken};
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::Deserialize;

pub use auth::GoogleConfig;

#[derive(Clone)]
pub struct GoogleClient {
    pub(crate) config: GoogleConfig,
    pub(crate) http: Client,
    pub(crate) store: KeychainTokenStore,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    expires_in: i64,
    token_type: Option<String>,
    scope: Option<String>,
    refresh_token: Option<String>,
}

impl GoogleClient {
    pub fn new(config: GoogleConfig, store: KeychainTokenStore) -> Result<Self, IntegrationError> {
        config.validate()?;
        Ok(Self {
            config,
            http: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            store,
        })
    }

    pub async fn connect(&self, account: &str) -> Result<(), IntegrationError> {
        auth::connect(self, account).await
    }

    pub fn disconnect(&self, account: &str) -> Result<(), IntegrationError> {
        self.store.delete("google", account)
    }

    pub fn status(&self, account: &str) -> Result<ConnectionStatus, IntegrationError> {
        Ok(ConnectionStatus {
            connected: self.store.load("google", account)?.is_some(),
        })
    }

    pub(crate) async fn access_token(&self, account: &str) -> Result<String, IntegrationError> {
        let token = self
            .store
            .load("google", account)?
            .ok_or(IntegrationError::NotConnected)?;
        if !token.is_expiring() {
            return Ok(token.access_token);
        }
        self.refresh(account, token).await
    }

    async fn refresh(&self, account: &str, token: OAuthToken) -> Result<String, IntegrationError> {
        let refresh_token = token
            .refresh_token
            .clone()
            .ok_or(IntegrationError::MissingRefreshToken)?;
        let mut form = vec![
            ("client_id", self.config.client_id.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ];
        if let Some(secret) = self.config.client_secret.as_deref() {
            form.push(("client_secret", secret));
        }
        let response = self
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
        let refreshed: RefreshResponse = serde_json::from_str(&body)?;
        let access_token = refreshed.access_token.clone();
        self.store.save(
            "google",
            account,
            &OAuthToken {
                access_token: refreshed.access_token,
                refresh_token: refreshed.refresh_token.or(Some(refresh_token)),
                token_type: refreshed.token_type.unwrap_or(token.token_type),
                scope: refreshed.scope.or(token.scope),
                expires_at: Utc::now() + Duration::seconds(refreshed.expires_in.max(60)),
            },
        )?;
        Ok(access_token)
    }

    pub(crate) async fn checked_json<T: serde::de::DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, IntegrationError> {
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(IntegrationError::Api {
                status: status.as_u16(),
                message: body,
            });
        }
        Ok(serde_json::from_str(&body)?)
    }
}

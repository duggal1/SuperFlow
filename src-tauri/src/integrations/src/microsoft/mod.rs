mod auth;
mod callback;
pub mod calendar;
pub mod onedrive;
pub mod outlook;

use crate::error::IntegrationError;
use crate::storage::KeychainTokenStore;
use crate::types::{ConnectionStatus, OAuthToken};
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::Deserialize;
use url::Url;

pub use auth::MicrosoftConfig;

#[derive(Clone)]
pub struct MicrosoftClient {
    pub(crate) config: MicrosoftConfig,
    pub(crate) http: Client,
    pub(crate) store: KeychainTokenStore,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
}

impl MicrosoftClient {
    pub fn new(config: MicrosoftConfig, store: KeychainTokenStore) -> Result<Self, IntegrationError> {
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
        self.store.delete("microsoft", account)
    }

    pub fn status(&self, account: &str) -> Result<ConnectionStatus, IntegrationError> {
        Ok(ConnectionStatus {
            connected: self.store.load("microsoft", account)?.is_some(),
        })
    }

    pub(crate) async fn access_token(&self, account: &str) -> Result<String, IntegrationError> {
        let token = self
            .store
            .load("microsoft", account)?
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
        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.config.tenant
        );
        let scopes = self.config.scopes.join(" ");
        let response = self
            .http
            .post(token_url)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
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
        let refreshed: RefreshResponse = serde_json::from_str(&body)?;
        let access_token = refreshed.access_token.clone();
        self.store.save(
            "microsoft",
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


    pub(crate) fn graph_next_link(&self, next_link: &str) -> Result<Url, IntegrationError> {
        let url = Url::parse(next_link)?;
        let valid = url.scheme() == "https"
            && url.host_str() == Some("graph.microsoft.com")
            && url.port().is_none()
            && url.path().starts_with("/v1.0/")
            && url.username().is_empty()
            && url.password().is_none();
        if !valid {
            return Err(IntegrationError::Configuration(
                "invalid Microsoft Graph pagination URL".into(),
            ));
        }
        Ok(url)
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

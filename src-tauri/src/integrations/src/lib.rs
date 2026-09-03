pub mod commands;
pub mod error;
pub mod google;
pub mod microsoft;
pub mod oauth;
pub mod storage;
pub mod types;

use google::{GoogleClient, GoogleConfig};
use microsoft::{MicrosoftClient, MicrosoftConfig};
use storage::KeychainTokenStore;

#[derive(Clone)]
pub struct Integrations {
    pub google: GoogleClient,
    pub microsoft: MicrosoftClient,
}

impl Integrations {
    pub fn new(
        keychain_service: impl Into<String>,
        google: GoogleConfig,
        microsoft: MicrosoftConfig,
    ) -> Result<Self, error::IntegrationError> {
        let store = KeychainTokenStore::new(keychain_service);
        Ok(Self {
            google: GoogleClient::new(google, store.clone())?,
            microsoft: MicrosoftClient::new(microsoft, store)?,
        })
    }

    /// Like [`Integrations::new`] but tolerates missing client IDs so the app
    /// can start, report status, and disconnect without OAuth credentials
    /// configured. `connect` still fails with a clear configuration error.
    pub fn new_unchecked(
        keychain_service: impl Into<String>,
        google: GoogleConfig,
        microsoft: MicrosoftConfig,
    ) -> Self {
        let store = KeychainTokenStore::new(keychain_service);
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("build integrations http client");
        Self {
            google: GoogleClient {
                config: google,
                http: http.clone(),
                store: store.clone(),
            },
            microsoft: MicrosoftClient {
                config: microsoft,
                http,
                store,
            },
        }
    }
}

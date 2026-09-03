use crate::error::IntegrationError;
use crate::types::OAuthToken;
use keyring::{Entry, Error as KeyringError};

#[derive(Clone, Debug)]
pub struct KeychainTokenStore {
    service: String,
}

impl KeychainTokenStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    pub fn load(&self, provider: &str, account: &str) -> Result<Option<OAuthToken>, IntegrationError> {
        let entry = self.entry(provider, account)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(serde_json::from_str(&value)?)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(IntegrationError::Keychain(error.to_string())),
        }
    }

    pub fn save(
        &self,
        provider: &str,
        account: &str,
        token: &OAuthToken,
    ) -> Result<(), IntegrationError> {
        let entry = self.entry(provider, account)?;
        entry
            .set_password(&serde_json::to_string(token)?)
            .map_err(|error| IntegrationError::Keychain(error.to_string()))
    }

    pub fn delete(&self, provider: &str, account: &str) -> Result<(), IntegrationError> {
        let entry = self.entry(provider, account)?;
        match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(IntegrationError::Keychain(error.to_string())),
        }
    }

    fn entry(&self, provider: &str, account: &str) -> Result<Entry, IntegrationError> {
        if account.trim().is_empty() || account.len() > 128 || account.contains('\0') {
            return Err(IntegrationError::Configuration(
                "account key must be between 1 and 128 characters".into(),
            ));
        }
        Entry::new(&self.service, &format!("oauth:{provider}:{account}"))
            .map_err(|error| IntegrationError::Keychain(error.to_string()))
    }
}

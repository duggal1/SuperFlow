use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at: DateTime<Utc>,
}

impl OAuthToken {
    pub fn is_expiring(&self) -> bool {
        self.expires_at <= Utc::now() + chrono::Duration::seconds(60)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub connected: bool,
}

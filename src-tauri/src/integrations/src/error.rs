use thiserror::Error;

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("oauth error: {0}")]
    OAuth(String),
    #[error("oauth callback timed out")]
    OAuthTimeout,
    #[error("oauth state mismatch")]
    OAuthStateMismatch,
    #[error("integration is not connected")]
    NotConnected,
    #[error("required refresh token is missing")]
    MissingRefreshToken,
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("url error: {0}")]
    Url(#[from] url::ParseError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("api error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("browser open failed: {0}")]
    Browser(String),
}

impl From<IntegrationError> for String {
    fn from(value: IntegrationError) -> Self {
        value.to_string()
    }
}

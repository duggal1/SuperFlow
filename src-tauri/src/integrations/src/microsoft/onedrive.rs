use super::MicrosoftClient;
use crate::error::IntegrationError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveItem {
    pub id: String,
    pub name: String,
    pub size: Option<u64>,
    #[serde(rename = "webUrl")]
    pub web_url: Option<String>,
    #[serde(rename = "lastModifiedDateTime")]
    pub last_modified_date_time: Option<String>,
    pub folder: Option<serde_json::Value>,
    pub file: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneDriveItemList {
    #[serde(default)]
    pub value: Vec<OneDriveItem>,
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

pub async fn list_root(
    client: &MicrosoftClient,
    account: &str,
    top: u32,
    next_link: Option<&str>,
) -> Result<OneDriveItemList, IntegrationError> {
    let token = client.access_token(account).await?;
    let request = if let Some(next_link) = next_link {
        client.http.get(client.graph_next_link(next_link)?)
    } else {
        client
            .http
            .get("https://graph.microsoft.com/v1.0/me/drive/root/children")
            .query(&[
                ("$top", top.clamp(1, 999).to_string()),
                (
                    "$select",
                    "id,name,size,webUrl,lastModifiedDateTime,folder,file".to_string(),
                ),
            ])
    };
    client.checked_json(request.bearer_auth(token)).await
}

pub async fn upload_small(
    client: &MicrosoftClient,
    account: &str,
    path: &str,
    mime_type: &str,
    bytes: Vec<u8>,
) -> Result<OneDriveItem, IntegrationError> {
    if path.trim().is_empty() || path.starts_with('/') || path.ends_with('/') {
        return Err(IntegrationError::Configuration(
            "OneDrive path must be a non-empty path relative to root".into(),
        ));
    }
    if bytes.len() > 250 * 1024 * 1024 {
        return Err(IntegrationError::Configuration(
            "OneDrive simple upload is limited to 250 MB".into(),
        ));
    }
    let token = client.access_token(account).await?;
    let encoded_path = path
        .split('/')
        .map(|segment| percent_encoding::utf8_percent_encode(segment, percent_encoding::NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/");
    client
        .checked_json(
            client
                .http
                .put(format!(
                    "https://graph.microsoft.com/v1.0/me/drive/root:/{encoded_path}:/content"
                ))
                .bearer_auth(token)
                .header(reqwest::header::CONTENT_TYPE, mime_type)
                .body(bytes),
        )
        .await
}

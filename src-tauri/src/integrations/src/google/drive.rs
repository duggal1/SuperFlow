use super::GoogleClient;
use crate::error::IntegrationError;
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(rename = "webViewLink")]
    pub web_view_link: Option<String>,
    #[serde(rename = "modifiedTime")]
    pub modified_time: Option<String>,
    pub size: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveFileList {
    #[serde(default)]
    pub files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

pub async fn list(
    client: &GoogleClient,
    account: &str,
    query: Option<&str>,
    page_size: u32,
    page_token: Option<&str>,
) -> Result<DriveFileList, IntegrationError> {
    let token = client.access_token(account).await?;
    let mut request = client
        .http
        .get("https://www.googleapis.com/drive/v3/files")
        .bearer_auth(token)
        .query(&[
            ("pageSize", page_size.clamp(1, 1000).to_string()),
            (
                "fields",
                "nextPageToken,files(id,name,mimeType,webViewLink,modifiedTime,size)".to_string(),
            ),
        ]);
    if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
        request = request.query(&[("q", query)]);
    }
    if let Some(page_token) = page_token {
        request = request.query(&[("pageToken", page_token)]);
    }
    client.checked_json(request).await
}

pub async fn upload(
    client: &GoogleClient,
    account: &str,
    name: &str,
    mime_type: &str,
    bytes: Vec<u8>,
    parent_id: Option<&str>,
) -> Result<DriveFile, IntegrationError> {
    if name.trim().is_empty() || mime_type.trim().is_empty() {
        return Err(IntegrationError::Configuration(
            "name and mime_type are required".into(),
        ));
    }
    let token = client.access_token(account).await?;
    let mut boundary_bytes = [0u8; 18];
    rand::rng().fill_bytes(&mut boundary_bytes);
    let boundary = format!("tori_{:x}", u128::from_le_bytes({
        let mut value = [0u8; 16];
        value.copy_from_slice(&boundary_bytes[..16]);
        value
    }));
    let metadata = match parent_id {
        Some(parent) => serde_json::json!({ "name": name, "parents": [parent] }),
        None => serde_json::json!({ "name": name }),
    };
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{}\r\n", metadata).as_bytes());
    body.extend_from_slice(format!("--{boundary}\r\nContent-Type: {mime_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    client
        .checked_json(
            client
                .http
                .post("https://www.googleapis.com/upload/drive/v3/files")
                .query(&[
                    ("uploadType", "multipart"),
                    ("fields", "id,name,mimeType,webViewLink,modifiedTime,size"),
                ])
                .bearer_auth(token)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    format!("multipart/related; boundary={boundary}"),
                )
                .body(body),
        )
        .await
}

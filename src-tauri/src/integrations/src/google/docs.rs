use super::GoogleClient;
use crate::error::IntegrationError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleDocument {
    #[serde(rename = "documentId")]
    pub document_id: String,
    pub title: Option<String>,
}

pub async fn create(
    client: &GoogleClient,
    account: &str,
    title: &str,
    content: &str,
) -> Result<GoogleDocument, IntegrationError> {
    if title.trim().is_empty() {
        return Err(IntegrationError::Configuration("title is required".into()));
    }
    let token = client.access_token(account).await?;
    let document: GoogleDocument = client
        .checked_json(
            client
                .http
                .post("https://docs.googleapis.com/v1/documents")
                .bearer_auth(&token)
                .json(&serde_json::json!({ "title": title })),
        )
        .await?;
    if !content.is_empty() {
        let response = client
            .http
            .post(format!(
                "https://docs.googleapis.com/v1/documents/{}:batchUpdate",
                document.document_id
            ))
            .bearer_auth(token)
            .json(&serde_json::json!({
                "requests": [{
                    "insertText": {
                        "location": { "index": 1 },
                        "text": content
                    }
                }]
            }))
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(IntegrationError::Api {
                status: status.as_u16(),
                message: response.text().await?,
            });
        }
    }
    Ok(document)
}

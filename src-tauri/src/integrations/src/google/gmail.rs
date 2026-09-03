use super::GoogleClient;
use crate::error::IntegrationError;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessageRef {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessageList {
    #[serde(default)]
    pub messages: Vec<GmailMessageRef>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate")]
    pub result_size_estimate: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessage {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
    pub snippet: Option<String>,
    #[serde(rename = "internalDate")]
    pub internal_date: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendGmailInput {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
}

pub async fn list(
    client: &GoogleClient,
    account: &str,
    query: Option<&str>,
    max_results: u32,
    page_token: Option<&str>,
) -> Result<GmailMessageList, IntegrationError> {
    let token = client.access_token(account).await?;
    let mut request = client
        .http
        .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
        .bearer_auth(token)
        .query(&[("maxResults", max_results.clamp(1, 100).to_string())]);
    if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
        request = request.query(&[("q", query)]);
    }
    if let Some(page_token) = page_token {
        request = request.query(&[("pageToken", page_token)]);
    }
    client.checked_json(request).await
}

pub async fn get(
    client: &GoogleClient,
    account: &str,
    message_id: &str,
) -> Result<GmailMessage, IntegrationError> {
    let token = client.access_token(account).await?;
    client
        .checked_json(
            client
                .http
                .get(format!(
                    "https://gmail.googleapis.com/gmail/v1/users/me/messages/{message_id}"
                ))
                .bearer_auth(token)
                .query(&[("format", "full")]),
        )
        .await
}

pub async fn send(
    client: &GoogleClient,
    account: &str,
    input: SendGmailInput,
) -> Result<GmailMessageRef, IntegrationError> {
    if input.to.is_empty() {
        return Err(IntegrationError::Configuration(
            "at least one Gmail recipient is required".into(),
        ));
    }
    validate_recipients(&input.to)?;
    validate_recipients(&input.cc)?;
    validate_recipients(&input.bcc)?;
    let token = client.access_token(account).await?;
    let mut headers = format!("To: {}\r\n", input.to.join(", "));
    if !input.cc.is_empty() {
        headers.push_str(&format!("Cc: {}\r\n", input.cc.join(", ")));
    }
    if !input.bcc.is_empty() {
        headers.push_str(&format!("Bcc: {}\r\n", input.bcc.join(", ")));
    }
    headers.push_str(&format!(
        "Subject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}",
        sanitize_header(&input.subject),
        input.body
    ));
    client
        .checked_json(
            client
                .http
                .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
                .bearer_auth(token)
                .json(&serde_json::json!({ "raw": URL_SAFE_NO_PAD.encode(headers.as_bytes()) })),
        )
        .await
}

fn sanitize_header(value: &str) -> String {
    value.replace('\r', " ").replace('\n', " ")
}

fn validate_recipients(values: &[String]) -> Result<(), IntegrationError> {
    if values.iter().any(|value| value.contains('\r') || value.contains('\n')) {
        return Err(IntegrationError::Configuration("invalid Gmail recipient".into()));
    }
    Ok(())
}

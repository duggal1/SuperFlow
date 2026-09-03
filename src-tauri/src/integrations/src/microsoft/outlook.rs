use super::MicrosoftClient;
use crate::error::IntegrationError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlookEmailAddress {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlookRecipient {
    #[serde(rename = "emailAddress")]
    pub email_address: OutlookEmailAddress,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlookMessage {
    pub id: String,
    pub subject: Option<String>,
    #[serde(rename = "bodyPreview")]
    pub body_preview: Option<String>,
    #[serde(rename = "receivedDateTime")]
    pub received_date_time: Option<String>,
    #[serde(rename = "isRead")]
    pub is_read: Option<bool>,
    pub from: Option<OutlookRecipient>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlookMessageList {
    #[serde(default)]
    pub value: Vec<OutlookMessage>,
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendOutlookInput {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub html: bool,
}

pub async fn list(
    client: &MicrosoftClient,
    account: &str,
    top: u32,
    next_link: Option<&str>,
) -> Result<OutlookMessageList, IntegrationError> {
    let token = client.access_token(account).await?;
    let request = if let Some(next_link) = next_link {
        client.http.get(client.graph_next_link(next_link)?)
    } else {
        client
            .http
            .get("https://graph.microsoft.com/v1.0/me/messages")
            .query(&[
                ("$top", top.clamp(1, 1000).to_string()),
                (
                    "$select",
                    "id,subject,bodyPreview,receivedDateTime,isRead,from".to_string(),
                ),
                ("$orderby", "receivedDateTime desc".to_string()),
            ])
    };
    client.checked_json(request.bearer_auth(token)).await
}

pub async fn send(
    client: &MicrosoftClient,
    account: &str,
    input: SendOutlookInput,
) -> Result<(), IntegrationError> {
    if input.to.is_empty() {
        return Err(IntegrationError::Configuration(
            "at least one Outlook recipient is required".into(),
        ));
    }
    let token = client.access_token(account).await?;
    let recipients = |values: Vec<String>| {
        values
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .map(|address| serde_json::json!({ "emailAddress": { "address": address } }))
            .collect::<Vec<_>>()
    };
    let response = client
        .http
        .post("https://graph.microsoft.com/v1.0/me/sendMail")
        .bearer_auth(token)
        .json(&serde_json::json!({
            "message": {
                "subject": input.subject,
                "body": {
                    "contentType": if input.html { "HTML" } else { "Text" },
                    "content": input.body
                },
                "toRecipients": recipients(input.to),
                "ccRecipients": recipients(input.cc),
                "bccRecipients": recipients(input.bcc)
            }
        }))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(IntegrationError::Api {
            status: response.status().as_u16(),
            message: response.text().await?,
        });
    }
    Ok(())
}

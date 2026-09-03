use super::MicrosoftClient;
use crate::error::IntegrationError;
use serde::{Deserialize, Serialize};


#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftCalendar {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "canEdit")]
    pub can_edit: Option<bool>,
    #[serde(rename = "isDefaultCalendar")]
    pub is_default_calendar: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftCalendarList {
    #[serde(default)]
    pub value: Vec<MicrosoftCalendar>,
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftDateTimeZone {
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
    #[serde(rename = "timeZone")]
    pub time_zone: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftEvent {
    pub id: String,
    pub subject: Option<String>,
    #[serde(rename = "bodyPreview")]
    pub body_preview: Option<String>,
    pub start: Option<MicrosoftDateTimeZone>,
    pub end: Option<MicrosoftDateTimeZone>,
    #[serde(rename = "webLink")]
    pub web_link: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrosoftEventList {
    #[serde(default)]
    pub value: Vec<MicrosoftEvent>,
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMicrosoftEventInput {
    pub calendar_id: Option<String>,
    pub subject: String,
    pub body: Option<String>,
    pub html: bool,
    pub start: String,
    pub end: String,
    pub time_zone: String,
    pub location: Option<String>,
    pub attendees: Vec<String>,
}


pub async fn list_calendars(
    client: &MicrosoftClient,
    account: &str,
    top: u32,
    next_link: Option<&str>,
) -> Result<MicrosoftCalendarList, IntegrationError> {
    let token = client.access_token(account).await?;
    let request = if let Some(next_link) = next_link {
        client.http.get(client.graph_next_link(next_link)?)
    } else {
        client
            .http
            .get("https://graph.microsoft.com/v1.0/me/calendars")
            .query(&[
                ("$top", top.clamp(1, 100).to_string()),
                ("$select", "id,name,canEdit,isDefaultCalendar".to_string()),
            ])
    };
    client.checked_json(request.bearer_auth(token)).await
}

pub async fn list_events(
    client: &MicrosoftClient,
    account: &str,
    top: u32,
    next_link: Option<&str>,
) -> Result<MicrosoftEventList, IntegrationError> {
    let token = client.access_token(account).await?;
    let request = if let Some(next_link) = next_link {
        client.http.get(client.graph_next_link(next_link)?)
    } else {
        client
            .http
            .get("https://graph.microsoft.com/v1.0/me/events")
            .query(&[
                ("$top", top.clamp(1, 1000).to_string()),
                (
                    "$select",
                    "id,subject,bodyPreview,start,end,webLink".to_string(),
                ),
            ])
    };
    client.checked_json(request.bearer_auth(token)).await
}

pub async fn create_event(
    client: &MicrosoftClient,
    account: &str,
    input: CreateMicrosoftEventInput,
) -> Result<MicrosoftEvent, IntegrationError> {
    if input.subject.trim().is_empty() || input.start.trim().is_empty() || input.end.trim().is_empty() {
        return Err(IntegrationError::Configuration(
            "subject, start, and end are required".into(),
        ));
    }
    let token = client.access_token(account).await?;
    let endpoint = match input.calendar_id.as_deref() {
        Some(calendar_id) if !calendar_id.trim().is_empty() => {
            let mut url = url::Url::parse("https://graph.microsoft.com")?;
            url.path_segments_mut()
                .map_err(|_| IntegrationError::Configuration("invalid Microsoft Graph base URL".into()))?
                .extend(["v1.0", "me", "calendars", calendar_id, "events"]);
            url.to_string()
        },
        _ => "https://graph.microsoft.com/v1.0/me/calendar/events".into(),
    };
    let time_zone = input.time_zone.clone();
    let attendees = input
        .attendees
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .map(|address| {
            serde_json::json!({
                "emailAddress": { "address": address },
                "type": "required"
            })
        })
        .collect::<Vec<_>>();
    client
        .checked_json(
            client
                .http
                .post(endpoint)
                .bearer_auth(token)
                .json(&serde_json::json!({
                    "subject": input.subject,
                    "body": {
                        "contentType": if input.html { "HTML" } else { "Text" },
                        "content": input.body.unwrap_or_default()
                    },
                    "start": { "dateTime": input.start, "timeZone": time_zone },
                    "end": { "dateTime": input.end, "timeZone": input.time_zone },
                    "location": { "displayName": input.location.unwrap_or_default() },
                    "attendees": attendees
                })),
        )
        .await
}

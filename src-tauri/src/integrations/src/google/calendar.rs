use super::GoogleClient;
use crate::error::IntegrationError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCalendar {
    pub id: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "timeZone")]
    pub time_zone: Option<String>,
    pub primary: Option<bool>,
    #[serde(rename = "accessRole")]
    pub access_role: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleCalendarList {
    #[serde(default)]
    pub items: Vec<GoogleCalendar>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEventDateTime {
    #[serde(rename = "dateTime")]
    pub date_time: Option<String>,
    pub date: Option<String>,
    #[serde(rename = "timeZone")]
    pub time_zone: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEvent {
    pub id: String,
    pub status: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: Option<GoogleEventDateTime>,
    pub end: Option<GoogleEventDateTime>,
    #[serde(rename = "htmlLink")]
    pub html_link: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleEventList {
    #[serde(default)]
    pub items: Vec<GoogleEvent>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGoogleEventInput {
    pub calendar_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start: String,
    pub end: String,
    pub time_zone: Option<String>,
    pub attendees: Vec<String>,
}

pub async fn list_calendars(
    client: &GoogleClient,
    account: &str,
) -> Result<GoogleCalendarList, IntegrationError> {
    let token = client.access_token(account).await?;
    client
        .checked_json(
            client
                .http
                .get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
                .bearer_auth(token),
        )
        .await
}

pub async fn list_events(
    client: &GoogleClient,
    account: &str,
    calendar_id: &str,
    time_min: Option<&str>,
    time_max: Option<&str>,
    max_results: u32,
) -> Result<GoogleEventList, IntegrationError> {
    let token = client.access_token(account).await?;
    let url = events_url(calendar_id)?;
    let mut request = client
        .http
        .get(url)
        .bearer_auth(token)
        .query(&[
            ("singleEvents", "true".to_string()),
            ("orderBy", "startTime".to_string()),
            ("maxResults", max_results.clamp(1, 2500).to_string()),
        ]);
    if let Some(value) = time_min {
        request = request.query(&[("timeMin", value)]);
    }
    if let Some(value) = time_max {
        request = request.query(&[("timeMax", value)]);
    }
    client.checked_json(request).await
}

pub async fn create_event(
    client: &GoogleClient,
    account: &str,
    input: CreateGoogleEventInput,
) -> Result<GoogleEvent, IntegrationError> {
    if input.summary.trim().is_empty() || input.start.trim().is_empty() || input.end.trim().is_empty() {
        return Err(IntegrationError::Configuration(
            "summary, start, and end are required".into(),
        ));
    }
    let token = client.access_token(account).await?;
    let url = events_url(&input.calendar_id)?;
    let time_zone = input.time_zone.clone();
    let attendees = input
        .attendees
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .map(|email| serde_json::json!({ "email": email }))
        .collect::<Vec<_>>();
    let body = serde_json::json!({
        "summary": input.summary,
        "description": input.description,
        "location": input.location,
        "start": { "dateTime": input.start, "timeZone": time_zone },
        "end": { "dateTime": input.end, "timeZone": input.time_zone },
        "attendees": attendees
    });
    client
        .checked_json(
            client
                .http
                .post(url)
                .bearer_auth(token)
                .json(&body),
        )
        .await
}

fn events_url(calendar_id: &str) -> Result<url::Url, IntegrationError> {
    let mut url = url::Url::parse("https://www.googleapis.com")?;
    url.path_segments_mut()
        .map_err(|_| IntegrationError::Configuration("invalid Google Calendar base URL".into()))?
        .extend(["calendar", "v3", "calendars", calendar_id, "events"]);
    Ok(url)
}

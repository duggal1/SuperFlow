use crate::settings::AiCleanupThinkingLevel;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Serialize)]
struct Part<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct Content<'a> {
    parts: [Part<'a>; 1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ThinkingConfig<'a> {
    thinking_level: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig<'a> {
    thinking_config: ThinkingConfig<'a>,
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    system_instruction: Content<'a>,
    contents: [Content<'a>; 1],
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig<'a>,
}

#[derive(Deserialize)]
struct GenerateResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize)]
struct ResponseContent {
    #[serde(default)]
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: Option<String>,
}

fn thinking_name(level: AiCleanupThinkingLevel) -> &'static str {
    match level {
        AiCleanupThinkingLevel::Minimal => "minimal",
        AiCleanupThinkingLevel::Low => "low",
        AiCleanupThinkingLevel::Medium => "medium",
        AiCleanupThinkingLevel::High => "high",
    }
}

async fn send(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    level: AiCleanupThinkingLevel,
    system_prompt: &str,
    user_content: &str,
) -> Result<(StatusCode, String), String> {
    let request = GenerateRequest {
        system_instruction: Content {
            parts: [Part {
                text: system_prompt,
            }],
        },
        contents: [Content {
            parts: [Part { text: user_content }],
        }],
        generation_config: GenerationConfig {
            thinking_config: ThinkingConfig {
                thinking_level: thinking_name(level),
            },
        },
    };

    let response = client
        .post(format!("{GEMINI_BASE_URL}/{model}:generateContent"))
        .header("x-goog-api-key", api_key)
        .json(&request)
        .send()
        .await
        .map_err(|error| format!("Gemini request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Gemini response could not be read: {error}"))?;
    Ok((status, body))
}

pub async fn generate(
    api_key: &str,
    model: &str,
    level: AiCleanupThinkingLevel,
    system_prompt: &str,
    user_content: &str,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|error| format!("Gemini client could not be created: {error}"))?;

    let mut first_transport_error = None;
    let first = send(&client, api_key, model, level, system_prompt, user_content).await;
    let (mut status, mut body) = match first {
        Ok(response) => response,
        Err(error) => {
            first_transport_error = Some(error);
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            send(&client, api_key, model, level, system_prompt, user_content).await?
        }
    };

    // A voice command is safe to retry: generation has no external side
    // effect, and insertion happens only after a complete response. Retry one
    // transient quota/server failure instead of falsely reporting the model as
    // unavailable after a single network edge failure.
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        (status, body) = send(&client, api_key, model, level, system_prompt, user_content)
            .await
            .map_err(|retry_error| {
                first_transport_error.map_or(retry_error.clone(), |first_error| {
                    format!("{first_error}; retry failed: {retry_error}")
                })
            })?;
    }

    if status == StatusCode::BAD_REQUEST
        && model == "gemini-3.5-flash-lite"
        && level == AiCleanupThinkingLevel::Minimal
    {
        (status, body) = send(
            &client,
            api_key,
            model,
            AiCleanupThinkingLevel::Low,
            system_prompt,
            user_content,
        )
        .await?;
    }

    if !status.is_success() {
        return Err(format!("Gemini returned HTTP {status}"));
    }

    let response: GenerateResponse = serde_json::from_str(&body)
        .map_err(|error| format!("Gemini returned an invalid response: {error}"))?;
    let output = response
        .candidates
        .first()
        .map(|candidate| {
            candidate
                .content
                .parts
                .iter()
                .filter_map(|part| part.text.as_deref())
                .collect::<String>()
        })
        .unwrap_or_default();

    if output.trim().is_empty() {
        return Err("Gemini returned an empty response".to_string());
    }

    Ok(output.trim().to_string())
}

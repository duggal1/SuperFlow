use serde_json::Value;

use crate::gmail_voice::context::GmailContext;
use crate::gmail_voice::grammar::{GmailIntent, GmailVoiceCommand};
use crate::settings::AppSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GmailGeneratedContent {
    Reply { body: String },
    Compose { subject: String, body: String },
}

impl GmailGeneratedContent {
    pub fn body(&self) -> &str {
        match self {
            Self::Reply { body } | Self::Compose { body, .. } => body,
        }
    }
}

pub struct GmailGenerator;

const SYSTEM_PROMPT: &str = r#"You write email messages on behalf of the user.

Use the supplied Gmail thread as factual context and the spoken instruction as the user's desired response.

Requirements:
- Follow the user's instruction exactly.
- If the instruction is only "reply" or "respond", write a direct response to the supplied email's actual content; never use a generic networking template.
- Preserve all dates, times, names, commitments, uncertainty, limitations, negations, and requested actions.
- Use the Gmail thread only to understand factual context and tone.
- Address the sender or organization from the supplied recipient fields when a greeting is appropriate.
- Never propose connecting, meeting, or discussing further unless the instruction or source email calls for it.
- Never invent facts, promises, deadlines, meetings, attachments, people, addresses, or commitments.
- Never change or infer recipient identity.
- Never answer questions the user did not ask you to answer.
- Make the email concise, natural, professional, and human.
- Match the formality of the existing conversation when possible.
- Return strict JSON only, with no markdown fences, analysis, commentary, or metadata.

For reply mode return exactly: {"body":"..."}
For compose mode return exactly: {"subject":"...","body":"..."}"#;

fn user_prompt(command: &GmailVoiceCommand, context: &GmailContext) -> Result<String, String> {
    let mut sections = Vec::new();
    match (command.intent, context) {
        (GmailIntent::Reply, GmailContext::Reply(reply)) => {
            sections.push("MODE:\nreply".to_string());
            sections.push(format!(
                "RECIPIENT:\n{} <{}>",
                reply.sender_name, reply.sender_email
            ));
            if let Some((_, domain)) = reply.sender_email.rsplit_once('@') {
                sections.push(format!("SENDER_ORGANIZATION_DOMAIN:\n{domain}"));
            }
            sections.push(format!("SUBJECT:\n{}", reply.subject));
            sections.push(format!("EMAIL_BEING_REPLIED_TO:\n{}", reply.source_message));
            if let Some(thread) = reply.thread_context.as_deref() {
                sections.push(format!("OPTIONAL_THREAD_CONTEXT:\n{thread}"));
            }
        }
        (GmailIntent::Compose, GmailContext::Compose(compose)) => {
            sections.push("MODE:\ncompose".to_string());
            if let Some(recipient) = compose.recipient_email.as_deref() {
                sections.push(format!("RECIPIENT:\n{recipient}"));
            } else if let Some(hint) = command.recipient_hint.as_deref() {
                sections.push(format!(
                    "RECIPIENT_HINT_ONLY_DO_NOT_RESOLVE_OR_INVENT:\n{hint}"
                ));
            } else {
                sections.push("RECIPIENT:\nunresolved".to_string());
            }
            if let Some(subject) = compose.subject.as_deref() {
                sections.push(format!("EXISTING_SUBJECT:\n{subject}"));
            }
        }
        _ => return Err("Gmail command and context mode do not match".to_string()),
    }
    let instruction = if command.instruction.trim().is_empty() {
        "Write an appropriate direct reply to the email above."
    } else {
        command.instruction.trim()
    };
    sections.push(format!("USER_INSTRUCTION:\n{instruction}"));
    Ok(sections.join("\n\n"))
}

impl GmailGenerator {
    pub async fn generate(
        settings: &AppSettings,
        command: &GmailVoiceCommand,
        context: &GmailContext,
    ) -> Result<GmailGeneratedContent, String> {
        let prompt = user_prompt(command, context)?;
        let response =
            crate::ai_cleanup::generate_with_system_prompt(SYSTEM_PROMPT, &prompt, settings)
                .await?;
        parse_response(command.intent, &response)
    }
}

fn parse_response(intent: GmailIntent, raw: &str) -> Result<GmailGeneratedContent, String> {
    let trimmed = raw.trim();
    let without_prefix = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    let cleaned = without_prefix
        .strip_suffix("```")
        .unwrap_or(without_prefix)
        .trim();
    let value: Value = serde_json::from_str(cleaned)
        .map_err(|error| format!("invalid Gmail generator JSON: {error}"))?;
    let body = required_text(&value, "body")?;
    match intent {
        GmailIntent::Reply => Ok(GmailGeneratedContent::Reply { body }),
        GmailIntent::Compose => Ok(GmailGeneratedContent::Compose {
            subject: required_text(&value, "subject")?,
            body,
        }),
    }
}

fn required_text(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("Gmail generator response missing {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gmail_voice::context::{ComposeContext, ReplyContext};
    use crate::gmail_voice::grammar::TerminalAction;

    fn command(intent: GmailIntent) -> GmailVoiceCommand {
        GmailVoiceCommand {
            intent,
            instruction: "Tell Alex I can join at 10 AM for 30 minutes.".to_string(),
            recipient_hint: Some("Alex".to_string()),
            terminal_action: TerminalAction::None,
        }
    }

    #[test]
    fn reply_prompt_contains_authoritative_context_and_thread() {
        let context = GmailContext::Reply(ReplyContext {
            sender_name: "Alexander Chen".to_string(),
            sender_email: "alexander@company.com".to_string(),
            subject: "Project status".to_string(),
            source_message: "Can you join the review?".to_string(),
            thread_context: Some("Earlier message".to_string()),
        });
        let prompt = user_prompt(&command(GmailIntent::Reply), &context).unwrap();
        assert!(prompt.contains("Alexander Chen <alexander@company.com>"));
        assert!(prompt.contains("SENDER_ORGANIZATION_DOMAIN:\ncompany.com"));
        assert!(prompt.contains("Can you join the review?"));
        assert!(prompt.contains("Earlier message"));
    }

    #[test]
    fn bare_reply_command_gets_a_contextual_default_instruction() {
        let context = GmailContext::Reply(ReplyContext {
            sender_name: "OpenAI".to_string(),
            sender_email: "noreply@openai.com".to_string(),
            subject: "New sign-in to your OpenAI account".to_string(),
            source_message: "If this was you, no action is needed.".to_string(),
            thread_context: None,
        });
        let mut command = command(GmailIntent::Reply);
        command.instruction.clear();
        let prompt = user_prompt(&command, &context).unwrap();
        assert!(prompt.contains("Write an appropriate direct reply to the email above."));
        assert!(prompt.contains("If this was you, no action is needed."));
    }

    #[test]
    fn compose_requires_structured_subject_and_body() {
        assert_eq!(
            parse_response(
                GmailIntent::Compose,
                r#"{"subject":"Project review","body":"Hi Alex"}"#
            )
            .unwrap(),
            GmailGeneratedContent::Compose {
                subject: "Project review".to_string(),
                body: "Hi Alex".to_string(),
            }
        );
        assert!(parse_response(GmailIntent::Compose, r#"{"body":"Hi"}"#).is_err());
    }

    #[test]
    fn mismatched_context_is_rejected() {
        let context = GmailContext::Compose(ComposeContext {
            recipient_email: None,
            subject: None,
        });
        assert!(user_prompt(&command(GmailIntent::Reply), &context).is_err());
    }
}

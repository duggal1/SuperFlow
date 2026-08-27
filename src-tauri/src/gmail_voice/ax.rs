#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::time::Duration;

use crate::gmail_voice::bridge::{GmailAgentRequest, GmailAgentResponse};
use crate::gmail_voice::context::{
    CapturedGmailContext, ComposeContext, GmailContext, ReplyContext,
};
use crate::gmail_voice::grammar::GmailIntent;
use crate::gmail_voice::session::GmailTargetIdentity;

type AXUIElementRef = *mut c_void;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type AxError = i32;

const AX_SUCCESS: AxError = 0;
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AxError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AxError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AxError;
    fn CFRelease(value: CFTypeRef);
    fn CFRetain(value: CFTypeRef);
    fn CFEqual(left: CFTypeRef, right: CFTypeRef) -> bool;
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
    fn CFGetTypeID(value: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut u8,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
}

#[derive(Clone, Copy)]
struct AxAttribute(CFStringRef);

unsafe impl Send for AxAttribute {}
unsafe impl Sync for AxAttribute {}

macro_rules! ax_attr {
    ($name:literal) => {{
        static ATTRIBUTE: std::sync::OnceLock<AxAttribute> = std::sync::OnceLock::new();
        ATTRIBUTE
            .get_or_init(|| {
                let value = objc2_foundation::NSString::from_str($name);
                AxAttribute(objc2::rc::Retained::into_raw(value) as CFStringRef)
            })
            .0
    }};
}

struct CfRef(CFTypeRef);

impl CfRef {
    fn take(value: CFTypeRef) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    fn retained(value: CFTypeRef) -> Option<Self> {
        if value.is_null() {
            return None;
        }
        unsafe { CFRetain(value) };
        Some(Self(value))
    }

    fn element(&self) -> AXUIElementRef {
        self.0 as AXUIElementRef
    }
}

impl Drop for CfRef {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

struct LiveTarget {
    identity: GmailTargetIdentity,
    body: String,
    compose_container: CfRef,
}

pub fn execute(request: GmailAgentRequest) -> GmailAgentResponse {
    match execute_inner(request) {
        Ok(response) => response,
        Err(error) => GmailAgentResponse::Rejected(error),
    }
}

fn execute_inner(request: GmailAgentRequest) -> Result<GmailAgentResponse, String> {
    match request {
        GmailAgentRequest::Capture {
            intent,
            expected_pid,
            expected_bundle_id,
        } => {
            let live = capture_live_target(expected_pid, &expected_bundle_id)?;
            let context = match intent {
                GmailIntent::Reply => GmailContext::Reply(extract_reply_context(
                    expected_pid,
                    &live.compose_container,
                )?),
                GmailIntent::Compose => GmailContext::Compose(ComposeContext {
                    recipient_email: live.identity.recipient_email.clone(),
                    subject: compose_subject(&live.compose_container),
                }),
            };
            Ok(GmailAgentResponse::Captured(CapturedGmailContext {
                identity: live.identity,
                context,
                editor_body: live.body,
            }))
        }
        GmailAgentRequest::Verify {
            identity,
            expected_body,
            expected_recipient_email,
        } => {
            let live = recapture(&identity)?;
            verify_identity(&identity, &live.identity)?;
            verify_expected(
                &live,
                expected_body.as_deref(),
                expected_recipient_email.as_deref(),
            )?;
            Ok(GmailAgentResponse::Verified(live.identity))
        }
        GmailAgentRequest::PopulateCompose {
            identity,
            recipient_email,
            subject,
        } => {
            let live = recapture(&identity)?;
            verify_identity(&identity, &live.identity)?;
            set_compose_subject(&live.compose_container, &subject)?;
            if let Some(recipient) = recipient_email.as_deref() {
                set_compose_recipient(&live.compose_container, recipient)?;
            }
            let updated = recapture_after_compose_change(&identity)?;
            verify_expected(&updated, None, recipient_email.as_deref())?;
            Ok(GmailAgentResponse::Verified(updated.identity))
        }
        GmailAgentRequest::Send {
            identity,
            expected_body,
            expected_recipient_email,
        } => {
            let live = recapture(&identity)?;
            verify_identity(&identity, &live.identity)?;
            verify_expected(&live, Some(&expected_body), Some(&expected_recipient_email))?;
            let send = find_exact_send_button(&live.compose_container).ok_or_else(|| {
                "exact Gmail Send button was not found for this editor".to_string()
            })?;
            if perform_action(send.element(), "AXPress") {
                Ok(GmailAgentResponse::Sent)
            } else {
                Err("Gmail Send button rejected AXPress".to_string())
            }
        }
        GmailAgentRequest::ClearEditor { identity } => {
            let live = recapture(&identity)?;
            verify_identity(&identity, &live.identity)?;
            let application =
                CfRef::take(unsafe { AXUIElementCreateApplication(identity.pid) } as CFTypeRef)
                    .ok_or_else(|| "Gmail Accessibility application was unavailable".to_string())?;
            let editor = copy_attribute(application.element(), ax_attr!("AXFocusedUIElement"))
                .ok_or_else(|| "Gmail editor was not focused".to_string())?;
            ensure_message_body_editor(editor.element())?;
            set_string_value(editor.element(), "")?;
            if !element_text(editor.element())
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err("Gmail editor did not accept the clear".to_string());
            }
            Ok(GmailAgentResponse::Verified(identity))
        }
    }
}

fn capture_live_target(pid: i32, expected_bundle_id: &str) -> Result<LiveTarget, String> {
    // An overlay or notification can briefly steal focus between the recording
    // stop and this agent run. The caller already gated on a verified Gmail
    // snapshot, so proceed against the expected pid instead of failing the
    // whole command on a transient focus change.
    let frontmost = crate::context::detector::frontmost_app();
    if !frontmost
        .as_ref()
        .is_some_and(|app| app.pid == pid && app.bundle_id.as_deref() == Some(expected_bundle_id))
    {
        log::warn!(
            target: "gmail_voice",
            "frontmost application changed during capture; proceeding against expected pid {pid}"
        );
    }

    // URL/title are a best-effort hint only — never a hard gate here.
    let tab = crate::context::browser::frontmost_tab(Some(expected_bundle_id), pid);
    let url = tab.as_ref().and_then(|tab| tab.url.clone());
    let title = tab.as_ref().and_then(|tab| tab.title.clone());
    if !crate::context::classify::classify(
        Some(expected_bundle_id),
        url.as_deref(),
        title.as_deref(),
    )
    .is_gmail_like()
    {
        log::debug!(
            target: "gmail_voice",
            "live surface no longer classifies as Gmail; relying on the snapshot gate"
        );
    }

    let application = CfRef::take(unsafe { AXUIElementCreateApplication(pid) } as CFTypeRef)
        .ok_or_else(|| "Gmail Accessibility application was unavailable".to_string())?;
    let window = copy_attribute(application.element(), ax_attr!("AXFocusedWindow"))
        .or_else(|| copy_attribute(application.element(), ax_attr!("AXMainWindow")))
        .ok_or_else(|| "Gmail window was unavailable".to_string())?;
    let editor = copy_attribute(application.element(), ax_attr!("AXFocusedUIElement"))
        .ok_or_else(|| "Gmail editor was not focused".to_string())?;
    ensure_message_body_editor(editor.element())?;
    let editor_path = element_path(window.element(), editor.element())
        .ok_or_else(|| "focused Gmail editor was not inside the active window".to_string())?;
    let compose_container = nearest_compose_container(editor.element(), window.element())
        .ok_or_else(|| {
            "focused editor was not inside a Gmail compose/reply container".to_string()
        })?;
    let recipient_email = compose_recipient(&compose_container);
    let resolved_url = url.unwrap_or_else(|| {
        format!(
            "gmail-native://{expected_bundle_id}/{}",
            title.clone().unwrap_or_default()
        )
    });
    let thread_key = thread_key(&resolved_url, title.as_deref());
    let window_identity = format!(
        "{}:{}",
        string_attribute(window.element(), "AXIdentifier").unwrap_or_default(),
        string_attribute(window.element(), "AXTitle").unwrap_or_default()
    );
    let editor_identity = format!(
        "{}:{}",
        editor_path
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        semantic_label(editor.element())
    );
    let body = element_text(editor.element()).unwrap_or_default();

    Ok(LiveTarget {
        identity: GmailTargetIdentity {
            bundle_id: expected_bundle_id.to_string(),
            pid,
            url: resolved_url,
            window_identity,
            thread_key,
            editor_identity,
            recipient_email,
        },
        body,
        compose_container,
    })
}

fn recapture(identity: &GmailTargetIdentity) -> Result<LiveTarget, String> {
    let mut live = capture_live_target(identity.pid, &identity.bundle_id)?;
    if live.identity.recipient_email.is_none() && identity.recipient_email.is_some() {
        let reply = extract_reply_context(identity.pid, &live.compose_container)?;
        live.identity.recipient_email = Some(reply.sender_email);
    }
    Ok(live)
}

fn recapture_after_compose_change(identity: &GmailTargetIdentity) -> Result<LiveTarget, String> {
    let live = recapture(identity)?;
    if live.identity.bundle_id != identity.bundle_id
        || live.identity.pid != identity.pid
        || live.identity.url != identity.url
        || live.identity.window_identity != identity.window_identity
        || live.identity.thread_key != identity.thread_key
        || live.identity.editor_identity != identity.editor_identity
    {
        return Err("Gmail target changed while populating compose fields".to_string());
    }
    Ok(live)
}

fn verify_identity(
    expected: &GmailTargetIdentity,
    actual: &GmailTargetIdentity,
) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err("Gmail window, thread, editor, or recipient changed".to_string())
    }
}

fn verify_expected(
    live: &LiveTarget,
    expected_body: Option<&str>,
    expected_recipient: Option<&str>,
) -> Result<(), String> {
    if expected_body.is_some_and(|expected| normalize_text(&live.body) != normalize_text(expected))
    {
        return Err("generated Gmail body was not verified in the captured editor".to_string());
    }
    if let Some(expected) = expected_recipient {
        // A literal address must round-trip exactly. A spoken name hint only
        // requires that Gmail resolved it to some concrete address — Gmail's
        // contact data is authoritative and is never second-guessed.
        let expected_is_literal =
            crate::gmail_voice::grammar::literal_recipient_email(Some(expected)).is_some();
        let matches = match live.identity.recipient_email.as_deref() {
            Some(actual) if actual == expected => true,
            Some(actual) if !expected_is_literal && !actual.is_empty() => true,
            _ => false,
        };
        if !matches {
            return Err("Gmail recipient changed or could not be verified".to_string());
        }
    }
    Ok(())
}

fn ensure_message_body_editor(element: AXUIElementRef) -> Result<(), String> {
    let role = string_attribute(element, "AXRole").unwrap_or_default();
    let label = semantic_label(element).to_lowercase();
    if matches!(role.as_str(), "AXTextArea" | "AXTextEditor")
        && (label.contains("message body") || label.contains("messagebody"))
    {
        Ok(())
    } else {
        Err("focused element is not the Gmail message body editor".to_string())
    }
}

fn nearest_compose_container(editor: AXUIElementRef, window: AXUIElementRef) -> Option<CfRef> {
    let mut current = CfRef::retained(editor as CFTypeRef)?;
    for _ in 0..18 {
        if find_exact_send_button(&current).is_some() {
            return Some(current);
        }
        if unsafe { CFEqual(current.0, window as CFTypeRef) } {
            break;
        }
        current = copy_attribute(current.element(), ax_attr!("AXParent"))?;
    }
    None
}

fn find_exact_send_button(root: &CfRef) -> Option<CfRef> {
    descendants(root.element(), 16, 4_096)
        .into_iter()
        .find(|element| {
            string_attribute(element.element(), "AXRole").as_deref() == Some("AXButton")
                && ["AXTitle", "AXDescription", "AXHelp"]
                    .into_iter()
                    .filter_map(|attribute| string_attribute(element.element(), attribute))
                    .any(|label| is_send_label(&label))
        })
}

fn is_send_label(label: &str) -> bool {
    let label = normalize_text(label).to_lowercase();
    label == "send" || label.starts_with("send (") || label.starts_with("send, shortcut")
}

fn compose_recipient(container: &CfRef) -> Option<String> {
    descendants(container.element(), 12, 2_048)
        .into_iter()
        .filter(|element| {
            let label = semantic_label(element.element()).to_lowercase();
            label == "to" || label.contains("to recipients") || label.contains("recipient")
        })
        .find_map(|element| {
            extract_single_email(&element_text(element.element()).unwrap_or_default())
        })
        .or_else(|| {
            descendants(container.element(), 12, 2_048)
                .into_iter()
                .filter_map(|element| {
                    extract_single_email(&element_text(element.element()).unwrap_or_default())
                })
                .next()
        })
}

fn compose_subject(container: &CfRef) -> Option<String> {
    find_compose_field(container, "subject")
        .and_then(|field| element_text(field.element()))
        .filter(|value| !value.trim().is_empty())
}

fn set_compose_subject(container: &CfRef, subject: &str) -> Result<(), String> {
    let field = find_compose_field(container, "subject")
        .ok_or_else(|| "Gmail Subject field was not found".to_string())?;
    set_string_value(field.element(), subject)?;
    if element_text(field.element()).as_deref().map(str::trim) != Some(subject.trim()) {
        return Err("Gmail Subject field did not retain the generated subject".to_string());
    }
    Ok(())
}

const RECIPIENT_RESOLVE_ATTEMPTS: usize = 8;
const RECIPIENT_RESOLVE_DELAY_MS: u64 = 60;

fn set_compose_recipient(container: &CfRef, recipient: &str) -> Result<(), String> {
    if compose_recipient(container).as_deref() == Some(recipient) {
        return Ok(());
    }
    let field = find_compose_field(container, "to")
        .ok_or_else(|| "Gmail To field was not found".to_string())?;
    set_string_value(field.element(), recipient)?;
    // Gmail turns a written value into a recipient chip asynchronously, so
    // poll briefly for the read-back. A literal address must come back
    // exactly; a spoken name hint ("Alex") is accepted once Gmail resolves it
    // to any single concrete address. Nothing is invented here either way.
    let expects_literal =
        crate::gmail_voice::grammar::literal_recipient_email(Some(recipient)).is_some();
    for _ in 0..RECIPIENT_RESOLVE_ATTEMPTS {
        if let Some(resolved) = compose_recipient(container) {
            if resolved == recipient || !expects_literal {
                return Ok(());
            }
            return Err("Gmail did not resolve the literal recipient address".to_string());
        }
        std::thread::sleep(Duration::from_millis(RECIPIENT_RESOLVE_DELAY_MS));
    }
    Err(if expects_literal {
        "Gmail did not resolve the literal recipient address".to_string()
    } else {
        "Gmail did not resolve the recipient hint to an address".to_string()
    })
}

fn find_compose_field(container: &CfRef, field: &str) -> Option<CfRef> {
    descendants(container.element(), 12, 2_048)
        .into_iter()
        .find(|element| {
            let role = string_attribute(element.element(), "AXRole").unwrap_or_default();
            if !matches!(role.as_str(), "AXTextField" | "AXComboBox") {
                return false;
            }
            let label = semantic_label(element.element()).to_lowercase();
            match field {
                "subject" => label == "subject" || label.contains("subject"),
                "to" => label == "to" || label.contains("to recipients"),
                _ => false,
            }
        })
}

fn extract_reply_context(pid: i32, compose_container: &CfRef) -> Result<ReplyContext, String> {
    let application = CfRef::take(unsafe { AXUIElementCreateApplication(pid) } as CFTypeRef)
        .ok_or_else(|| "Gmail Accessibility application was unavailable".to_string())?;
    let window = copy_attribute(application.element(), ax_attr!("AXFocusedWindow"))
        .or_else(|| copy_attribute(application.element(), ax_attr!("AXMainWindow")))
        .ok_or_else(|| "Gmail window was unavailable".to_string())?;
    let nodes = semantic_nodes(window.element(), 20, 8_192);
    let compose_index = nodes
        .iter()
        .position(|node| unsafe { CFEqual(node.element.0, compose_container.0) })
        .ok_or_else(|| "Gmail compose container was not inside the active window".to_string())?;
    let thread_nodes = &nodes[..compose_index];
    let subject = unique_subject(thread_nodes, window.element())
        .ok_or_else(|| "Gmail subject was not available".to_string())?;
    let (sender_index, sender_name, sender_email) = latest_structured_sender(thread_nodes)?;
    let source_message = source_message_after_sender(thread_nodes, sender_index)?;
    let thread_context = extract_thread_context(thread_nodes, &source_message);
    Ok(ReplyContext {
        sender_name,
        sender_email,
        subject,
        source_message,
        thread_context,
    })
}

struct SemanticNode {
    element: CfRef,
    role: String,
    label: String,
    text: String,
}

fn semantic_nodes(root: AXUIElementRef, max_depth: usize, max_nodes: usize) -> Vec<SemanticNode> {
    descendants(root, max_depth, max_nodes)
        .into_iter()
        .map(|element| SemanticNode {
            role: string_attribute(element.element(), "AXRole").unwrap_or_default(),
            label: semantic_label(element.element()),
            text: element_text(element.element()).unwrap_or_default(),
            element,
        })
        .collect()
}

fn unique_subject(nodes: &[SemanticNode], window: AXUIElementRef) -> Option<String> {
    let subjects = nodes
        .iter()
        .filter(|node| node.role == "AXHeading")
        .map(|node| {
            normalize_text(if node.text.is_empty() {
                &node.label
            } else {
                &node.text
            })
        })
        .filter(|text| {
            !text.is_empty()
                && !matches!(text.to_lowercase().as_str(), "gmail" | "inbox" | "search")
        })
        .collect::<Vec<_>>();
    dedupe_subjects(subjects).or_else(|| window_title_subject(window))
}

/// Normalized form used to merge duplicate subject headings ("Re: X" and "X").
fn subject_canonical(value: &str) -> &str {
    let mut current = value.trim();
    loop {
        let lowered = current.to_lowercase();
        let stripped = if lowered.starts_with("fwd:") {
            current.get(4..)
        } else if lowered.starts_with("re:") || lowered.starts_with("fw:") {
            current.get(3..)
        } else {
            return current.trim();
        };
        let Some(stripped) = stripped else {
            return current.trim();
        };
        current = stripped.trim();
    }
}

/// Dedupe subject candidates case- and Re:-insensitively. With several
/// distinct candidates, prefer the thread subject (Re:/Fwd:), else the
/// longest — the shortest heading is usually page chrome.
fn dedupe_subjects(subjects: Vec<String>) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for subject in subjects {
        if seen.insert(subject_canonical(&subject).to_lowercase()) {
            unique.push(subject);
        }
    }
    match unique.len() {
        0 => None,
        1 => unique.into_iter().next(),
        _ => unique.into_iter().max_by_key(|subject| {
            let canonical = subject_canonical(subject).to_lowercase();
            (
                canonical.starts_with("re:")
                    || canonical.starts_with("fw:")
                    || canonical.starts_with("fwd:"),
                subject.chars().count(),
            )
        }),
    }
}

/// Last-resort subject source: the Gmail window/tab title
/// ("(3) Re: Project status - Gmail").
fn window_title_subject(window: AXUIElementRef) -> Option<String> {
    window_title_subject_from(&string_attribute(window, "AXTitle").unwrap_or_default())
}

fn window_title_subject_from(title: &str) -> Option<String> {
    let cleaned = title.trim();
    if cleaned.is_empty() {
        return None;
    }
    let without_suffix = cleaned
        .strip_suffix(" – Gmail")
        .or_else(|| cleaned.strip_suffix(" - Gmail"))
        .unwrap_or(cleaned)
        .trim();
    // Leading unread badge: "(3) Re: Project status".
    let without_badge = match without_suffix.strip_prefix('(') {
        Some(rest) => rest
            .find(')')
            .map(|end| rest[end + 1..].trim_start())
            .unwrap_or(without_suffix),
        None => without_suffix,
    };
    // Trailing unread badge: "Inbox (12)".
    let without_badge = match without_badge.rfind('(') {
        Some(start) if without_badge.ends_with(')') => {
            let digits = &without_badge[start + 1..without_badge.len() - 1];
            if !digits.is_empty() && digits.chars().all(|c: char| c.is_ascii_digit()) {
                without_badge[..start].trim()
            } else {
                without_badge
            }
        }
        _ => without_badge,
    };
    let subject = without_badge.trim();
    if subject.is_empty()
        || matches!(
            subject.to_lowercase().as_str(),
            "gmail" | "inbox" | "search" | "starred" | "snoozed" | "sent" | "drafts" | "spam"
                | "trash" | "important"
        )
    {
        None
    } else {
        Some(subject.to_string())
    }
}

fn latest_structured_sender(nodes: &[SemanticNode]) -> Result<(usize, String, String), String> {
    nodes
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, node)| {
            let evidence = format!("{} {}", node.label, node.text);
            structured_name_and_email(&evidence).map(|(name, email)| (index, name, email))
        })
        .or_else(|| {
            // Fallback: Gmail sometimes exposes only a bare email ("From:
            // alex@company.com" or a lone address). Accept nodes exposing
            // exactly one email and derive the display name from the text
            // before it — the address itself is used when no name is present.
            nodes.iter().enumerate().rev().find_map(|(index, node)| {
                let evidence = format!("{} {}", node.label, node.text);
                bare_email_sender(&evidence).map(|(name, email)| (index, name, email))
            })
        })
        .ok_or_else(|| "Gmail sender identity was not exposed semantically".to_string())
}

const SENDER_HEADER_LABELS: &[&str] = &["from", "to", "cc", "bcc", "date", "reply"];

fn bare_email_sender(evidence: &str) -> Option<(String, String)> {
    let email = extract_single_email(evidence)?;
    let email_start = evidence.find(&email)?;
    let mut name = evidence[..email_start]
        .trim()
        .trim_matches(|character: char| {
            matches!(
                character,
                '"' | '\'' | '(' | ')' | ':' | '-' | '–' | ',' | '.'
            )
        });
    // Strip a leading "from" header word ("From: Alex" / "From Alex").
    if let (Some(head), Some(tail)) = (name.get(..4), name.get(4..)) {
        if head.eq_ignore_ascii_case("from")
            && tail
                .chars()
                .next()
                .is_none_or(|character| !character.is_alphanumeric())
        {
            name = tail.trim();
        }
    }
    // A bare header label carries no name — fall back to the address itself.
    if name.is_empty() || SENDER_HEADER_LABELS.contains(&name.to_lowercase().as_str()) {
        return Some((email.clone(), email));
    }
    Some((name.to_string(), email))
}

fn source_message_after_sender(
    nodes: &[SemanticNode],
    sender_index: usize,
) -> Result<String, String> {
    let mut blocks = Vec::new();
    for node in nodes.iter().skip(sender_index + 1) {
        if structured_name_and_email(&format!("{} {}", node.label, node.text)).is_some()
            && !blocks.is_empty()
        {
            break;
        }
        if matches!(node.role.as_str(), "AXStaticText" | "AXParagraph") {
            let text = normalize_text(&node.text);
            if text.len() >= 2 && !is_gmail_chrome_text(&text) {
                blocks.push(text);
            }
        }
    }
    blocks.dedup();
    let body = blocks.join("\n");
    if body.chars().count() < 10 {
        Err("Gmail source message body was not exposed semantically".to_string())
    } else {
        Ok(body)
    }
}

fn extract_thread_context(nodes: &[SemanticNode], source_message: &str) -> Option<String> {
    let mut blocks = Vec::new();
    for node in nodes {
        if matches!(node.role.as_str(), "AXStaticText" | "AXParagraph" | "AXHeading") {
            let text = normalize_text(&node.text);
            if text.len() >= 10 && !is_gmail_chrome_text(&text) && text != *source_message {
                // Avoid duplicating the source message itself, but keep other thread messages
                if !blocks.contains(&text) {
                    blocks.push(text);
                }
            }
        }
    }
    if blocks.is_empty() {
        return None;
    }
    // Join with separator, keep most recent first, truncate to 8000 chars
    let mut thread = blocks.join("\n\n---\n\n");
    if thread.chars().count() > 8000 {
        thread = thread.chars().take(8000).collect::<String>() + "\n…truncated";
    }
    // Only return if it adds value beyond source_message
    if thread.trim() == source_message.trim() {
        None
    } else {
        Some(thread)
    }
}

fn is_gmail_chrome_text(value: &str) -> bool {
    matches!(
        value.to_lowercase().as_str(),
        "send" | "more options" | "discard draft" | "reply" | "forward" | "show details"
    )
}

fn structured_name_and_email(value: &str) -> Option<(String, String)> {
    let start = value.find('<')?;
    let end = value[start + 1..].find('>')? + start + 1;
    let email = extract_single_email(&value[start + 1..end])?;
    let name = value[..start]
        .trim()
        .trim_matches(['"', '\'', '(', ')'])
        .trim()
        .to_string();
    (!name.is_empty()).then_some((name, email))
}

fn extract_single_email(value: &str) -> Option<String> {
    let emails = value
        .split(|character: char| character.is_whitespace() || "<>()\"',;".contains(character))
        .filter_map(|token| {
            let token = token.trim_matches(|character: char| matches!(character, '.' | ':' | ','));
            let (local, domain) = token.split_once('@')?;
            (!local.is_empty() && domain.contains('.') && !domain.ends_with('.'))
                .then(|| token.to_string())
        })
        .collect::<Vec<_>>();
    (emails.len() == 1).then(|| emails[0].clone())
}

fn thread_key(url: &str, title: Option<&str>) -> String {
    url.split_once('#')
        .map(|(_, fragment)| fragment)
        .filter(|fragment| !fragment.is_empty())
        .or(title)
        .unwrap_or(url)
        .to_string()
}

fn element_path(root: AXUIElementRef, element: AXUIElementRef) -> Option<Vec<usize>> {
    let mut current = CfRef::retained(element as CFTypeRef)?;
    let mut path = Vec::new();
    for _ in 0..32 {
        if unsafe { CFEqual(current.0, root as CFTypeRef) } {
            path.reverse();
            return Some(path);
        }
        let parent = copy_attribute(current.element(), ax_attr!("AXParent"))?;
        let children = copy_attribute(parent.element(), ax_attr!("AXChildren"))?;
        let count = unsafe { CFArrayGetCount(children.0) };
        let index = (0..count).find(|index| unsafe {
            CFEqual(CFArrayGetValueAtIndex(children.0, *index), current.0)
        })?;
        path.push(index as usize);
        current = parent;
    }
    None
}

fn descendants(root: AXUIElementRef, max_depth: usize, max_nodes: usize) -> Vec<CfRef> {
    let Some(root) = CfRef::retained(root as CFTypeRef) else {
        return Vec::new();
    };
    let mut stack = vec![(root, 0usize)];
    let mut output = Vec::new();
    while let Some((element, depth)) = stack.pop() {
        if output.len() >= max_nodes {
            break;
        }
        if depth < max_depth {
            if let Some(children) = copy_attribute(element.element(), ax_attr!("AXChildren")) {
                let count = unsafe { CFArrayGetCount(children.0) };
                for index in (0..count).rev() {
                    let child = unsafe { CFArrayGetValueAtIndex(children.0, index) };
                    if let Some(child) = CfRef::retained(child) {
                        stack.push((child, depth + 1));
                    }
                }
            }
        }
        output.push(element);
    }
    output
}

fn copy_attribute(element: AXUIElementRef, attribute: CFStringRef) -> Option<CfRef> {
    let mut value = std::ptr::null();
    let status = unsafe { AXUIElementCopyAttributeValue(element, attribute, &mut value) };
    (status == AX_SUCCESS).then(|| CfRef::take(value)).flatten()
}

fn string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let attribute_ref = runtime_cf_string(attribute);
    let result = copy_attribute(element, attribute_ref).and_then(|value| cf_string(value.0));
    unsafe { CFRelease(attribute_ref) };
    result
}

fn element_text(element: AXUIElementRef) -> Option<String> {
    ["AXValue", "AXTitle", "AXDescription"]
        .into_iter()
        .filter_map(|attribute| string_attribute(element, attribute))
        .find(|value| !value.trim().is_empty())
}

fn semantic_label(element: AXUIElementRef) -> String {
    [
        "AXTitle",
        "AXDescription",
        "AXRoleDescription",
        "AXDOMIdentifier",
        "AXIdentifier",
    ]
    .into_iter()
    .filter_map(|attribute| string_attribute(element, attribute))
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

fn set_string_value(element: AXUIElementRef, value: &str) -> Result<(), String> {
    let string = runtime_cf_string(value);
    let status = unsafe { AXUIElementSetAttributeValue(element, ax_attr!("AXValue"), string) };
    unsafe { CFRelease(string) };
    (status == AX_SUCCESS)
        .then_some(())
        .ok_or_else(|| "Gmail field rejected the Accessibility value".to_string())
}

fn perform_action(element: AXUIElementRef, action: &str) -> bool {
    let action = runtime_cf_string(action);
    let success = unsafe { AXUIElementPerformAction(element, action) == AX_SUCCESS };
    unsafe { CFRelease(action) };
    success
}

fn runtime_cf_string(value: &str) -> CFStringRef {
    let value = objc2_foundation::NSString::from_str(value);
    objc2::rc::Retained::into_raw(value) as CFStringRef
}

fn cf_string(value: CFTypeRef) -> Option<String> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFStringGetTypeID() } {
        return None;
    }
    let mut buffer = [0_u8; 32_768];
    let success = unsafe {
        CFStringGetCString(
            value as CFStringRef,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if !success {
        return None;
    }
    let end = buffer.iter().position(|byte| *byte == 0)?;
    Some(String::from_utf8_lossy(&buffer[..end]).into_owned())
}

fn normalize_text(value: &str) -> String {
    value.replace("\r\n", "\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_labels_are_exact_and_never_substring_matches() {
        assert!(is_send_label("Send"));
        assert!(is_send_label("Send (⌘Enter)"));
        assert!(!is_send_label("Send & Archive"));
        assert!(!is_send_label("Resend verification"));
    }

    #[test]
    fn sender_requires_structured_name_and_single_email() {
        assert_eq!(
            structured_name_and_email("Alexander Chen <alexander@company.com>"),
            Some((
                "Alexander Chen".to_string(),
                "alexander@company.com".to_string()
            ))
        );
        assert!(structured_name_and_email("alexander@company.com").is_none());
        assert!(structured_name_and_email("<alexander@company.com>").is_none());
    }

    #[test]
    fn thread_identity_prefers_url_fragment() {
        assert_eq!(
            thread_key(
                "https://mail.google.com/mail/u/0/#inbox/FMfcgzExample",
                Some("Subject - Gmail")
            ),
            "inbox/FMfcgzExample"
        );
    }

    #[test]
    fn window_title_subject_strips_badges_and_gmail_suffix() {
        assert_eq!(
            window_title_subject_from("Re: Project status - Gmail"),
            Some("Re: Project status".to_string())
        );
        assert_eq!(
            window_title_subject_from("(3) Re: Project status - Gmail"),
            Some("Re: Project status".to_string())
        );
        assert_eq!(window_title_subject_from("Inbox (12) - Gmail"), None);
        assert_eq!(window_title_subject_from("Inbox - Gmail"), None);
        assert_eq!(window_title_subject_from(""), None);
        assert_eq!(
            window_title_subject_from("Meeting notes - Gmail"),
            Some("Meeting notes".to_string())
        );
    }

    #[test]
    fn subject_candidates_dedupe_and_prefer_thread_subject() {
        assert_eq!(
            dedupe_subjects(vec![
                "Project status".to_string(),
                "Re: Project status".to_string()
            ])
            .as_deref(),
            Some("Re: Project status")
        );
        assert_eq!(
            dedupe_subjects(vec![
                "Hello world".to_string(),
                "Hello world again and again".to_string()
            ])
            .as_deref(),
            Some("Hello world again and again")
        );
        assert_eq!(dedupe_subjects(vec![]), None);
        assert_eq!(
            dedupe_subjects(vec!["Fwd: invite".to_string(), "invite".to_string()])
                .as_deref(),
            Some("Fwd: invite")
        );
    }

    #[test]
    fn bare_email_sender_uses_context_before_the_address() {
        assert_eq!(
            bare_email_sender("From: alexander@company.com"),
            Some((
                "alexander@company.com".to_string(),
                "alexander@company.com".to_string()
            ))
        );
        assert_eq!(
            bare_email_sender("Alexander Chen alexander@company.com"),
            Some((
                "Alexander Chen".to_string(),
                "alexander@company.com".to_string()
            ))
        );
        assert_eq!(
            bare_email_sender("To me@company.com"),
            Some(("me@company.com".to_string(), "me@company.com".to_string()))
        );
        // Two addresses in one node: never treat as a sender.
        assert_eq!(bare_email_sender("alex@a.com beth@b.com"), None);
    }
}

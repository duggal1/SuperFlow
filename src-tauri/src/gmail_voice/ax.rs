#![cfg(target_os = "macos")]

use std::ffi::c_void;

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
    }
}

fn capture_live_target(pid: i32, expected_bundle_id: &str) -> Result<LiveTarget, String> {
    let app = crate::context::detector::frontmost_app()
        .ok_or_else(|| "frontmost application could not be read".to_string())?;
    if app.pid != pid || app.bundle_id.as_deref() != Some(expected_bundle_id) {
        return Err("frontmost application changed".to_string());
    }

    let tab = crate::context::browser::frontmost_tab(app.bundle_id.as_deref(), pid);
    let url = tab.as_ref().and_then(|tab| tab.url.clone());
    let title = tab.as_ref().and_then(|tab| tab.title.clone());
    if crate::context::classify::classify(
        app.bundle_id.as_deref(),
        url.as_deref(),
        title.as_deref(),
    ) != crate::context::types::Surface::Gmail
    {
        return Err("frontmost surface is not verified Gmail".to_string());
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
    if expected_recipient
        .is_some_and(|expected| live.identity.recipient_email.as_deref() != Some(expected))
    {
        return Err("Gmail recipient changed or could not be verified".to_string());
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

fn set_compose_recipient(container: &CfRef, recipient: &str) -> Result<(), String> {
    if compose_recipient(container).as_deref() == Some(recipient) {
        return Ok(());
    }
    let field = find_compose_field(container, "to")
        .ok_or_else(|| "Gmail To field was not found".to_string())?;
    set_string_value(field.element(), recipient)?;
    if compose_recipient(container).as_deref() != Some(recipient) {
        return Err("Gmail did not resolve the literal recipient address".to_string());
    }
    Ok(())
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
    let subject = unique_subject(thread_nodes)?;
    let (sender_index, sender_name, sender_email) = latest_structured_sender(thread_nodes)?;
    let source_message = source_message_after_sender(thread_nodes, sender_index)?;
    Ok(ReplyContext {
        sender_name,
        sender_email,
        subject,
        source_message,
        thread_context: None,
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

fn unique_subject(nodes: &[SemanticNode]) -> Result<String, String> {
    let mut subjects = nodes
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
    subjects.sort();
    subjects.dedup();
    match subjects.as_slice() {
        [subject] => Ok(subject.clone()),
        [] => Err("Gmail subject heading was not exposed semantically".to_string()),
        _ => Err("Gmail subject heading was ambiguous".to_string()),
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
        .ok_or_else(|| "Gmail sender identity was not exposed as Name <email>".to_string())
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
}

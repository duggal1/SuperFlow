//! Focused-element text capture for developer surfaces (macOS).
//!
//! Reads the accessibility value of the frontmost application's focused UI
//! element — the visible terminal buffer in Ghostty/Apple Terminal, or editor
//! content in VS Code. This is what lets a spoken reference like "hero dot
//! tsx" be resolved against real paths the user can see.
//!
//! Self-contained AX FFI mirroring `browser.rs`: the two capturers evolve per
//! surface and share nothing but the pattern, so neither lane's edits can
//! collide. Everything here is best-effort; any error returns `None`.

#![cfg(target_os = "macos")]

use std::ffi::c_void;

type AxError = i32;
const AX_SUCCESS: AxError = 0;

// kCFStringEncodingUTF8
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AxError;
    fn CFRelease(cf: CFTypeRef);
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut u8,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
}

type AXUIElementRef = *mut c_void;
type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;

/// Wrapper making a leaked CFString pointer usable inside a `OnceLock`
/// static (raw pointers are not `Send`/`Sync` by default).
#[derive(Clone, Copy)]
struct AxAttrName(CFStringRef);

// SAFETY: a leaked, never-mutated CFString is immutable and thread-safe.
unsafe impl Send for AxAttrName {}
unsafe impl Sync for AxAttrName {}

/// Owns a +1 CoreFoundation reference; releases on drop.
struct CfRef(CFTypeRef);

impl CfRef {
    fn take(raw: CFTypeRef) -> Option<Self> {
        (!raw.is_null()).then(|| Self(raw))
    }
}

impl Drop for CfRef {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

fn copy_attribute(element: AXUIElementRef, attribute: CFStringRef) -> Option<CfRef> {
    let mut out: CFTypeRef = std::ptr::null();
    let status = unsafe { AXUIElementCopyAttributeValue(element, attribute, &mut out) };
    if status != AX_SUCCESS || out.is_null() {
        return None;
    }
    CfRef::take(out)
}

/// Attribute-name constants. Created once from bridged `NSString`s and leaked
/// deliberately — they live for the process lifetime. The leaked immutable
/// string is inherently safe to share across threads.
macro_rules! ax_attr {
    ($name:literal) => {{
        static ATTR: std::sync::OnceLock<AxAttrName> = std::sync::OnceLock::new();
        ATTR.get_or_init(|| {
            let ns = objc2_foundation::NSString::from_str($name);
            // Leak the string: +1 reference, intentionally never released.
            AxAttrName(objc2::rc::Retained::into_raw(ns) as CFStringRef)
        })
        .0
    }};
}

fn cf_string_to_string(string: CFStringRef) -> Option<String> {
    if string.is_null() {
        return None;
    }
    // Focused terminal/editor content can exceed any fixed buffer; grow until
    // the conversion fits rather than truncating mid-character.
    let mut size = 4096;
    loop {
        let mut buffer = vec![0u8; size];
        let ok = unsafe {
            CFStringGetCString(
                string,
                buffer.as_mut_ptr(),
                buffer.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
            )
        };
        if ok {
            let end = buffer.iter().position(|&b| b == 0)?;
            return String::from_utf8_lossy(&buffer[..end]).into_owned().into();
        }
        size = size.checked_mul(2)?;
    }
}

/// Drops C0 control characters except newline and tab — terminal buffers can
/// carry escape sequences that would only pollute an LLM prompt.
fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|&c| c == '\n' || c == '\t' || !c.is_control())
        .collect()
}

/// Keeps the tail of the text (the cursor side — most recent terminal output,
/// current edit location) within `max_chars`, splitting on a char boundary.
fn keep_tail(text: &str, max_chars: usize) -> &str {
    if text.chars().count() <= max_chars {
        return text;
    }
    let skip = text.char_indices().nth(text.chars().count() - max_chars);
    match skip {
        Some((index, _)) => &text[index..],
        None => text,
    }
}

/// Visible text of `pid`'s focused UI element, capped to [`MAX_CHARS`].
pub fn focused_element_text(pid: i32) -> Option<String> {
    const MAX_CHARS: usize = 4000;

    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return None;
    }
    let _app_guard = CfRef(app as CFTypeRef);

    let focused = copy_attribute(app, ax_attr!("AXFocusedUIElement"))?;
    let value = copy_attribute(focused.0 as AXUIElementRef, ax_attr!("AXValue"))?;
    let text = cf_string_to_string(value.0 as CFStringRef)?;

    let sanitized = sanitize(&text);
    (!sanitized.trim().is_empty())
        .then(|| keep_tail(&sanitized, MAX_CHARS).to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn sanitizes_control_characters_but_keeps_newlines() {
        assert_eq!(
            super::sanitize("cargo run\u{1b}[0m\nok\tdone\u{7}"),
            "cargo run[0m\nok\tdone"
        );
    }

    #[test]
    fn tail_preserves_char_boundaries() {
        let text = "abcあいうえお";
        assert_eq!(super::keep_tail(text, 4), "いうえお");
        assert_eq!(super::keep_tail("short", 100), "short");
    }

    #[test]
    fn focused_capture_never_panics() {
        // No real app pid 0 target expected to expose text; contract is
        // Option-only outcomes, never a panic.
        let _ = super::focused_element_text(0);
    }
}

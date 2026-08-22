//! Browser tab URL/title capture via the macOS Accessibility API.
//!
//! Strategy per engine:
//! - Chromium family (Chrome/Arc/Edge/Brave): walk `AXFocusedWindow` →
//!   `AXToolbar`, find the address-bar text field (`AXTextField`) and read its
//!   value. The window title carries `<page title> - <browser>` regardless.
//! - Safari: read the `AXURL` attribute off the focused window when exposed,
//!   otherwise degrade to title-only.
//!
//! Everything here is best-effort: any AX error returns `None` fields and the
//! caller keeps a usable snapshot. Declares the handful of C APIs it needs
//! directly so no new dependency enters the tree (CoreFoundation symbols are
//! always present on macOS).

#![cfg(target_os = "macos")]

use std::ffi::c_void;

use super::classify::{is_known_browser, CHROMIUM_BUNDLE_PREFIXES, SAFARI_BUNDLE_ID};

type AxError = i32;
const AX_SUCCESS: AxError = 0;

// kCFStringEncodingUTF8
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AxError;
    fn CFRelease(cf: CFTypeRef);
    fn CFArrayGetCount(array: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, index: isize) -> CFTypeRef;
    fn CFURLCopyAbsoluteURL(url: CFURLRef) -> CFURLRef;
    fn CFURLGetString(url: CFURLRef) -> CFStringRef;
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
type CFURLRef = *const c_void;
type CFArrayRef = *const c_void;

/// Attribute-name constants. Created once from bridged `NSString`s and leaked
/// deliberately — they live for the process lifetime. The leaked immutable
/// string is inherently safe to share across threads.
#[derive(Clone, Copy)]
struct AxAttrName(CFStringRef);

// SAFETY: a leaked, never-mutated CFString is immutable and thread-safe.
unsafe impl Send for AxAttrName {}
unsafe impl Sync for AxAttrName {}

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

impl CfRef {
    fn as_element(&self) -> AXUIElementRef {
        self.0 as AXUIElementRef
    }

    fn as_array(&self) -> CFArrayRef {
        self.0 as CFArrayRef
    }

    fn as_string(&self) -> Option<String> {
        cf_string_to_string(self.0 as CFStringRef)
    }
}

fn cf_string_to_string(string: CFStringRef) -> Option<String> {
    if string.is_null() {
        return None;
    }
    let mut buffer = [0u8; 2048];
    let ok = unsafe {
        CFStringGetCString(string, buffer.as_mut_ptr(), buffer.len() as isize, K_CF_STRING_ENCODING_UTF8)
    };
    if !ok {
        return None;
    }
    let end = buffer.iter().position(|&b| b == 0)?;
    String::from_utf8_lossy(&buffer[..end]).into_owned().into()
}

fn copy_attribute(element: AXUIElementRef, attribute: CFStringRef) -> Option<CfRef> {
    let mut out: CFTypeRef = std::ptr::null();
    let status = unsafe { AXUIElementCopyAttributeValue(element, attribute, &mut out) };
    if status != AX_SUCCESS || out.is_null() {
        return None;
    }
    CfRef::take(out)
}

fn element_role(element: AXUIElementRef) -> Option<String> {
    copy_attribute(element, ax_attr!("AXRole"))?.as_string()
}

fn element_value_string(element: AXUIElementRef) -> Option<String> {
    copy_attribute(element, ax_attr!("AXValue"))?.as_string()
}

/// Breadth-first search for the first descendant whose role equals `role`.
/// Bounded in depth and node count so pathological trees can't stall capture.
fn find_descendant_by_role(
    root: AXUIElementRef,
    role: &str,
    max_depth: usize,
    max_nodes: usize,
) -> Option<AXUIElementRef> {
    let mut queue = std::collections::VecDeque::from([(root, 0usize)]);
    let mut visited = 0usize;
    while let Some((element, depth)) = queue.pop_front() {
        if depth > max_depth || visited >= max_nodes {
            return None;
        }
        visited += 1;
        if element_role(element).as_deref() == Some(role) {
            return Some(element);
        }
        if let Some(children) = copy_attribute(element, ax_attr!("AXChildren")) {
            let count = unsafe { CFArrayGetCount(children.as_array()) };
            for index in 0..count {
                let child = unsafe { CFArrayGetValueAtIndex(children.as_array(), index) };
                if !child.is_null() {
                    queue.push_back((child as AXUIElementRef, depth + 1));
                }
            }
        }
    }
    None
}

pub struct TabInfo {
    pub url: Option<String>,
    pub title: Option<String>,
}

fn window_url_safari(window: AXUIElementRef) -> Option<String> {
    let url = copy_attribute(window, ax_attr!("AXURL"))?;
    let absolute =
        CfRef::take(unsafe { CFURLCopyAbsoluteURL(url.0 as CFURLRef) })?;
    let string = CfRef::take(unsafe { CFURLGetString(absolute.0 as CFStringRef) })?;
    string.as_string()
}

fn window_url_chromium(window: AXUIElementRef) -> Option<String> {
    let toolbar = find_descendant_by_role(window, "AXToolbar", 2, 64)?;
    let field = find_descendant_by_role(toolbar, "AXTextField", 8, 256)?;
    let value = element_value_string(field)?;
    // The omnibox holds the URL while browsing; only trust URL-shaped values.
    (value.starts_with("http://") || value.starts_with("https://")).then_some(value)
}

/// Capture URL/title for the given frontmost browser. Returns `None` entirely
/// when the bundle isn't a supported browser.
pub fn frontmost_tab(bundle_id: Option<&str>, pid: i32) -> Option<TabInfo> {
    if !is_known_browser(bundle_id) {
        return None;
    }

    let system_wide = unsafe { AXUIElementCreateSystemWide() };
    if system_wide.is_null() {
        return None;
    }
    let _system_wide_guard = CfRef(system_wide as CFTypeRef);

    let focused_app = copy_attribute(system_wide, ax_attr!("AXFocusedApplication"))?;
    let pid_attr = unsafe { AXUIElementCreateApplication(pid) };
    if pid_attr.is_null() {
        return None;
    }
    let _app_guard = CfRef(pid_attr as CFTypeRef);

    // Prefer the app's own focused window; fall back to its main window.
    let window = copy_attribute(focused_app.as_element(), ax_attr!("AXFocusedWindow"))
        .or_else(|| copy_attribute(focused_app.as_element(), ax_attr!("AXMainWindow")))?
        .as_element();

    let title = copy_attribute(window, ax_attr!("AXTitle")).and_then(|t| t.as_string());

    let is_safari = bundle_id == Some(SAFARI_BUNDLE_ID);
    let is_chromium = CHROMIUM_BUNDLE_PREFIXES
        .iter()
        .any(|prefix| bundle_id.is_some_and(|b| b.starts_with(prefix)));

    let url = if is_safari {
        window_url_safari(window)
    } else if is_chromium {
        window_url_chromium(window)
    } else {
        None
    };

    // A missing URL alone shouldn't discard the title signal.
    if url.is_none() && title.is_none() {
        return None;
    }

    Some(TabInfo { url, title })
}

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

use super::classify::{
    is_known_browser, CHROMIUM_BUNDLE_PREFIXES, SAFARI_BUNDLE_ID, SAFARI_WEB_APP_BUNDLE_PREFIX,
};

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
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AxError;
    fn CFRelease(cf: CFTypeRef);
    fn CFRetain(cf: CFTypeRef);
    fn CFArrayGetCount(array: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, index: isize) -> CFTypeRef;
    fn CFURLCopyAbsoluteURL(url: CFURLRef) -> CFURLRef;
    fn CFURLGetString(url: CFURLRef) -> CFStringRef;
    static kCFBooleanTrue: CFTypeRef;
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

    /// Take ownership of a non-owning borrow (e.g. an element borrowed from a
    /// CFArray) by retaining it. The returned handle keeps the object alive
    /// independently of its original container.
    fn retained(raw: CFTypeRef) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        unsafe { CFRetain(raw) };
        Some(Self(raw))
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
        CFStringGetCString(
            string,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
        )
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
///
/// Ownership: `CFArrayGetValueAtIndex` yields NON-owning borrows whose
/// backing array dies with its [`CfRef`]. Every enqueued node is therefore
/// retained into an owned handle — a raw pointer must never outlive the scope
/// that produced it (this exact pattern caused fatal `_AXUIElementValidate`
/// SIGTRAPs; see crash report 2026-08-22-202121).
fn find_descendant_by_role(
    root: AXUIElementRef,
    role: &str,
    max_depth: usize,
    max_nodes: usize,
) -> Option<CfRef> {
    let mut queue: std::collections::VecDeque<(CfRef, usize)> = std::collections::VecDeque::new();
    queue.push_back((CfRef::retained(root as CFTypeRef)?, 0usize));

    let mut visited = 0usize;
    while let Some((element, depth)) = queue.pop_front() {
        if depth > max_depth || visited >= max_nodes {
            return None;
        }
        visited += 1;
        if element_role(element.as_element()).as_deref() == Some(role) {
            return Some(element);
        }
        if let Some(children) = copy_attribute(element.as_element(), ax_attr!("AXChildren")) {
            let count = unsafe { CFArrayGetCount(children.as_array()) };
            for index in 0..count {
                let child = unsafe { CFArrayGetValueAtIndex(children.as_array(), index) };
                if let Some(owned) = CfRef::retained(child) {
                    queue.push_back((owned, depth + 1));
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
    let absolute = CfRef::take(unsafe { CFURLCopyAbsoluteURL(url.0 as CFURLRef) })?;
    let string = unsafe { CFURLGetString(absolute.0 as CFURLRef) };
    cf_string_to_string(string)
}

fn window_url_chromium(window: AXUIElementRef) -> Option<String> {
    // Owned handles: toolbar and field each keep their own reference alive
    // independent of any parent attribute fetch.
    let Some(toolbar) = find_descendant_by_role(window, "AXToolbar", 2, 64) else {
        eprintln!("browser_ax: chromium — no AXToolbar within 2 levels / 64 nodes");
        return None;
    };
    let Some(field) = find_descendant_by_role(toolbar.as_element(), "AXTextField", 16, 1024) else {
        eprintln!("browser_ax: chromium — no AXTextField (omnibox) within 16 levels / 1024 nodes — retrying 24/2048");
        if let Some(field) = find_descendant_by_role(toolbar.as_element(), "AXTextField", 24, 2048) {
            let Some(value) = element_value_string(field.as_element()) else {
                eprintln!("browser_ax: chromium — omnibox retry value was not a string");
                return None;
            };
            if value.starts_with("http://") || value.starts_with("https://") {
                return Some(value);
            }
            eprintln!("browser_ax: chromium — omnibox retry value not URL-shaped: {:?}", value.chars().take(80).collect::<String>());
            return None;
        }
        return None;
    };
    let Some(value) = element_value_string(field.as_element()) else {
        eprintln!("browser_ax: chromium — omnibox value was not a string");
        return None;
    };
    // The omnibox holds the URL while browsing; only trust URL-shaped values.
    (value.starts_with("http://") || value.starts_with("https://")).then_some(value)
}

/// Chromium browsers only expose their full web-content accessibility tree
/// once an assistive client opts in by setting `AXManualAccessibility` to
/// `true` on the application element. Without this, Chrome/Edge/Brave report
/// no toolbar, omnibox, or web content via AX, so tab URL/title extraction
/// silently returns nothing and gmail.com misclassifies as `Surface::Other`.
/// The flag persists for the browser session once set, so the first capture
/// after a browser launch may still come up empty while Chromium builds its
/// tree; later captures then succeed. Safari exposes its tree unconditionally.
pub fn ensure_chromium_accessibility(pid: i32, bundle_id: Option<&str>) {
    let is_chromium = CHROMIUM_BUNDLE_PREFIXES
        .iter()
        .any(|prefix| bundle_id.is_some_and(|bundle| bundle.starts_with(prefix)));
    if !is_chromium {
        return;
    }
    let app = unsafe { AXUIElementCreateApplication(pid) };
    if app.is_null() {
        return;
    }
    let _app_guard = CfRef(app as CFTypeRef);
    // Wake nudge: while Chromium's accessibility support is dormant its
    // attribute handlers are not registered, so the SETs below fail with
    // kAXErrorAttributeUnsupported (-25205). A basic attribute query makes
    // Chromium register them.
    let _ = copy_attribute(app, ax_attr!("AXRole"));
    // Chromium gates web content behind either flag; set both because support
    // varies by version/channel. AXEnhancedUserInterface is what Chromium's
    // macOS code watches (with a documented ~2s activation delay);
    // AXManualAccessibility is the automation opt-in.
    let status_manual = unsafe {
        AXUIElementSetAttributeValue(app, ax_attr!("AXManualAccessibility"), k_cf_boolean_true())
    };
    let status_enhanced = unsafe {
        AXUIElementSetAttributeValue(app, ax_attr!("AXEnhancedUserInterface"), k_cf_boolean_true())
    };
    if status_manual != AX_SUCCESS || status_enhanced != AX_SUCCESS {
        // eprintln, not log::debug!: this also runs inside the logger-less
        // --context-agent subprocess, whose stderr the supervisor forwards.
        eprintln!(
            "browser_ax: AX opt-in pid {pid} — AXManualAccessibility err={status_manual}, AXEnhancedUserInterface err={status_enhanced}"
        );
    }
}

/// `kCFBooleanTrue` — CoreFoundation's canonical `true` object (exported data
/// symbol, toll-free bridged with NSNumber).
fn k_cf_boolean_true() -> CFTypeRef {
    unsafe { kCFBooleanTrue }
}

/// Capture URL/title for the given frontmost browser. Returns `None` entirely
/// when the bundle isn't a supported browser.
pub fn frontmost_tab(bundle_id: Option<&str>, pid: i32) -> Option<TabInfo> {
    if !is_known_browser(bundle_id) {
        return None;
    }

    // Chromium activates its accessibility tree lazily (~2s after opt-in) and
    // puts it back to sleep shortly after the last AX client disconnects, so a
    // single-shot read usually loses the race. Retry with backoff instead of
    // degrading to None.
    for attempt in 0..6 {
        if let Some(tab) = frontmost_tab_once(bundle_id, pid) {
            if attempt > 0 {
                eprintln!("browser_ax: tab captured on retry {attempt} for pid {pid}");
            }
            return Some(tab);
        }
        eprintln!(
            "browser_ax: attempt {attempt} found no tab data for pid {pid} ({bundle_id:?}); retrying"
        );
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    eprintln!("browser_ax: no tab data for pid {pid} ({bundle_id:?}) after retries");
    None
}

fn frontmost_tab_once(bundle_id: Option<&str>, pid: i32) -> Option<TabInfo> {
    // Read the window tree from the pid-based application element. The
    // system-wide AXFocusedApplication attribute is unreliable from the
    // logger-less --context-agent subprocess (it fails there even when the
    // same query works from a long-lived process), while a pid element works.
    let raw_app = unsafe { AXUIElementCreateApplication(pid) };
    if raw_app.is_null() {
        eprintln!("browser_ax: AXUIElementCreateApplication({pid}) returned null");
        return None;
    }
    let pid_attr = CfRef(raw_app as CFTypeRef);
    let app_element = pid_attr.as_element();

    // Chromium gates its web-content AX tree behind this opt-in; without it
    // the toolbar/omnibox walk below finds nothing.
    ensure_chromium_accessibility(pid, bundle_id);

    // Prefer the app's own focused window; fall back to its main window, then
    // to its first listed window.
    let Some(window_handle) = copy_attribute(app_element, ax_attr!("AXFocusedWindow"))
        .or_else(|| copy_attribute(app_element, ax_attr!("AXMainWindow")))
        .or_else(|| {
            copy_attribute(app_element, ax_attr!("AXWindows")).and_then(|windows| {
                let count = unsafe { CFArrayGetCount(windows.as_array()) };
                (count > 0).then(|| {
                    let first = unsafe { CFArrayGetValueAtIndex(windows.as_array(), 0) };
                    CfRef::retained(first)
                })?
            })
        })
    else {
        eprintln!("browser_ax: no AXFocusedWindow/AXMainWindow/AXWindows for pid {pid} ({bundle_id:?}) — accessibility denied or window closing");
        return None;
    };
    let window = window_handle.as_element();

    let title = copy_attribute(window, ax_attr!("AXTitle")).and_then(|t| t.as_string());

    let is_safari = bundle_id.is_some_and(|bundle| {
        bundle == SAFARI_BUNDLE_ID || bundle.starts_with(SAFARI_WEB_APP_BUNDLE_PREFIX)
    });
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
        eprintln!("browser_ax: no url and no title for {bundle_id:?} pid {pid}");
        return None;
    }
    eprintln!("browser_ax: {bundle_id:?} url={url:?} title={title:?}");

    Some(TabInfo { url, title })
}

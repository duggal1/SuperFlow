//! Session-scoped raw-Escape watcher (macOS).
//!
//! While a dictation session is active, Escape cancels it — but ONLY as part
//! of the deliberate combo gesture: Escape pressed while the transcription
//! trigger key (or any other key) is still held, or carrying the Fn/Globe
//! flag. A BARE Escape is deliberately ignored: the overlay may be on screen
//! while the user is actually aiming Escape at something else (dismissing a
//! dialog, closing a sheet), and hijacking it played the cancel cue out of
//! nowhere. Combo Escapes must be observed as raw keycodes because hotkey
//! matching elsewhere is exact: a registered `escape` binding never fires
//! while the user's transcription trigger key is still down, and `fn+escape`
//! cannot even be registered through the Tauri global-shortcut backend ("fn"
//! is invisible to it).
//!
//! One listen-only session-level CGEventTap lives for the whole app lifetime
//! (the app already requires Accessibility permission for paste injection);
//! [`set_session_active`] merely gates what the callback does, so arming and
//! disarming per session has zero setup cost and no start/stop races. When
//! Secure Input holds the event stream the tap sees nothing, which is fine —
//! dictation into password fields is refused upstream anyway.

#[cfg(target_os = "macos")]
mod imp {
    use log::{error, info, warn};
    use std::collections::HashSet;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Mutex, OnceLock};
    use tauri::{AppHandle, Manager};

    /// Escape keycode on macOS (kVK_Escape).
    const KVK_ESCAPE: i64 = 53;
    /// CGEventType values we care about.
    const KEY_DOWN: u32 = 10;
    const KEY_UP: u32 = 11;
    const FLAGS_CHANGED: u32 = 12;
    const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
    const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
    /// kCGEventFlagMaskSecondaryFn — the Fn/Globe modifier flag. Fn never
    /// arrives as an ordinary keycode keydown on internal keyboards; it only
    /// shows up here (and as keycode-63 flagsChanged events).
    const FLAG_SECONDARY_FN: i64 = 0x0080_0000;

    static SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);
    static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
    /// Keycodes currently physically held, tracked from keyDown/keyUp/
    /// flagsChanged so the callback can tell a bare Escape from the
    /// hold-trigger-hit-Escape combo gesture.
    static DOWN_KEYS: LazyLock<Mutex<HashSet<i64>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));
    /// Re-enabling a disabled tap needs the port; stored once at install.
    /// Raw mach port; only ever dereferenced on the main thread.
    #[derive(Clone, Copy)]
    struct TapPort(std::ptr::NonNull<c_void>);
    unsafe impl Send for TapPort {}
    unsafe impl Sync for TapPort {}

    static TAP_PORT: OnceLock<TapPort> = OnceLock::new();

    type Cfmachportref = *mut c_void;
    type Cgeventref = *mut c_void;
    type Cfrunloopsourceref = *mut c_void;
    type Cfrunloopref = *mut c_void;
    type Cfallocatorref = *const c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            event_mask: u64,
            callback: unsafe extern "C-unwind" fn(
                proxy: *mut c_void,
                event_type: u32,
                event: Cgeventref,
                user_info: *mut c_void,
            ) -> Cgeventref,
            user_info: *mut c_void,
        ) -> Cfmachportref;
        fn CGEventTapEnable(tap: Cfmachportref, enable: bool);
        fn CGEventGetIntegerValueField(event: Cgeventref, field: u32) -> i64;
        fn CGEventGetFlags(event: Cgeventref) -> i64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFAllocatorDefault: Cfallocatorref;
        fn CFMachPortCreateRunLoopSource(
            allocator: Cfallocatorref,
            port: Cfmachportref,
            order: isize,
        ) -> Cfrunloopsourceref;
        fn CFRunLoopAddSource(
            run_loop: Cfrunloopref,
            source: Cfrunloopsourceref,
            mode: *const c_void,
        );
        fn CFRunLoopRun();
    }

    extern "C" {
        fn CFRunLoopGetCurrent() -> Cfrunloopref;
    }

    // kCFRunLoopCommonModes is an extern CFStringRef constant.
    extern "C" {
        static kCFRunLoopCommonModes: *const c_void;
    }

    unsafe extern "C-unwind" fn escape_tap_callback(
        _proxy: *mut c_void,
        event_type: u32,
        event: Cgeventref,
        _user_info: *mut c_void,
    ) -> *mut c_void {
        // macOS stops delivering events to a tap that times out; re-enable
        // immediately (Apple-documented pattern, same as handy-keys' tap).
        if matches!(
            event_type,
            TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER_INPUT
        ) {
            if let Some(TapPort(port)) = TAP_PORT.get() {
                CGEventTapEnable(port.as_ptr(), true);
            }
            return std::ptr::null_mut();
        }

        let keycode = CGEventGetIntegerValueField(event, 9); // kCGKeyboardEventKeycode

        // Maintain the held-key map. flagsChanged events carry the modifier's
        // keycode (fn = 63, cmd = 54, …) with flags telling pressed vs released.
        if event_type == KEY_UP || event_type == FLAGS_CHANGED {
            let down =
                event_type == KEY_DOWN || CGEventGetFlags(event) & 0x00FF_0000 != 0;
            if let Ok(mut keys) = DOWN_KEYS.lock() {
                if down {
                    keys.insert(keycode);
                } else {
                    keys.remove(&keycode);
                }
            }
            return std::ptr::null_mut();
        }
        if event_type != KEY_DOWN || !SESSION_ACTIVE.load(Ordering::Relaxed) {
            return std::ptr::null_mut();
        }

        let is_fn_flagged = CGEventGetFlags(event) & FLAG_SECONDARY_FN != 0;
        if let Ok(mut keys) = DOWN_KEYS.lock() {
            keys.insert(keycode);
        }

        if keycode != KVK_ESCAPE {
            return std::ptr::null_mut();
        }

        // Belt and braces: only cancel while the overlay is actually on
        // screen, so a stale session flag can never swallow an Escape aimed
        // at another app after everything has already settled.
        let overlay_visible = APP_HANDLE
            .get()
            .and_then(|app| app.get_webview_window("recording_overlay"))
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);
        if !overlay_visible {
            return std::ptr::null_mut();
        }

        // Combo gate: Escape only cancels as the deliberate gesture — pressed
        // while another key (the transcription trigger, e.g. Fn) is still held,
        // or carrying the Fn/Globe flag itself. A BARE Escape is ignored: it
        // usually belongs to whatever app the user is actually looking at.
        let combo_gesture = match DOWN_KEYS.lock() {
            Ok(keys) => keys.len() > 1,
            Err(_) => false,
        } || is_fn_flagged;
        if !combo_gesture {
            info!("Bare Escape during session ignored — hold the transcription trigger and press Escape to cancel");
            return std::ptr::null_mut();
        }

        if let Some(app) = APP_HANDLE.get() {
            info!("Trigger+Escape combo during active session — cancelling");
            crate::utils::cancel_current_operation(app);
        }

        std::ptr::null_mut()
    }

    pub fn init(app: &AppHandle) {
        if APP_HANDLE.set(app.clone()).is_err() {
            return; // already installed
        }

        // Session-level tap, head-insert, listen-only (never swallows keys).
        // keyDown drives cancel; keyUp/flagsChanged maintain the held-key map
        // used to tell a bare Escape from the trigger+Escape combo.
        let port = unsafe {
            CGEventTapCreate(
                1, // kCGSessionEventTap
                0, // kCGHeadInsertEventTap
                1, // kCGEventTapOptionListen — observe, do not block
                (1 << KEY_DOWN) | (1 << KEY_UP) | (1 << FLAGS_CHANGED),
                escape_tap_callback,
                std::ptr::null_mut(),
            )
        };
        if port.is_null() {
            warn!(
                "Escape watcher: CGEventTapCreate failed (Accessibility permission missing?) — \
                 keyboard cancel falls back to the configured binding"
            );
            return;
        }
        let _ = TAP_PORT.set(TapPort(
            std::ptr::NonNull::new(port).expect("non-null checked above"),
        ));

        let app_handle = app.clone();
        std::thread::Builder::new()
            .name("escape-cancel-tap".into())
            .spawn(move || unsafe {
                let Some(TapPort(port)) = TAP_PORT.get() else {
                    return;
                };
                let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, port.as_ptr(), 0);
                if source.is_null() {
                    error!("Escape watcher: failed to create run-loop source");
                    return;
                }
                CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
                CGEventTapEnable(port.as_ptr(), true);
                info!("Escape watcher armed (listen-only)");
                let _ = app_handle; // keep the handle alive for the process lifetime
                CFRunLoopRun();
            })
            .map(|_| ())
            .map_err(|e| error!("Escape watcher: failed to spawn tap thread: {e}"))
            .ok();
    }

    pub fn set_session_active(active: bool) {
        SESSION_ACTIVE.store(active, Ordering::Relaxed);
    }
}

#[cfg(target_os = "macos")]
pub use imp::{init, set_session_active};

#[cfg(not(target_os = "macos"))]
pub mod imp_stub {
    pub fn init(_app: &tauri::AppHandle) {}
    pub fn set_session_active(_active: bool) {}
}
#[cfg(not(target_os = "macos"))]
pub use imp_stub::{init, set_session_active};

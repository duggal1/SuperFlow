//! Apple Voice Processing backend – Rust FFI wrapper for enhancer.swift
//! Keeps enhancer.rs dead (experiment) as requested; this Swift path is the active one on macOS.

use std::os::raw::{c_char, c_int};

#[cfg(target_os = "macos")]
extern "C" {
    fn apple_voice_enhancer_create() -> *mut std::ffi::c_void;
    fn apple_voice_enhancer_destroy(handle: *mut std::ffi::c_void);
    fn apple_voice_enhancer_start(
        handle: *mut std::ffi::c_void,
        callback: Option<unsafe extern "C" fn(*const f32, i32, u32, *mut std::ffi::c_void)>,
        context: *mut std::ffi::c_void,
    ) -> c_int;
    fn apple_voice_enhancer_stop(handle: *mut std::ffi::c_void);
    fn apple_voice_enhancer_is_running(handle: *mut std::ffi::c_void) -> c_int;
    fn apple_voice_enhancer_last_error(
        handle: *mut std::ffi::c_void,
        buffer: *mut c_char,
        capacity: c_int,
    ) -> c_int;
    fn apple_voice_enhancer_active_microphone_mode() -> c_int;
    fn apple_voice_enhancer_preferred_microphone_mode() -> c_int;
    fn apple_voice_enhancer_show_microphone_modes();
}

#[cfg(target_os = "macos")]
pub fn show_microphone_modes() {
    unsafe { apple_voice_enhancer_show_microphone_modes() }
}

#[cfg(target_os = "macos")]
pub fn active_microphone_mode() -> i32 {
    unsafe { apple_voice_enhancer_active_microphone_mode() }
}

#[cfg(target_os = "macos")]
pub fn preferred_microphone_mode() -> i32 {
    unsafe { apple_voice_enhancer_preferred_microphone_mode() }
}

#[cfg(target_os = "macos")]
pub struct AppleVoiceHandle {
    raw: *mut std::ffi::c_void,
}

#[cfg(target_os = "macos")]
impl AppleVoiceHandle {
    pub fn new() -> Option<Self> {
        let raw = unsafe { apple_voice_enhancer_create() };
        if raw.is_null() {
            None
        } else {
            Some(Self { raw })
        }
    }

    pub fn start(
        &self,
        callback: unsafe extern "C" fn(*const f32, i32, u32, *mut std::ffi::c_void),
        context: *mut std::ffi::c_void,
    ) -> Result<(), String> {
        let rc = unsafe { apple_voice_enhancer_start(self.raw, Some(callback), context) };
        if rc == 0 {
            Ok(())
        } else {
            let mut buf = vec![0u8; 512];
            unsafe {
                apple_voice_enhancer_last_error(
                    self.raw,
                    buf.as_mut_ptr() as *mut c_char,
                    buf.len() as c_int,
                );
            }
            let msg = String::from_utf8_lossy(&buf)
                .trim_matches(char::from(0))
                .trim()
                .to_string();
            Err(if msg.is_empty() {
                "Apple Voice Processing failed to start".to_string()
            } else {
                msg
            })
        }
    }

    pub fn stop(&self) {
        unsafe { apple_voice_enhancer_stop(self.raw) }
    }

    pub fn is_running(&self) -> bool {
        unsafe { apple_voice_enhancer_is_running(self.raw) != 0 }
    }
}

#[cfg(target_os = "macos")]
impl Drop for AppleVoiceHandle {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe { apple_voice_enhancer_destroy(self.raw) }
        }
    }
}

#[cfg(target_os = "macos")]
unsafe impl Send for AppleVoiceHandle {}
#[cfg(target_os = "macos")]
unsafe impl Sync for AppleVoiceHandle {}

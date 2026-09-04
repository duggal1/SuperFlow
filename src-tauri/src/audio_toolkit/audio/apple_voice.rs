//! Apple Voice Processing backend – Rust FFI wrapper for enhancer.swift
//! Keeps enhancer.rs dead (experiment) as requested; this Swift path is the active one on macOS.

use std::os::raw::c_int;

#[cfg(target_os = "macos")]
extern "C" {
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

#![cfg(target_os = "macos")]

use std::ffi::{c_char, CStr};

extern "C" {
    fn superflow_capture_page_context(pid: i32) -> *mut c_char;
    fn superflow_free_page_context(pointer: *mut c_char);
}

pub fn capture(pid: i32) -> Option<String> {
    let pointer = unsafe { superflow_capture_page_context(pid) };
    if pointer.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned();
    unsafe { superflow_free_page_context(pointer) };
    (!text.trim().is_empty()).then_some(text)
}

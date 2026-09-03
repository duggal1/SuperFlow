#[cfg(target_os = "macos")]
use crate::audio_toolkit::audio::{
    active_microphone_mode, preferred_microphone_mode, show_microphone_modes,
};

#[tauri::command]
#[specta::specta]
pub fn apple_voice_show_microphone_modes() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        show_microphone_modes();
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Microphone modes only available on macOS".to_string())
    }
}

#[tauri::command]
#[specta::specta]
pub fn apple_voice_get_microphone_modes() -> Result<(i32, i32), String> {
    #[cfg(target_os = "macos")]
    {
        Ok((active_microphone_mode(), preferred_microphone_mode()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Microphone modes only available on macOS".to_string())
    }
}

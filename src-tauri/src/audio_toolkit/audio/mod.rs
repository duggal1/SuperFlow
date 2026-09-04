// Re-export all audio components
mod apple_voice;
mod device;
mod recorder;
mod resampler;
mod utils;
mod visualizer;

// The apple-voice microphone-mode API is macOS-only (backed by enhancer.swift);
// the commands layer returns an error on every other platform.
#[cfg(target_os = "macos")]
pub use apple_voice::{active_microphone_mode, preferred_microphone_mode, show_microphone_modes};
pub use device::{
    list_input_devices, list_output_devices, safe_macos_input_device_name, CpalDeviceInfo,
};
pub use recorder::{
    is_microphone_access_denied, is_no_input_device_error, AudioRecorder, VadPolicy,
};
pub use resampler::FrameResampler;
pub use utils::{read_f32_part, read_wav_samples, save_wav_file, verify_wav_file};
pub use visualizer::AudioVisualiser;

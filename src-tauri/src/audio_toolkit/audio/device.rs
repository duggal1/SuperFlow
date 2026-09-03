use cpal::traits::{DeviceTrait, HostTrait};

#[cfg(target_os = "macos")]
use std::ffi::{CStr, CString};

pub struct CpalDeviceInfo {
    pub index: String,
    pub name: String,
    pub is_default: bool,
    pub device: cpal::Device,
}

pub fn list_input_devices() -> Result<Vec<CpalDeviceInfo>, Box<dyn std::error::Error>> {
    let host = crate::audio_toolkit::get_cpal_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());

    let mut out = Vec::<CpalDeviceInfo>::new();

    for (index, device) in host.input_devices()?.enumerate() {
        let name = device.name().unwrap_or_else(|_| "Unknown".into());

        let is_default = Some(name.clone()) == default_name;

        out.push(CpalDeviceInfo {
            index: index.to_string(),
            name,
            is_default,
            device,
        });
    }

    Ok(out)
}

pub fn list_output_devices() -> Result<Vec<CpalDeviceInfo>, Box<dyn std::error::Error>> {
    let host = crate::audio_toolkit::get_cpal_host();
    let default_name = host.default_output_device().and_then(|d| d.name().ok());

    let mut out = Vec::<CpalDeviceInfo>::new();

    for (index, device) in host.output_devices()?.enumerate() {
        let name = device.name().unwrap_or_else(|_| "Unknown".into());

        let is_default = Some(name.clone()) == default_name;

        out.push(CpalDeviceInfo {
            index: index.to_string(),
            name,
            is_default,
            device,
        });
    }

    Ok(out)
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn superflow_safe_input_device_name(
        requested_name: *const std::ffi::c_char,
        output: *mut std::ffi::c_char,
        output_capacity: usize,
    ) -> bool;
}

/// CoreAudio transport-aware fallback for Bluetooth headset microphones.
/// Returns `None` for every safe input, so normal device selection is unchanged.
pub fn safe_macos_input_device_name(requested_name: Option<&str>) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let requested = requested_name.and_then(|name| CString::new(name).ok());
        let mut output = [0_i8; 1024];
        let changed = unsafe {
            superflow_safe_input_device_name(
                requested
                    .as_ref()
                    .map_or(std::ptr::null(), |name| name.as_ptr()),
                output.as_mut_ptr(),
                output.len(),
            )
        };
        if !changed {
            return None;
        }
        let name = unsafe { CStr::from_ptr(output.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        (!name.is_empty()).then_some(name)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = requested_name;
        None
    }
}

use crate::calendar::{CalendarErrorResult, CalendarSuccessResult, ValidatedCalendarEvent};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[cfg(target_os = "macos")]
extern "C" {
    fn superflow_calendar_create_event(
        title: *const c_char,
        start: *const c_char,
        end: *const c_char,
        calendar: *const c_char,
        location: *const c_char,
        notes: *const c_char,
        reminders: *const c_char,
        success_message: *const c_char,
    ) -> *mut c_char;
    fn superflow_calendar_free_string(ptr: *mut c_char);
}

#[cfg(not(target_os = "macos"))]
fn superflow_calendar_create_event_stub() -> *mut c_char {
    std::ptr::null_mut()
}

pub fn create_event(
    validated: &ValidatedCalendarEvent,
) -> Result<CalendarSuccessResult, CalendarErrorResult> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err(CalendarErrorResult {
            ok: false,
            error: "unsupported".to_string(),
            message: "Calendar only supported on macOS".to_string(),
            details: None,
        });
    }

    #[cfg(target_os = "macos")]
    unsafe {
        let title = CString::new(validated.title.clone()).unwrap();
        let start = CString::new(validated.start_str.clone()).unwrap();
        let end = CString::new(validated.end_str.clone()).unwrap();
        let success_msg = CString::new(validated.success_message.clone()).unwrap();

        let calendar_c = validated
            .calendar
            .as_ref()
            .map(|s| CString::new(s.clone()).unwrap());
        let location_c = validated
            .location
            .as_ref()
            .map(|s| CString::new(s.clone()).unwrap());
        let notes_c = validated
            .notes
            .as_ref()
            .map(|s| CString::new(s.clone()).unwrap());
        let reminders_json = if validated.reminders_minutes_before.is_empty() {
            None
        } else {
            let json = serde_json::to_string(&validated.reminders_minutes_before).unwrap();
            Some(CString::new(json).unwrap())
        };

        let calendar_ptr = calendar_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());
        let location_ptr = location_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());
        let notes_ptr = notes_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());
        let reminders_ptr = reminders_json
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        let result_ptr = superflow_calendar_create_event(
            title.as_ptr(),
            start.as_ptr(),
            end.as_ptr(),
            calendar_ptr,
            location_ptr,
            notes_ptr,
            reminders_ptr,
            success_msg.as_ptr(),
        );

        if result_ptr.is_null() {
            return Err(CalendarErrorResult {
                ok: false,
                error: "eventkit_error".to_string(),
                message: "Failed to create calendar event (null response)".to_string(),
                details: None,
            });
        }

        let c_str = CStr::from_ptr(result_ptr);
        let json_str = c_str.to_string_lossy().to_string();
        superflow_calendar_free_string(result_ptr);

        // Parse Swift JSON response
        let val: serde_json::Value =
            serde_json::from_str(&json_str).map_err(|_| CalendarErrorResult {
                ok: false,
                error: "eventkit_error".to_string(),
                message: "Invalid response from Calendar helper".to_string(),
                details: Some(json_str.clone()),
            })?;

        if val.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            let res: CalendarSuccessResult =
                serde_json::from_value(val).map_err(|e| CalendarErrorResult {
                    ok: false,
                    error: "eventkit_error".to_string(),
                    message: format!("Failed to parse success: {}", e),
                    details: Some(json_str),
                })?;
            Ok(res)
        } else {
            let err: CalendarErrorResult =
                serde_json::from_value(val.clone()).unwrap_or(CalendarErrorResult {
                    ok: false,
                    error: val
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("eventkit_error")
                        .to_string(),
                    message: val
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Calendar error")
                        .to_string(),
                    details: Some(json_str),
                });
            Err(err)
        }
    }
}

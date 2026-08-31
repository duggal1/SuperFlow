import EventKit
import Foundation

private func jsonError(_ code: String, _ message: String) -> String {
    let obj: [String: Any] = ["ok": false, "error": code, "message": message]
    if let data = try? JSONSerialization.data(withJSONObject: obj, options: []),
       let str = String(data: data, encoding: .utf8) {
        return str
    }
    return "{\"ok\":false,\"error\":\"\(code)\",\"message\":\"\(message)\"}"
}

private func jsonSuccess(eventId: String, title: String, start: String, end: String, calendar: String, successMessage: String) -> String {
    let obj: [String: Any] = [
        "ok": true,
        "action": "calendar.create_event",
        "title": title,
        "start": start,
        "end": end,
        "calendar": calendar,
        "event_id": eventId,
        "success_message": successMessage
    ]
    if let data = try? JSONSerialization.data(withJSONObject: obj, options: []),
       let str = String(data: data, encoding: .utf8) {
        return str
    }
    return "{\"ok\":false,\"error\":\"serialization\",\"message\":\"Failed to serialize success\"}"
}

// Helper to parse ISO8601 with timezone
private func parseISO8601(_ str: String) -> Date? {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    if let d = formatter.date(from: str) {
        return d
    }
    // Try without fractional seconds
    formatter.formatOptions = [.withInternetDateTime]
    return formatter.date(from: str)
}

// Find writable calendar by name, or default, or first writable
private func findCalendar(store: EKEventStore, name: String?) -> EKCalendar? {
    let calendars = store.calendars(for: .event)
    let writable = calendars.filter { $0.allowsContentModifications }
    if let name = name, !name.trimmingCharacters(in: .whitespaces).isEmpty {
        if let found = writable.first(where: { $0.title.caseInsensitiveCompare(name) == .orderedSame }) {
            return found
        }
        // Also try contains
        if let found = writable.first(where: { $0.title.localizedCaseInsensitiveContains(name) }) {
            return found
        }
    }
    if let def = store.defaultCalendarForNewEvents, def.allowsContentModifications {
        return def
    }
    return writable.first
}

// Synchronous permission check/request using semaphore
private func ensureCalendarAccess(store: EKEventStore) -> (Bool, String?) {
    let status = EKEventStore.authorizationStatus(for: .event)
    switch status {
    case .authorized, .fullAccess:
        return (true, nil)
    case .notDetermined:
        var granted = false
        var requestError: String? = nil
        let semaphore = DispatchSemaphore(value: 0)
        if #available(macOS 14.0, *) {
            Task {
                do {
                    granted = try await store.requestFullAccessToEvents()
                } catch {
                    requestError = error.localizedDescription
                }
                semaphore.signal()
            }
        } else {
            store.requestAccess(to: .event) { ok, err in
                granted = ok
                if let e = err { requestError = e.localizedDescription }
                semaphore.signal()
            }
        }
        semaphore.wait()
        if granted {
            return (true, nil)
        } else {
            return (false, requestError ?? "Calendar access denied")
        }
    case .denied, .restricted:
        return (false, "Calendar access denied. Enable in System Settings → Privacy & Security → Calendars.")
    case .writeOnly:
        // writeOnly still allows creating events
        return (true, nil)
    @unknown default:
        return (false, "Unknown calendar authorization status")
    }
}

@_cdecl("superflow_calendar_create_event")
public func superflowCalendarCreateEvent(
    _ titlePtr: UnsafePointer<CChar>?,
    _ startPtr: UnsafePointer<CChar>?,
    _ endPtr: UnsafePointer<CChar>?,
    _ calendarPtr: UnsafePointer<CChar>?,
    _ locationPtr: UnsafePointer<CChar>?,
    _ notesPtr: UnsafePointer<CChar>?,
    _ remindersPtr: UnsafePointer<CChar>?,
    _ successMessagePtr: UnsafePointer<CChar>?
) -> UnsafeMutablePointer<CChar>? {

    guard let titlePtr = titlePtr, let startPtr = startPtr, let endPtr = endPtr, let successMessagePtr = successMessagePtr else {
        let err = jsonError("validation_error", "Missing required fields")
        return strdup(err)
    }
    let title = String(cString: titlePtr).trimmingCharacters(in: .whitespacesAndNewlines)
    let startStr = String(cString: startPtr)
    let endStr = String(cString: endPtr)
    let successMessage = String(cString: successMessagePtr)
    let calendarName: String? = calendarPtr.map { String(cString: $0) }
    let location: String? = locationPtr.map { String(cString: $0) }.flatMap { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : $0 }
    let notes: String? = notesPtr.map { String(cString: $0) }.flatMap { $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : $0 }
    let remindersStr: String? = remindersPtr.map { String(cString: $0) }

    if title.isEmpty {
        let err = jsonError("validation_error", "Title is required")
        return strdup(err)
    }
    guard let startDate = parseISO8601(startStr) else {
        let err = jsonError("validation_error", "Invalid start time format")
        return strdup(err)
    }
    guard let endDate = parseISO8601(endStr) else {
        let err = jsonError("validation_error", "Invalid end time format")
        return strdup(err)
    }
    if endDate <= startDate {
        let err = jsonError("validation_error", "End must be after start")
        return strdup(err)
    }

    // Parse reminders: JSON array string like "[15,10]" or "15" or null
    var reminderMinutes: [Int] = []
    if let r = remindersStr, !r.trimmingCharacters(in: .whitespaces).isEmpty, r != "null" {
        if let data = r.data(using: .utf8),
           let arr = try? JSONSerialization.jsonObject(with: data) as? [Int] {
            reminderMinutes = arr
        } else if let data = r.data(using: .utf8),
                  let single = try? JSONSerialization.jsonObject(with: data) as? Int {
            reminderMinutes = [single]
        } else {
            // Try comma separated
            let parts = r.replacingOccurrences(of: "[", with: "").replacingOccurrences(of: "]", with: "").split(separator: ",")
            for p in parts {
                if let v = Int(p.trimmingCharacters(in: .whitespaces)) {
                    reminderMinutes.append(v)
                }
            }
        }
    }

    let store = EKEventStore()
    let (hasAccess, accessErr) = ensureCalendarAccess(store: store)
    if !hasAccess {
        let err = jsonError("permission_denied", accessErr ?? "Calendar access denied")
        return strdup(err)
    }

    guard let calendar = findCalendar(store: store, name: calendarName) else {
        let err = jsonError("calendar_not_found", "No writable calendar found")
        return strdup(err)
    }

    let event = EKEvent(eventStore: store)
    event.title = title
    event.startDate = startDate
    event.endDate = endDate
    event.calendar = calendar
    if let loc = location { event.location = loc }
    if let n = notes { event.notes = n }
    // Alarms
    if !reminderMinutes.isEmpty {
        var alarms: [EKAlarm] = []
        for m in reminderMinutes {
            // Clamp 0...40320 (4 weeks)
            let clamped = max(0, min(m, 40320))
            let alarm = EKAlarm(relativeOffset: Double(-clamped * 60))
            alarms.append(alarm)
        }
        event.alarms = alarms
    }

    do {
        try store.save(event, span: .thisEvent)
        let result = jsonSuccess(eventId: event.eventIdentifier ?? "", title: title, start: startStr, end: endStr, calendar: calendar.title, successMessage: successMessage)
        return strdup(result)
    } catch {
        let err = jsonError("eventkit_error", "Failed to save event: \(error.localizedDescription)")
        return strdup(err)
    }
}

@_cdecl("superflow_calendar_check_permission")
public func superflowCalendarCheckPermission() -> Int32 {
    let status = EKEventStore.authorizationStatus(for: .event)
    switch status {
    case .authorized, .fullAccess, .writeOnly:
        return 1 // authorized
    case .notDetermined:
        return 0 // not determined
    case .denied, .restricted:
        return 2 // denied
    @unknown default:
        return 2
    }
}

@_cdecl("superflow_calendar_free_string")
public func superflowCalendarFreeString(_ ptr: UnsafeMutablePointer<CChar>?) {
    if let p = ptr { free(p) }
}

// For testing / opening Calendar app via AppleScript helper (narrow use)
@_cdecl("superflow_calendar_open_app")
public func superflowCalendarOpenApp() -> Bool {
    let task = Process()
    task.launchPath = "/usr/bin/open"
    task.arguments = ["-a", "Calendar"]
    do {
        try task.run()
        return true
    } catch {
        return false
    }
}

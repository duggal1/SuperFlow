import EventKit
import Foundation

public actor CalendarAutomation: AutomationServiceHandler {
    public nonisolated let service: AutomationService = .calendar
    private let store: EKEventStore

    public init(store: EKEventStore = EKEventStore()) {
        self.store = store
    }

    public func execute(
        action: String,
        parameters: [String: JSONValue]
    ) async throws -> [String: JSONValue] {
        try await ensureAccess()

        switch action {
        case "find":
            return try await find(parameters)
        case "create":
            return try create(parameters)
        case "update":
            return try await update(parameters)
        case "delete":
            return try await delete(parameters)
        default:
            throw AutomationError.unsupportedAction(service: .calendar, action: action)
        }
    }

    private func ensureAccess() async throws {
        let status = EKEventStore.authorizationStatus(for: .event)

        if #available(macOS 14.0, *) {
            if status == .fullAccess || status == .authorized || status == .writeOnly {
                return
            }
        } else {
            if status == .authorized {
                return
            }
        }

        if status == .denied || status == .restricted {
            throw AutomationError.permissionDenied("Calendar access is denied. Enable in System Settings → Privacy & Security → Calendars.")
        }

        // notDetermined
        let granted: Bool = try await withCheckedThrowingContinuation { continuation in
            if #available(macOS 14.0, *) {
                store.requestFullAccessToEvents { granted, error in
                    if let error {
                        continuation.resume(throwing: error)
                    } else {
                        continuation.resume(returning: granted)
                    }
                }
            } else {
                store.requestAccess(to: .event) { granted, error in
                    if let error {
                        continuation.resume(throwing: error)
                    } else {
                        continuation.resume(returning: granted)
                    }
                }
            }
        }

        guard granted else {
            throw AutomationError.permissionDenied("Calendar access was not granted")
        }
    }

    // MARK: - find

    private func find(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let query = try ParameterReader.string("query", from: parameters)!
        let limit = min(max(try ParameterReader.int("limit", from: parameters, default: 20), 1), 50)
        let fromStr = try ParameterReader.string("from", from: parameters, required: false)
        let toStr = try ParameterReader.string("to", from: parameters, required: false)

        let fromDate: Date
        let toDate: Date

        if let s = fromStr, let d = Self.isoDate(s) {
            fromDate = d
        } else {
            fromDate = Calendar.current.date(byAdding: .month, value: -1, to: Date()) ?? Date()
        }

        if let s = toStr, let d = Self.isoDate(s) {
            toDate = d
        } else {
            toDate = Calendar.current.date(byAdding: .month, value: 3, to: Date()) ?? Date()
        }

        let calendars = store.calendars(for: .event)
        let predicate = store.predicateForEvents(withStart: fromDate, end: toDate, calendars: calendars)
        let events = store.events(matching: predicate)
            .filter { event in
                event.title.localizedCaseInsensitiveContains(query) ||
                (event.notes?.localizedCaseInsensitiveContains(query) ?? false) ||
                (event.location?.localizedCaseInsensitiveContains(query) ?? false) ||
                event.calendar.title.localizedCaseInsensitiveContains(query)
            }
            .sorted { ($0.startDate ?? Date.distantPast) < ($1.startDate ?? Date.distantPast) }
            .prefix(limit)

        let matches: [JSONValue] = events.map { event in
            var obj: [String: JSONValue] = [
                "eventId": .string(event.eventIdentifier ?? ""),
                "title": .string(event.title ?? "(No Title)"),
                "calendar": .string(event.calendar.title),
                "isAllDay": .bool(event.isAllDay),
                "status": .string("\(event.status.rawValue)")
            ]
            if let start = event.startDate {
                obj["startAt"] = .string(ISO8601DateFormatter().string(from: start))
            }
            if let end = event.endDate {
                obj["endAt"] = .string(ISO8601DateFormatter().string(from: end))
            }
            if let loc = event.location, !loc.isEmpty {
                obj["location"] = .string(loc)
            }
            if let notes = event.notes, !notes.isEmpty {
                obj["notes"] = .string(String(notes.prefix(2000)))
            }
            return .object(obj)
        }

        return [
            "query": .string(query),
            "count": .int(matches.count),
            "matches": .array(Array(matches))
        ]
    }

    // MARK: - create

    private func create(_ parameters: [String: JSONValue]) throws -> [String: JSONValue] {
        let title = try ParameterReader.string("title", from: parameters)!
        let startAt = try ParameterReader.string("startAt", from: parameters)!
        let endAt = try ParameterReader.string("endAt", from: parameters, required: false)
        let durationMinutes = try? ParameterReader.int("durationMinutes", from: parameters, default: 60)
        let calendarName = try ParameterReader.string("calendar", from: parameters, required: false)
        let notes = try ParameterReader.string("notes", from: parameters, required: false)
        let location = try ParameterReader.string("location", from: parameters, required: false)

        guard !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw AutomationError.invalidInput("calendar.create requires non-empty title")
        }

        guard let startDate = Self.isoDate(startAt) else {
            throw AutomationError.invalidInput("startAt must be ISO-8601 with timezone, e.g. 2026-09-04T10:00:00+05:30")
        }

        let endDate: Date
        if let e = endAt, let d = Self.isoDate(e) {
            endDate = d
        } else if let mins = durationMinutes, mins > 0 {
            endDate = startDate.addingTimeInterval(Double(mins * 60))
        } else {
            endDate = startDate.addingTimeInterval(3600)
        }

        guard endDate > startDate else {
            throw AutomationError.invalidInput("endAt must be after startAt")
        }

        if endDate.timeIntervalSince(startDate) > 24 * 3600 {
            throw AutomationError.invalidInput("Calendar event too long (max 24h)")
        }

        if endDate < Date().addingTimeInterval(-5 * 60) {
            throw AutomationError.invalidInput("Event is in the past")
        }

        guard let calendar = resolveWritableCalendar(named: calendarName) else {
            throw AutomationError.executionFailed("No writable calendar found")
        }

        let event = EKEvent(eventStore: store)
        event.title = title
        event.startDate = startDate
        event.endDate = endDate
        event.calendar = calendar
        event.notes = notes
        event.location = location

        try store.save(event, span: .thisEvent, commit: true)

        var result: [String: JSONValue] = [
            "eventId": .string(event.eventIdentifier ?? ""),
            "title": .string(title),
            "startAt": .string(ISO8601DateFormatter().string(from: startDate)),
            "endAt": .string(ISO8601DateFormatter().string(from: endDate)),
            "calendar": .string(calendar.title),
            "created": .bool(true)
        ]
        if let l = location { result["location"] = .string(l) }
        if let n = notes { result["notes"] = .string(n) }
        return result
    }

    // MARK: - update

    private func update(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let eventId = try ParameterReader.string("eventId", from: parameters)!
        let title = try ParameterReader.string("title", from: parameters, required: false)
        let startAt = try ParameterReader.string("startAt", from: parameters, required: false)
        let endAt = try ParameterReader.string("endAt", from: parameters, required: false)
        let notes = try ParameterReader.string("notes", from: parameters, required: false)
        let location = try ParameterReader.string("location", from: parameters, required: false)

        guard let event = store.event(withIdentifier: eventId) else {
            throw AutomationError.executionFailed("Calendar event not found: \(eventId)")
        }

        if let t = title, !t.trimmingCharacters(in: .whitespaces).isEmpty {
            event.title = t
        }
        if let s = startAt {
            guard let d = Self.isoDate(s) else {
                throw AutomationError.invalidInput("startAt must be ISO-8601")
            }
            event.startDate = d
        }
        if let e = endAt {
            guard let d = Self.isoDate(e) else {
                throw AutomationError.invalidInput("endAt must be ISO-8601")
            }
            event.endDate = d
        }
        if let n = notes { event.notes = n }
        if let l = location { event.location = l }

        try store.save(event, span: .thisEvent, commit: true)

        return [
            "eventId": .string(event.eventIdentifier ?? eventId),
            "updated": .bool(true)
        ]
    }

    // MARK: - delete

    private func delete(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let eventId = try ParameterReader.string("eventId", from: parameters)!
        guard let event = store.event(withIdentifier: eventId) else {
            throw AutomationError.executionFailed("Calendar event not found: \(eventId)")
        }
        try store.remove(event, span: .thisEvent, commit: true)
        return [
            "eventId": .string(eventId),
            "deleted": .bool(true)
        ]
    }

    // MARK: - helpers

    private func resolveWritableCalendar(named name: String?) -> EKCalendar? {
        let calendars = store.calendars(for: .event).filter { $0.allowsContentModifications }
        if let name = name?.trimmingCharacters(in: .whitespacesAndNewlines), !name.isEmpty {
            if let found = calendars.first(where: { $0.title.caseInsensitiveCompare(name) == .orderedSame }) {
                return found
            }
            if let found = calendars.first(where: { $0.title.localizedCaseInsensitiveContains(name) }) {
                return found
            }
        }
        if let def = store.defaultCalendarForNewEvents, def.allowsContentModifications {
            return def
        }
        return calendars.first
    }

    private static func isoDate(_ s: String) -> Date? {
        let f1 = ISO8601DateFormatter()
        f1.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let d = f1.date(from: s) { return d }
        let f2 = ISO8601DateFormatter()
        f2.formatOptions = [.withInternetDateTime]
        return f2.date(from: s)
    }
}

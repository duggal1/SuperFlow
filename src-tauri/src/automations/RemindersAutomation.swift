import Foundation
import EventKit

public actor RemindersAutomation: AutomationServiceHandler {
    public nonisolated let service: AutomationService = .reminders
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
        case "create_list":
            return try createList(parameters)
        case "complete":
            return try await complete(parameters)
        default:
            throw AutomationError.unsupportedAction(service: .reminders, action: action)
        }
    }

    private func ensureAccess() async throws {
        let status = EKEventStore.authorizationStatus(for: .reminder)

        if #available(macOS 14.0, *) {
            if status == .fullAccess || status == .authorized {
                return
            }
        } else {
            if status == .authorized {
                return
            }
        }

        if status == .denied || status == .restricted {
            throw AutomationError.permissionDenied("Reminders access is denied")
        }

        let granted: Bool = try await withCheckedThrowingContinuation { continuation in
            if #available(macOS 14.0, *) {
                store.requestFullAccessToReminders { granted, error in
                    if let error {
                        continuation.resume(throwing: error)
                    } else {
                        continuation.resume(returning: granted)
                    }
                }
            } else {
                store.requestAccess(to: .reminder) { granted, error in
                    if let error {
                        continuation.resume(throwing: error)
                    } else {
                        continuation.resume(returning: granted)
                    }
                }
            }
        }

        guard granted else {
            throw AutomationError.permissionDenied("Reminders access was not granted")
        }
    }

    private func find(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let query = try ParameterReader.string("query", from: parameters)!
        let limit = min(max(try ParameterReader.int("limit", from: parameters, default: 20), 1), 100)
        let predicate = store.predicateForReminders(in: nil)

        let reminders: [EKReminder] = await withCheckedContinuation { continuation in
            store.fetchReminders(matching: predicate) { reminders in
                continuation.resume(returning: reminders ?? [])
            }
        }

        let matches = reminders
            .filter {
                $0.title.localizedCaseInsensitiveContains(query) ||
                ($0.notes?.localizedCaseInsensitiveContains(query) ?? false)
            }
            .prefix(limit)
            .map { reminder -> JSONValue in
                var object: [String: JSONValue] = [
                    "reminderId": .string(reminder.calendarItemIdentifier),
                    "title": .string(reminder.title),
                    "completed": .bool(reminder.isCompleted),
                    "list": .string(reminder.calendar.title)
                ]

                if let notes = reminder.notes {
                    object["notes"] = .string(notes)
                }

                if let due = reminder.dueDateComponents?.date {
                    object["dueAt"] = .string(ISO8601DateFormatter().string(from: due))
                }

                return .object(object)
            }

        return [
            "query": .string(query),
            "count": .int(matches.count),
            "matches": .array(Array(matches))
        ]
    }

    private func create(_ parameters: [String: JSONValue]) throws -> [String: JSONValue] {
        let title = try ParameterReader.string("title", from: parameters)!
        let listName = try ParameterReader.string("list", from: parameters, required: false)
        let notes = try ParameterReader.string("notes", from: parameters, required: false)
        let dueAt = try ParameterReader.string("dueAt", from: parameters, required: false)
        let priority = try ParameterReader.int("priority", from: parameters, default: 0)

        let calendar = try resolveCalendar(named: listName, createIfMissing: false)
        let reminder = EKReminder(eventStore: store)
        reminder.title = title
        reminder.calendar = calendar
        reminder.notes = notes
        reminder.priority = min(max(priority, 0), 9)

        if let dueAt {
            guard let date = ISO8601DateFormatter().date(from: dueAt) else {
                throw AutomationError.invalidInput("dueAt must be ISO-8601")
            }
            reminder.dueDateComponents = Calendar.current.dateComponents(
                [.calendar, .timeZone, .year, .month, .day, .hour, .minute, .second],
                from: date
            )
        }

        try store.save(reminder, commit: true)

        return [
            "reminderId": .string(reminder.calendarItemIdentifier),
            "title": .string(title),
            "list": .string(calendar.title),
            "created": .bool(true)
        ]
    }

    private func createList(_ parameters: [String: JSONValue]) throws -> [String: JSONValue] {
        let name = try ParameterReader.string("name", from: parameters)!

        if let existing = store.calendars(for: .reminder).first(where: {
            $0.title.caseInsensitiveCompare(name) == .orderedSame
        }) {
            return [
                "listId": .string(existing.calendarIdentifier),
                "name": .string(existing.title),
                "created": .bool(false)
            ]
        }

        guard let source = store.defaultCalendarForNewReminders()?.source ??
                store.sources.first(where: { $0.sourceType == .local }) ??
                store.sources.first else {
            throw AutomationError.executionFailed("No writable Reminders source is available")
        }

        let calendar = EKCalendar(for: .reminder, eventStore: store)
        calendar.title = name
        calendar.source = source
        try store.saveCalendar(calendar, commit: true)

        return [
            "listId": .string(calendar.calendarIdentifier),
            "name": .string(calendar.title),
            "created": .bool(true)
        ]
    }

    private func complete(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let reminderID = try ParameterReader.string("reminderId", from: parameters)!
        let predicate = store.predicateForReminders(in: nil)

        let reminders: [EKReminder] = await withCheckedContinuation { continuation in
            store.fetchReminders(matching: predicate) { reminders in
                continuation.resume(returning: reminders ?? [])
            }
        }

        guard let reminder = reminders.first(where: { $0.calendarItemIdentifier == reminderID }) else {
            throw AutomationError.executionFailed("Reminder not found")
        }

        reminder.isCompleted = true
        reminder.completionDate = Date()
        try store.save(reminder, commit: true)

        return [
            "reminderId": .string(reminderID),
            "completed": .bool(true)
        ]
    }

    private func resolveCalendar(
        named name: String?,
        createIfMissing: Bool
    ) throws -> EKCalendar {
        if let name {
            if let match = store.calendars(for: .reminder).first(where: {
                $0.title.caseInsensitiveCompare(name) == .orderedSame
            }) {
                return match
            }

            if createIfMissing {
                guard let source = store.defaultCalendarForNewReminders()?.source ??
                        store.sources.first else {
                    throw AutomationError.executionFailed("No writable Reminders source is available")
                }

                let calendar = EKCalendar(for: .reminder, eventStore: store)
                calendar.title = name
                calendar.source = source
                try store.saveCalendar(calendar, commit: true)
                return calendar
            }

            throw AutomationError.executionFailed("Reminder list \(name) not found")
        }

        guard let calendar = store.defaultCalendarForNewReminders() else {
            throw AutomationError.executionFailed("No default Reminders list is available")
        }

        return calendar
    }
}

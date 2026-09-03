import Foundation

public enum ActionRisk: String, Codable, Sendable {
    case readOnly
    case reversibleWrite
    case externalWrite
    case destructive
}

public struct ActionDefinition: Codable, Equatable, Sendable {
    public let service: AutomationService
    public let action: String
    public let requiredParameters: [String]
    public let optionalParameters: [String]
    public let risk: ActionRisk

    public init(
        service: AutomationService,
        action: String,
        requiredParameters: [String],
        optionalParameters: [String] = [],
        risk: ActionRisk
    ) {
        self.service = service
        self.action = action
        self.requiredParameters = requiredParameters
        self.optionalParameters = optionalParameters
        self.risk = risk
    }
}

public struct ActionRegistry: Sendable {
    public let definitions: [ActionDefinition]

    public init(definitions: [ActionDefinition] = ActionRegistry.standardDefinitions) {
        self.definitions = definitions
    }

    public func definition(for step: AutomationStep) throws -> ActionDefinition {
        guard let definition = definitions.first(where: {
            $0.service == step.service && $0.action == step.action
        }) else {
            throw AutomationError.unsupportedAction(service: step.service, action: step.action)
        }

        for key in definition.requiredParameters {
            guard step.parameters[key] != nil else {
                throw AutomationError.invalidInput("Missing parameter \(key) for \(step.service.rawValue).\(step.action)")
            }
        }

        let allowed = Set(definition.requiredParameters + definition.optionalParameters)
        let unknown = Set(step.parameters.keys).subtracting(allowed)

        if !unknown.isEmpty {
            throw AutomationError.invalidInput(
                "Unknown parameters for \(step.service.rawValue).\(step.action): \(unknown.sorted().joined(separator: ", "))"
            )
        }

        return definition
    }

    public func plannerCatalog() throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(definitions)
        guard let string = String(data: data, encoding: .utf8) else {
            throw AutomationError.executionFailed("Unable to encode action registry")
        }
        return string
    }

    public static let standardDefinitions: [ActionDefinition] = [
        .init(service: .mail, action: "find", requiredParameters: ["query"], optionalParameters: ["limit"], risk: .readOnly),
        .init(service: .mail, action: "read", requiredParameters: ["localId"], risk: .readOnly),
        .init(service: .mail, action: "draft_reply", requiredParameters: ["localId", "body"], risk: .reversibleWrite),
        .init(service: .mail, action: "send_reply", requiredParameters: ["localId", "body"], risk: .externalWrite),
        .init(service: .mail, action: "move", requiredParameters: ["localId", "mailbox"], risk: .reversibleWrite),
        .init(service: .calendar, action: "find", requiredParameters: ["query"], optionalParameters: ["limit", "from", "to"], risk: .readOnly),
        .init(service: .calendar, action: "create", requiredParameters: ["title", "startAt"], optionalParameters: ["endAt", "durationMinutes", "calendar", "notes", "location"], risk: .reversibleWrite),
        .init(service: .calendar, action: "update", requiredParameters: ["eventId"], optionalParameters: ["title", "startAt", "endAt", "notes", "location"], risk: .reversibleWrite),
        .init(service: .calendar, action: "delete", requiredParameters: ["eventId"], risk: .destructive),
        .init(service: .notes, action: "find", requiredParameters: ["query"], optionalParameters: ["limit"], risk: .readOnly),
        .init(service: .notes, action: "create", requiredParameters: ["title", "body"], risk: .reversibleWrite),
        .init(service: .notes, action: "update", requiredParameters: ["noteId"], optionalParameters: ["title", "body"], risk: .reversibleWrite),
        .init(service: .reminders, action: "find", requiredParameters: ["query"], optionalParameters: ["limit"], risk: .readOnly),
        .init(service: .reminders, action: "create", requiredParameters: ["title"], optionalParameters: ["list", "notes", "dueAt", "priority"], risk: .reversibleWrite),
        .init(service: .reminders, action: "create_list", requiredParameters: ["name"], risk: .reversibleWrite),
        .init(service: .reminders, action: "complete", requiredParameters: ["reminderId"], risk: .reversibleWrite)
    ]
}

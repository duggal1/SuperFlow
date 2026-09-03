import Foundation

public enum AutomationService: String, Codable, CaseIterable, Sendable {
    case mail
    case calendar
    case notes
    case reminders
}

public struct AutomationStep: Codable, Equatable, Sendable {
    public let id: String
    public let service: AutomationService
    public let action: String
    public let parameters: [String: JSONValue]
    public let taskStatus: String?
    public let requiresConfirmation: Bool?

    public init(
        id: String,
        service: AutomationService,
        action: String,
        parameters: [String: JSONValue] = [:],
        taskStatus: String? = nil,
        requiresConfirmation: Bool? = nil
    ) {
        self.id = id
        self.service = service
        self.action = action
        self.parameters = parameters
        self.taskStatus = taskStatus
        self.requiresConfirmation = requiresConfirmation
    }

    private enum CodingKeys: String, CodingKey {
        case id, service, action, parameters, taskStatus, requiresConfirmation
    }
}

public struct AutomationPlan: Codable, Equatable, Sendable {
    public let request: String?
    public let steps: [AutomationStep]

    public init(request: String? = nil, steps: [AutomationStep]) {
        self.request = request
        self.steps = steps
    }
}

public struct StepObservation: Codable, Equatable, Sendable {
    public let stepID: String
    public let service: AutomationService
    public let action: String
    public let output: [String: JSONValue]
    public let taskStatus: String?

    public init(
        stepID: String,
        service: AutomationService,
        action: String,
        output: [String: JSONValue],
        taskStatus: String? = nil
    ) {
        self.stepID = stepID
        self.service = service
        self.action = action
        self.output = output
        self.taskStatus = taskStatus
    }

    private enum CodingKeys: String, CodingKey {
        case stepID, service, action, output, taskStatus
    }
}

public enum PlannerStatus: String, Codable, Sendable {
    case `continue`
    case done
}

public struct PlannerResponse: Codable, Equatable, Sendable {
    public let status: PlannerStatus
    public let steps: [AutomationStep]
    public let finalMessage: String?

    public init(
        status: PlannerStatus,
        steps: [AutomationStep],
        finalMessage: String? = nil
    ) {
        self.status = status
        self.steps = steps
        self.finalMessage = finalMessage
    }
}

public struct ExecutionReport: Codable, Equatable, Sendable {
    public let observations: [StepObservation]
    public let finalMessage: String?

    public init(observations: [StepObservation], finalMessage: String? = nil) {
        self.observations = observations
        self.finalMessage = finalMessage
    }
}

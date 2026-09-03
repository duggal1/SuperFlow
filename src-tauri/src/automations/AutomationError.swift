import Foundation

public enum AutomationError: LocalizedError, Sendable {
    case invalidInput(String)
    case unsupportedAction(service: AutomationService, action: String)
    case executionFailed(String)
    case permissionDenied(String)
    case confirmationDenied(String)
    case plannerFailure(String)
    case exhaustedPlannerRounds(Int)
    case unresolvedReference(String)
    case duplicateStepID(String)
    case missingRuntimeRequirement(String)

    public var errorDescription: String? {
        switch self {
        case .invalidInput(let message):
            return message
        case .unsupportedAction(let service, let action):
            return "Unsupported action \(service.rawValue).\(action)"
        case .executionFailed(let message):
            return message
        case .permissionDenied(let message):
            return message
        case .confirmationDenied(let stepID):
            return "Confirmation denied for step \(stepID)"
        case .plannerFailure(let message):
            return message
        case .exhaustedPlannerRounds(let count):
            return "Planner exceeded maximum rounds: \(count)"
        case .unresolvedReference(let reference):
            return "Unable to resolve reference \(reference)"
        case .duplicateStepID(let stepID):
            return "Duplicate step id \(stepID)"
        case .missingRuntimeRequirement(let requirement):
            return "Missing runtime requirement: \(requirement)"
        }
    }
}

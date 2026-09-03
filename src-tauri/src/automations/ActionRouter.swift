import Foundation

public struct ActionRouter: Sendable {
    private let handlers: [AutomationService: any AutomationServiceHandler]

    public init(handlers: [any AutomationServiceHandler]) {
        self.handlers = Dictionary(uniqueKeysWithValues: handlers.map { ($0.service, $0) })
    }

    public func execute(
        step: AutomationStep,
        parameters: [String: JSONValue]
    ) async throws -> [String: JSONValue] {
        guard let handler = handlers[step.service] else {
            throw AutomationError.executionFailed("No handler registered for \(step.service.rawValue)")
        }

        return try await handler.execute(action: step.action, parameters: parameters)
    }
}

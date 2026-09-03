import Foundation

public protocol ConfirmationProvider: Sendable {
    func confirm(step: AutomationStep, definition: ActionDefinition) async -> Bool
}

public struct AutomaticConfirmationProvider: ConfirmationProvider {
    private let allowedRisks: Set<ActionRisk>

    public init(allowedRisks: Set<ActionRisk> = [.readOnly, .reversibleWrite]) {
        self.allowedRisks = allowedRisks
    }

    public func confirm(step: AutomationStep, definition: ActionDefinition) async -> Bool {
        allowedRisks.contains(definition.risk)
    }
}

public actor CLIConfirmationProvider: ConfirmationProvider {
    public init() {}

    public func confirm(step: AutomationStep, definition: ActionDefinition) async -> Bool {
        if definition.risk == .readOnly {
            return true
        }

        FileHandle.standardError.write(
            Data("Confirm \(step.service.rawValue).\(step.action) for \(step.id)? [y/N] ".utf8)
        )

        guard let line = readLine()?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() else {
            return false
        }

        return line == "y" || line == "yes"
    }
}

public struct ClosureConfirmationProvider: ConfirmationProvider {
    public let handler: @Sendable (AutomationStep, ActionDefinition) async -> Bool

    public init(
        handler: @escaping @Sendable (AutomationStep, ActionDefinition) async -> Bool
    ) {
        self.handler = handler
    }

    public func confirm(step: AutomationStep, definition: ActionDefinition) async -> Bool {
        await handler(step, definition)
    }
}

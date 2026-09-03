import Foundation

public struct PlanExecutor: Sendable {
    private let registry: ActionRegistry
    private let router: ActionRouter
    private let resolver: TemplateResolver
    private let confirmationProvider: any ConfirmationProvider

    public init(
        registry: ActionRegistry = ActionRegistry(),
        router: ActionRouter,
        confirmationProvider: any ConfirmationProvider
    ) {
        self.registry = registry
        self.router = router
        self.resolver = TemplateResolver()
        self.confirmationProvider = confirmationProvider
    }

    public func execute(
        steps: [AutomationStep],
        priorObservations: [StepObservation] = []
    ) async throws -> [StepObservation] {
        var observations = priorObservations
        var knownIDs = Set(priorObservations.map(\.stepID))
        var newObservations: [StepObservation] = []

        for step in steps {
            guard !knownIDs.contains(step.id) else {
                throw AutomationError.duplicateStepID(step.id)
            }

            let resolvedParameters = try resolver.resolve(
                parameters: step.parameters,
                observations: observations
            )

            let resolvedStep = AutomationStep(
                id: step.id,
                service: step.service,
                action: step.action,
                parameters: resolvedParameters,
                taskStatus: step.taskStatus,
                requiresConfirmation: step.requiresConfirmation
            )

            let definition = try registry.definition(for: resolvedStep)
            let mustConfirm = resolvedStep.requiresConfirmation ?? requiresConfirmation(definition.risk)

            if mustConfirm {
                let confirmed = await confirmationProvider.confirm(
                    step: resolvedStep,
                    definition: definition
                )

                guard confirmed else {
                    throw AutomationError.confirmationDenied(resolvedStep.id)
                }
            }

            let output = try await router.execute(
                step: resolvedStep,
                parameters: resolvedParameters
            )

            let observation = StepObservation(
                stepID: resolvedStep.id,
                service: resolvedStep.service,
                action: resolvedStep.action,
                output: output,
                taskStatus: resolvedStep.taskStatus
            )

            observations.append(observation)
            newObservations.append(observation)
            knownIDs.insert(resolvedStep.id)
        }

        return newObservations
    }

    private func requiresConfirmation(_ risk: ActionRisk) -> Bool {
        switch risk {
        case .readOnly:
            return false
        case .reversibleWrite:
            return false
        case .externalWrite, .destructive:
            return true
        }
    }
}

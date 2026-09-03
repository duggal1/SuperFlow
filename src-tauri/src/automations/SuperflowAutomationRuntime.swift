import Foundation

public struct SuperflowAutomationRuntime: Sendable {
    public let registry: ActionRegistry
    public let router: ActionRouter

    public init() {
        let handlers: [any AutomationServiceHandler] = [
            MailAutomation(),
            CalendarAutomation(),
            NotesAutomation(),
            RemindersAutomation()
        ]

        self.registry = ActionRegistry()
        self.router = ActionRouter(handlers: handlers)
    }

    public func executor(
        confirmationProvider: any ConfirmationProvider
    ) -> PlanExecutor {
        PlanExecutor(
            registry: registry,
            router: router,
            confirmationProvider: confirmationProvider
        )
    }

    public func execute(
        plan: AutomationPlan,
        confirmationProvider: any ConfirmationProvider
    ) async throws -> ExecutionReport {
        let executor = executor(confirmationProvider: confirmationProvider)
        let observations = try await executor.execute(steps: plan.steps)
        return ExecutionReport(observations: observations)
    }

    public func runAgent(
        request: String,
        planner: any AutomationPlanner,
        confirmationProvider: any ConfirmationProvider,
        maxRounds: Int = 4
    ) async throws -> ExecutionReport {
        let executor = executor(confirmationProvider: confirmationProvider)
        let agent = AgentOrchestrator(
            planner: planner,
            executor: executor,
            maxRounds: maxRounds
        )
        return try await agent.run(request: request)
    }
}

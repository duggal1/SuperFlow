import Foundation

public struct AgentOrchestrator: Sendable {
    private let planner: any AutomationPlanner
    private let executor: PlanExecutor
    private let reducer: ObservationReducer
    private let maxRounds: Int

    public init(
        planner: any AutomationPlanner,
        executor: PlanExecutor,
        reducer: ObservationReducer = ObservationReducer(),
        maxRounds: Int = 4
    ) {
        self.planner = planner
        self.executor = executor
        self.reducer = reducer
        self.maxRounds = max(1, maxRounds)
    }

    public func run(request: String) async throws -> ExecutionReport {
        var observations: [StepObservation] = []

        for round in 1...maxRounds {
            let plannerInput = reducer.reduce(observations)

            let response = try await planner.next(
                request: request,
                observations: plannerInput,
                round: round
            )

            if response.steps.isEmpty && response.status == .continue {
                throw AutomationError.plannerFailure("Planner returned continue with no steps")
            }

            let executed = try await executor.execute(
                steps: response.steps,
                priorObservations: observations
            )

            observations.append(contentsOf: executed)

            if response.status == .done {
                return ExecutionReport(
                    observations: observations,
                    finalMessage: response.finalMessage
                )
            }
        }

        throw AutomationError.exhaustedPlannerRounds(maxRounds)
    }
}

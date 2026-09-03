import Foundation

public protocol AutomationPlanner: Sendable {
    func next(
        request: String,
        observations: [StepObservation],
        round: Int
    ) async throws -> PlannerResponse
}

public struct StaticPlanner: AutomationPlanner {
    private let response: PlannerResponse

    public init(steps: [AutomationStep]) {
        self.response = PlannerResponse(status: .done, steps: steps)
    }

    public func next(
        request: String,
        observations: [StepObservation],
        round: Int
    ) async throws -> PlannerResponse {
        response
    }
}

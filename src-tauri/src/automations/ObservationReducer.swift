import Foundation

public struct ObservationReducer: Sendable {
    public let maxStringCharacters: Int
    public let maxArrayItems: Int

    public init(
        maxStringCharacters: Int = 12_000,
        maxArrayItems: Int = 20
    ) {
        self.maxStringCharacters = maxStringCharacters
        self.maxArrayItems = maxArrayItems
    }

    public func reduce(_ observations: [StepObservation]) -> [StepObservation] {
        observations.map { observation in
            StepObservation(
                stepID: observation.stepID,
                service: observation.service,
                action: observation.action,
                output: observation.output.mapValues(reduce),
                taskStatus: observation.taskStatus
            )
        }
    }

    private func reduce(_ value: JSONValue) -> JSONValue {
        switch value {
        case .string(let string):
            if string.count <= maxStringCharacters {
                return value
            }
            return .string(String(string.prefix(maxStringCharacters)))
        case .array(let array):
            return .array(Array(array.prefix(maxArrayItems)).map(reduce))
        case .object(let object):
            return .object(object.mapValues(reduce))
        default:
            return value
        }
    }
}

import Foundation

public protocol AutomationServiceHandler: Sendable {
    var service: AutomationService { get }
    func execute(action: String, parameters: [String: JSONValue]) async throws -> [String: JSONValue]
}

public enum ParameterReader {
    public static func string(
        _ key: String,
        from parameters: [String: JSONValue],
        required: Bool = true
    ) throws -> String? {
        if let value = parameters[key]?.stringValue {
            return value
        }

        if required {
            throw AutomationError.invalidInput("Parameter \(key) must be a string")
        }

        return nil
    }

    public static func int(
        _ key: String,
        from parameters: [String: JSONValue],
        default defaultValue: Int
    ) throws -> Int {
        guard let raw = parameters[key] else {
            return defaultValue
        }

        guard let value = raw.intValue else {
            throw AutomationError.invalidInput("Parameter \(key) must be an integer")
        }

        return value
    }
}

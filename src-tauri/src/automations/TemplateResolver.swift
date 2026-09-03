import Foundation

public struct TemplateResolver: Sendable {
    private let regex: NSRegularExpression

    public init() {
        self.regex = try! NSRegularExpression(
            pattern: #"\{\{steps\.([A-Za-z0-9_-]+)\.([A-Za-z0-9_.-]+)\}\}"#,
            options: []
        )
    }

    public func resolve(
        parameters: [String: JSONValue],
        observations: [StepObservation]
    ) throws -> [String: JSONValue] {
        var resolved: [String: JSONValue] = [:]

        for (key, value) in parameters {
            resolved[key] = try resolve(value: value, observations: observations)
        }

        return resolved
    }

    private func resolve(
        value: JSONValue,
        observations: [StepObservation]
    ) throws -> JSONValue {
        switch value {
        case .string(let string):
            return try resolveString(string, observations: observations)
        case .array(let values):
            return .array(try values.map { try resolve(value: $0, observations: observations) })
        case .object(let object):
            var resolved: [String: JSONValue] = [:]
            for (key, nested) in object {
                resolved[key] = try resolve(value: nested, observations: observations)
            }
            return .object(resolved)
        default:
            return value
        }
    }

    private func resolveString(
        _ string: String,
        observations: [StepObservation]
    ) throws -> JSONValue {
        let range = NSRange(string.startIndex..<string.endIndex, in: string)
        let matches = regex.matches(in: string, options: [], range: range)

        guard !matches.isEmpty else {
            return .string(string)
        }

        if matches.count == 1,
           let match = matches.first,
           match.range == range {
            return try value(for: match, in: string, observations: observations)
        }

        var result = string

        for match in matches.reversed() {
            let value = try value(for: match, in: string, observations: observations)
            guard let replacement = value.scalarString() else {
                let token = (string as NSString).substring(with: match.range)
                throw AutomationError.unresolvedReference(token)
            }

            let swiftRange = Range(match.range, in: result)!
            result.replaceSubrange(swiftRange, with: replacement)
        }

        return .string(result)
    }

    private func value(
        for match: NSTextCheckingResult,
        in source: String,
        observations: [StepObservation]
    ) throws -> JSONValue {
        let nsSource = source as NSString
        let stepID = nsSource.substring(with: match.range(at: 1))
        let path = nsSource.substring(with: match.range(at: 2))
        let token = nsSource.substring(with: match.range)

        guard let observation = observations.last(where: { $0.stepID == stepID }) else {
            throw AutomationError.unresolvedReference(token)
        }

        var current: JSONValue = .object(observation.output)

        for component in path.split(separator: ".").map(String.init) {
            switch current {
            case .object(let object):
                guard let next = object[component] else {
                    throw AutomationError.unresolvedReference(token)
                }
                current = next
            case .array(let array):
                guard let index = Int(component), array.indices.contains(index) else {
                    throw AutomationError.unresolvedReference(token)
                }
                current = array[index]
            default:
                throw AutomationError.unresolvedReference(token)
            }
        }

        return current
    }
}

import Foundation

public enum PlanDecoder {
    public static func decode(_ data: Data) throws -> AutomationPlan {
        let decoder = JSONDecoder()

        if let plan = try? decoder.decode(AutomationPlan.self, from: data) {
            return plan
        }

        if let steps = try? decoder.decode([AutomationStep].self, from: data) {
            return AutomationPlan(steps: steps)
        }

        if let step = try? decoder.decode(AutomationStep.self, from: data) {
            return AutomationPlan(steps: [step])
        }

        throw AutomationError.invalidInput("JSON must be an AutomationPlan, an array of AutomationStep, or one AutomationStep")
    }

    public static func decodeString(_ string: String) throws -> AutomationPlan {
        guard let data = stripCodeFence(string).data(using: .utf8) else {
            throw AutomationError.invalidInput("Unable to encode JSON input")
        }
        return try decode(data)
    }

    public static func stripCodeFence(_ string: String) -> String {
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)

        guard trimmed.hasPrefix("```") else {
            return trimmed
        }

        let lines = trimmed.components(separatedBy: .newlines)
        guard lines.count >= 3 else {
            return trimmed
        }

        return lines.dropFirst().dropLast().joined(separator: "\n")
    }
}

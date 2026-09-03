import Foundation

@main
struct SuperflowCLI {
    static func main() async {
        do {
            let arguments = Array(CommandLine.arguments.dropFirst())

            guard let command = arguments.first else {
                throw AutomationError.invalidInput(
                    "Usage: superflow-automation plan <json-file> | agent <request>"
                )
            }

            let runtime = SuperflowAutomationRuntime()
            let confirmation = CLIConfirmationProvider()
            let report: ExecutionReport

            switch command {
            case "plan":
                guard arguments.count >= 2 else {
                    throw AutomationError.invalidInput("plan requires a JSON file path")
                }

                let data = try Data(contentsOf: URL(fileURLWithPath: arguments[1]))
                let plan = try PlanDecoder.decode(data)

                report = try await runtime.execute(
                    plan: plan,
                    confirmationProvider: confirmation
                )

            case "agent":
                guard arguments.count >= 2 else {
                    throw AutomationError.invalidInput("agent requires a natural-language request")
                }

                guard let apiKey = ProcessInfo.processInfo.environment["GEMINI_API_KEY"],
                      !apiKey.isEmpty else {
                    throw AutomationError.invalidInput("GEMINI_API_KEY is required")
                }

                guard let model = ProcessInfo.processInfo.environment["GEMINI_MODEL"],
                      !model.isEmpty else {
                    throw AutomationError.invalidInput("GEMINI_MODEL is required")
                }

                let request = arguments.dropFirst().joined(separator: " ")
                let planner = GeminiPlanner(
                    configuration: GeminiPlannerConfiguration(
                        apiKey: apiKey,
                        model: model
                    )
                )

                report = try await runtime.runAgent(
                    request: request,
                    planner: planner,
                    confirmationProvider: confirmation
                )

            default:
                throw AutomationError.invalidInput("Unknown command \(command)")
            }

            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(report)
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data("\n".utf8))
        } catch {
            let message = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            FileHandle.standardError.write(Data((message + "\n").utf8))
            exit(1)
        }
    }
}

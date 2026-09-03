import Foundation

public struct AppleScriptRunner: Sendable {
    public init() {}

    public func run(_ source: String, arguments: [String] = []) async throws -> String {
        try await Task.detached(priority: .userInitiated) {
            let process = Process()
            let output = Pipe()
            let error = Pipe()

            process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
            process.arguments = ["-e", source, "--"] + arguments
            process.standardOutput = output
            process.standardError = error

            try process.run()

            let outputData = output.fileHandleForReading.readDataToEndOfFile()
            let errorData = error.fileHandleForReading.readDataToEndOfFile()

            process.waitUntilExit()

            let stdout = String(data: outputData, encoding: .utf8) ?? ""
            let stderr = String(data: errorData, encoding: .utf8) ?? ""

            guard process.terminationStatus == 0 else {
                throw AutomationError.executionFailed(
                    stderr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        ? "AppleScript failed with status \(process.terminationStatus)"
                        : stderr.trimmingCharacters(in: .whitespacesAndNewlines)
                )
            }

            return stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        }.value
    }
}

import Foundation

enum ObsidianContentKind: String, Codable {
    case document
    case meeting
    case todo
    case research
}

enum ObsidianWriteMode: String, Codable {
    case upsert
    case append
}

struct ObsidianRoute: Codable {
    let vaultPath: String?
    let branch: String
    let fileKey: String?
    let mode: ObsidianWriteMode?
    let openAfterWrite: Bool?
}

struct ObsidianSection: Codable {
    let heading: String
    let body: String
}

struct ObsidianTask: Codable {
    let title: String
    let owner: String?
    let due: String?
    let priority: String?
    let completed: Bool?
}

struct ObsidianSource: Codable {
    let title: String
    let url: String?
    let author: String?
    let note: String?
}

struct ObsidianContent: Codable {
    let kind: ObsidianContentKind
    let title: String
    let summary: String?
    let date: String?
    let attendees: [String]?
    let agenda: [String]?
    let sections: [ObsidianSection]?
    let decisions: [String]?
    let actionItems: [ObsidianTask]?
    let tasks: [ObsidianTask]?
    let abstract: String?
    let sources: [ObsidianSource]?
}

enum ObsidianAutomationError: LocalizedError {
    case missingVault
    case invalidBranch
    case invalidFileKey
    case invalidContent(String)
    case writeFailed(String)

    var errorDescription: String? {
        switch self {
        case .missingVault:
            return "No Obsidian vault path was provided."
        case .invalidBranch:
            return "Invalid Obsidian branch."
        case .invalidFileKey:
            return "Invalid Obsidian file key."
        case .invalidContent(let message):
            return message
        case .writeFailed(let message):
            return message
        }
    }
}

struct ObsidianRenderer {
    func render(_ content: ObsidianContent) throws -> String {
        try validate(content)

        var output: [String] = []

        output.append("---")
        output.append("type: \(content.kind.rawValue)")
        output.append("title: \"\(escapeYAML(content.title))\"")
        output.append("updated: \(ISO8601DateFormatter().string(from: Date()))")
        output.append("---")
        output.append("")
        output.append("# \(content.title)")
        output.append("")

        if let summary = clean(content.summary) {
            output.append(summary)
            output.append("")
        }

        switch content.kind {
        case .document:
            renderDocument(content, into: &output)

        case .meeting:
            renderMeeting(content, into: &output)

        case .todo:
            renderTodo(content, into: &output)

        case .research:
            renderResearch(content, into: &output)
        }

        return output
            .joined(separator: "\n")
            .trimmingCharacters(in: .whitespacesAndNewlines)
            + "\n"
    }

    private func renderDocument(
        _ content: ObsidianContent,
        into output: inout [String]
    ) {
        renderSections(content.sections, into: &output)
    }

    private func renderMeeting(
        _ content: ObsidianContent,
        into output: inout [String]
    ) {
        if let date = clean(content.date) {
            output.append("**Date:** \(date)")
            output.append("")
        }

        if let attendees = content.attendees, !attendees.isEmpty {
            output.append("## Attendees")
            output.append("")
            attendees.forEach {
                output.append("- \($0)")
            }
            output.append("")
        }

        if let agenda = content.agenda, !agenda.isEmpty {
            output.append("## Agenda")
            output.append("")
            agenda.forEach {
                output.append("- \($0)")
            }
            output.append("")
        }

        renderSections(content.sections, into: &output)

        if let decisions = content.decisions, !decisions.isEmpty {
            output.append("## Decisions")
            output.append("")
            decisions.forEach {
                output.append("- \($0)")
            }
            output.append("")
        }

        if let actions = content.actionItems, !actions.isEmpty {
            output.append("## Action Items")
            output.append("")
            renderTasks(actions, into: &output)
        }
    }

    private func renderTodo(
        _ content: ObsidianContent,
        into output: inout [String]
    ) {
        guard let tasks = content.tasks else {
            return
        }

        renderTasks(tasks, into: &output)
    }

    private func renderResearch(
        _ content: ObsidianContent,
        into output: inout [String]
    ) {
        if let abstract = clean(content.abstract) {
            output.append("## Abstract")
            output.append("")
            output.append(abstract)
            output.append("")
        }

        renderSections(content.sections, into: &output)

        if let sources = content.sources, !sources.isEmpty {
            output.append("## Sources")
            output.append("")

            for source in sources {
                var line = "- \(source.title)"

                if let author = clean(source.author) {
                    line += " — \(author)"
                }

                if let url = clean(source.url) {
                    line += " — \(url)"
                }

                output.append(line)

                if let note = clean(source.note) {
                    output.append("  - \(note)")
                }
            }

            output.append("")
        }
    }

    private func renderSections(
        _ sections: [ObsidianSection]?,
        into output: inout [String]
    ) {
        guard let sections else {
            return
        }

        for section in sections {
            output.append("## \(section.heading)")
            output.append("")
            output.append(section.body)
            output.append("")
        }
    }

    private func renderTasks(
        _ tasks: [ObsidianTask],
        into output: inout [String]
    ) {
        for task in tasks {
            let completed = task.completed ?? false
            var line = "- [\(completed ? "x" : " ")] \(task.title)"

            var metadata: [String] = []

            if let owner = clean(task.owner) {
                metadata.append("Owner: \(owner)")
            }

            if let due = clean(task.due) {
                metadata.append("Due: \(due)")
            }

            if let priority = clean(task.priority) {
                metadata.append("Priority: \(priority)")
            }

            if !metadata.isEmpty {
                line += " — " + metadata.joined(separator: " · ")
            }

            output.append(line)
        }

        output.append("")
    }

    private func validate(_ content: ObsidianContent) throws {
        guard !content.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw ObsidianAutomationError.invalidContent(
                "Content title cannot be empty."
            )
        }

        switch content.kind {
        case .document:
            guard !(content.sections ?? []).isEmpty else {
                throw ObsidianAutomationError.invalidContent(
                    "Document requires at least one section."
                )
            }

        case .meeting:
            guard content.date != nil ||
                  content.sections != nil ||
                  content.actionItems != nil else {
                throw ObsidianAutomationError.invalidContent(
                    "Meeting requires meeting content."
                )
            }

        case .todo:
            guard !(content.tasks ?? []).isEmpty else {
                throw ObsidianAutomationError.invalidContent(
                    "Todo requires at least one task."
                )
            }

        case .research:
            guard content.abstract != nil ||
                  content.sections != nil ||
                  content.sources != nil else {
                throw ObsidianAutomationError.invalidContent(
                    "Research document requires research content."
                )
            }
        }
    }

    private func clean(_ value: String?) -> String? {
        guard let value else {
            return nil
        }

        let cleaned = value.trimmingCharacters(
            in: .whitespacesAndNewlines
        )

        return cleaned.isEmpty ? nil : cleaned
    }

    private func escapeYAML(_ value: String) -> String {
        value
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
    }
}

struct ObsidianWriter {
    private let renderer = ObsidianRenderer()

    func execute(
        route: ObsidianRoute,
        content: ObsidianContent
    ) throws -> URL {
        let vault = try resolveVault(route)
        let branch = try sanitizeComponent(route.branch)

        let workspace = vault.appendingPathComponent(
            branch,
            isDirectory: true
        )

        try FileManager.default.createDirectory(
            at: workspace,
            withIntermediateDirectories: true
        )

        let rawFileKey = route.fileKey ?? content.title
        let fileKey = try slug(rawFileKey)

        let fileURL = workspace
            .appendingPathComponent(fileKey)
            .appendingPathExtension("md")

        let markdown = try renderer.render(content)

        switch route.mode ?? .upsert {
        case .upsert:
            try atomicWrite(
                markdown,
                to: fileURL
            )

        case .append:
            try append(
                markdown,
                to: fileURL
            )
        }

        if route.openAfterWrite ?? false {
            openInObsidian(
                vault: vault,
                file: fileURL
            )
        }

        return fileURL
    }

    private func resolveVault(
        _ route: ObsidianRoute
    ) throws -> URL {
        if let path = route.vaultPath,
           !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return URL(
                fileURLWithPath: NSString(string: path)
                    .expandingTildeInPath,
                isDirectory: true
            )
        }

        if let path = ProcessInfo.processInfo.environment[
            "OBSIDIAN_VAULT_PATH"
        ],
           !path.isEmpty {
            return URL(
                fileURLWithPath: NSString(string: path)
                    .expandingTildeInPath,
                isDirectory: true
            )
        }

        if let detected = Self.detectVaultFromObsidianConfig() {
            return URL(
                fileURLWithPath: detected,
                isDirectory: true
            )
        }

        throw ObsidianAutomationError.missingVault
    }

    /// Reads the user's registered vaults from Obsidian's own config
    /// (`~/Library/Application Support/obsidian/obsidian.json`) and picks the
    /// currently-open one, falling back to the most recently opened.
    static func detectVaultFromObsidianConfig() -> String? {
        let configURL = FileManager.default
            .homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/obsidian/obsidian.json")

        guard let data = try? Data(contentsOf: configURL),
              let parsed = try? JSONSerialization.jsonObject(with: data),
              let vaults = parsed as? [String: Any],
              let entries = vaults["vaults"] as? [String: Any] else {
            return nil
        }

        var best: (open: Bool, ts: Double, path: String)?
        for value in entries.values {
            guard let entry = value as? [String: Any],
                  let path = entry["path"] as? String else {
                continue
            }
            let open = entry["open"] as? Bool ?? false
            let ts = entry["ts"] as? Double ?? 0
            let candidate = (open, ts, path)
            guard let current = best else {
                best = candidate
                continue
            }
            if open != current.open ? open : ts > current.ts {
                best = candidate
            }
        }

        return best?.path
    }

    private func sanitizeComponent(
        _ value: String
    ) throws -> String {
        let trimmed = value.trimmingCharacters(
            in: .whitespacesAndNewlines
        )

        guard !trimmed.isEmpty,
              trimmed != ".",
              trimmed != "..",
              !trimmed.contains("/"),
              !trimmed.contains("\\") else {
            throw ObsidianAutomationError.invalidBranch
        }

        return trimmed
    }

    private func slug(
        _ value: String
    ) throws -> String {
        let lowered = value
            .lowercased()
            .trimmingCharacters(in: .whitespacesAndNewlines)

        let allowed = CharacterSet.alphanumerics
            .union(CharacterSet(charactersIn: "-_"))

        let pieces = lowered
            .replacingOccurrences(of: " ", with: "-")
            .unicodeScalars
            .map { scalar -> String in
                allowed.contains(scalar)
                    ? String(scalar)
                    : "-"
            }

        var result = pieces.joined()

        while result.contains("--") {
            result = result.replacingOccurrences(
                of: "--",
                with: "-"
            )
        }

        result = result.trimmingCharacters(
            in: CharacterSet(charactersIn: "-")
        )

        guard !result.isEmpty else {
            throw ObsidianAutomationError.invalidFileKey
        }

        return result
    }

    private func atomicWrite(
        _ content: String,
        to url: URL
    ) throws {
        do {
            try content.write(
                to: url,
                atomically: true,
                encoding: .utf8
            )
        } catch {
            throw ObsidianAutomationError.writeFailed(
                error.localizedDescription
            )
        }
    }

    private func append(
        _ content: String,
        to url: URL
    ) throws {
        if !FileManager.default.fileExists(atPath: url.path) {
            try atomicWrite(content, to: url)
            return
        }

        guard let data = (
            "\n\n" + content
        ).data(using: .utf8) else {
            throw ObsidianAutomationError.writeFailed(
                "Unable to encode Markdown."
            )
        }

        do {
            let handle = try FileHandle(
                forWritingTo: url
            )

            try handle.seekToEnd()
            try handle.write(contentsOf: data)
            try handle.close()
        } catch {
            throw ObsidianAutomationError.writeFailed(
                error.localizedDescription
            )
        }
    }

    private func openInObsidian(
        vault: URL,
        file: URL
    ) {
        let vaultName = vault.lastPathComponent

        let relative = file.path
            .replacingOccurrences(
                of: vault.path + "/",
                with: ""
            )

        var components = URLComponents()
        components.scheme = "obsidian"
        components.host = "open"
        components.queryItems = [
            URLQueryItem(
                name: "vault",
                value: vaultName
            ),
            URLQueryItem(
                name: "file",
                value: relative
            )
        ]

        guard let url = components.url else {
            return
        }

        let process = Process()
        process.executableURL = URL(
            fileURLWithPath: "/usr/bin/open"
        )
        process.arguments = [
            url.absoluteString
        ]

        try? process.run()
    }
}

@main
struct ObsidianAutomationCLI {
    static func main() {
        do {
            let arguments = Array(
                CommandLine.arguments.dropFirst()
            )

            guard arguments.count == 2 else {
                throw ObsidianAutomationError.invalidContent(
                    "Usage: ObsidianAutomation route.json content.json"
                )
            }

            let routeData = try Data(
                contentsOf: URL(
                    fileURLWithPath: arguments[0]
                )
            )

            let contentData = try Data(
                contentsOf: URL(
                    fileURLWithPath: arguments[1]
                )
            )

            let decoder = JSONDecoder()

            let route = try decoder.decode(
                ObsidianRoute.self,
                from: routeData
            )

            let content = try decoder.decode(
                ObsidianContent.self,
                from: contentData
            )

            let result = try ObsidianWriter().execute(
                route: route,
                content: content
            )

            print(
                """
                {
                  "success": true,
                  "path": "\(result.path)"
                }
                """
            )
        } catch {
            let message = (
                error as? LocalizedError
            )?.errorDescription
                ?? error.localizedDescription

            FileHandle.standardError.write(
                Data((message + "\n").utf8)
            )

            exit(1)
        }
    }
}





// EXAMPLES 


// {
//   "kind": "meeting",
//   "title": "Q4 Security Architecture Review",
//   "date": "2026-09-04T10:00:00+05:30",
//   "attendees": [
//     "Maya Chen",
//     "Harshit Duggal"
//   ],
//   "agenda": [
//     "Review architecture changes",
//     "Resolve remaining security concerns",
//     "Agree on production rollout"
//   ],
//   "sections": [
//     {
//       "heading": "Context",
//       "body": "Review of the revised production architecture following the Q4 security assessment."
//     }
//   ],
//   "decisions": [
//     "Move authentication boundary behind the gateway.",
//     "Complete threat-model review before production rollout."
//   ],
//   "actionItems": [
//     {
//       "title": "Finish revised architecture document",
//       "owner": "Harshit Duggal",
//       "due": "2026-09-03T16:00:00+05:30",
//       "priority": "high",
//       "completed": false
//     }
//   ]
// }
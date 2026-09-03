import Foundation

public struct NotesAutomation: AutomationServiceHandler {
    public let service: AutomationService = .notes
    private let runner: AppleScriptRunner

    public init(runner: AppleScriptRunner = AppleScriptRunner()) {
        self.runner = runner
    }

    public func execute(
        action: String,
        parameters: [String: JSONValue]
    ) async throws -> [String: JSONValue] {
        switch action {
        case "find":
            return try await find(parameters)
        case "create":
            return try await create(parameters)
        case "update":
            return try await update(parameters)
        default:
            throw AutomationError.unsupportedAction(service: .notes, action: action)
        }
    }

    private func find(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let query = try ParameterReader.string("query", from: parameters)!
        let limit = min(max(try ParameterReader.int("limit", from: parameters, default: 10), 1), 50)

        let script = #"""
        on cleanValue(v)
            set s to v as text
            set AppleScript's text item delimiters to tab
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to return
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to linefeed
            set parts to text items of s
            set AppleScript's text item delimiters to " "
            set s to parts as text
            set AppleScript's text item delimiters to ""
            return s
        end cleanValue

        on run argv
            set searchText to item 1 of argv
            set maxResults to (item 2 of argv) as integer
            set resultRows to {}
            tell application "Notes"
                repeat with a in accounts
                    repeat with f in folders of a
                        repeat with n in notes of f
                            set noteName to ""
                            set noteBody to ""
                            try
                                set noteName to name of n as text
                            end try
                            try
                                set noteBody to body of n as text
                            end try
                            set isMatch to false
                            ignoring case
                                if noteName contains searchText or noteBody contains searchText then
                                    set isMatch to true
                                end if
                            end ignoring
                            if isMatch then
                                set noteID to id of n as text
                                set end of resultRows to my cleanValue(noteID) & tab & my cleanValue(noteName)
                                if (count of resultRows) ≥ maxResults then exit repeat
                            end if
                        end repeat
                        if (count of resultRows) ≥ maxResults then exit repeat
                    end repeat
                    if (count of resultRows) ≥ maxResults then exit repeat
                end repeat
            end tell
            set AppleScript's text item delimiters to linefeed
            set outputText to resultRows as text
            set AppleScript's text item delimiters to ""
            return outputText
        end run
        """#

        let output = try await runner.run(script, arguments: [query, String(limit)])
        let rows = output.isEmpty ? [] : output.components(separatedBy: .newlines)

        let matches: [JSONValue] = rows.compactMap { row in
            let fields = row.components(separatedBy: "\t")
            guard fields.count >= 2 else {
                return nil
            }

            return .object([
                "noteId": .string(fields[0]),
                "title": .string(fields[1])
            ])
        }

        return [
            "query": .string(query),
            "count": .int(matches.count),
            "matches": .array(matches)
        ]
    }

    private func create(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let title = try ParameterReader.string("title", from: parameters)!
        let body = try ParameterReader.string("body", from: parameters)!

        let script = #"""
        on run argv
            set noteTitle to item 1 of argv
            set noteBody to item 2 of argv
            tell application "Notes"
                set targetAccount to default account
                tell targetAccount
                    set newNote to make new note at folder "Notes" with properties {name:noteTitle, body:noteBody}
                    return id of newNote as text
                end tell
            end tell
        end run
        """#

        let noteID = try await runner.run(script, arguments: [title, body])

        return [
            "noteId": .string(noteID),
            "title": .string(title),
            "created": .bool(true)
        ]
    }

    private func update(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let noteID = try ParameterReader.string("noteId", from: parameters)!
        let title = try ParameterReader.string("title", from: parameters, required: false) ?? ""
        let body = try ParameterReader.string("body", from: parameters, required: false) ?? ""

        guard !title.isEmpty || !body.isEmpty else {
            throw AutomationError.invalidInput("notes.update requires title or body")
        }

        let script = #"""
        on run argv
            set targetID to item 1 of argv
            set newTitle to item 2 of argv
            set newBody to item 3 of argv
            tell application "Notes"
                repeat with a in accounts
                    repeat with f in folders of a
                        set matches to every note of f whose id is targetID
                        if (count of matches) > 0 then
                            set n to item 1 of matches
                            if newTitle is not "" then set name of n to newTitle
                            if newBody is not "" then set body of n to newBody
                            return id of n as text
                        end if
                    end repeat
                end repeat
            end tell
            error "Note not found"
        end run
        """#

        let updatedID = try await runner.run(script, arguments: [noteID, title, body])

        return [
            "noteId": .string(updatedID),
            "updated": .bool(true)
        ]
    }
}

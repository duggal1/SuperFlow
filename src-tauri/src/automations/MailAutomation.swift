import Foundation

public struct MailAutomation: AutomationServiceHandler {
    public let service: AutomationService = .mail
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
        case "read":
            return try await read(parameters)
        case "draft_reply":
            return try await reply(parameters, send: false)
        case "send_reply":
            return try await reply(parameters, send: true)
        case "move":
            return try await move(parameters)
        default:
            throw AutomationError.unsupportedAction(service: .mail, action: action)
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
            tell application "Mail"
                set inboxMessages to messages of inbox
                set scanLimit to count of inboxMessages
                if scanLimit > 750 then set scanLimit to 750
                repeat with i from 1 to scanLimit
                    set m to item i of inboxMessages
                    set messageSubject to ""
                    set messageSender to ""
                    try
                        set messageSubject to subject of m as text
                    end try
                    try
                        set messageSender to sender of m as text
                    end try
                    set isMatch to false
                    ignoring case
                        if messageSubject contains searchText or messageSender contains searchText then
                            set isMatch to true
                        end if
                    end ignoring
                    if isMatch then
                        set localID to id of m as text
                        set internetID to ""
                        set receivedAt to ""
                        try
                            set internetID to message id of m as text
                        end try
                        try
                            set receivedAt to date received of m as text
                        end try
                        set end of resultRows to my cleanValue(localID) & tab & my cleanValue(internetID) & tab & my cleanValue(messageSubject) & tab & my cleanValue(messageSender) & tab & my cleanValue(receivedAt)
                        if (count of resultRows) ≥ maxResults then exit repeat
                    end if
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
            guard fields.count >= 5 else {
                return nil
            }

            return .object([
                "localId": .string(fields[0]),
                "messageId": .string(fields[1]),
                "subject": .string(fields[2]),
                "sender": .string(fields[3]),
                "dateReceived": .string(fields[4])
            ])
        }

        return [
            "query": .string(query),
            "count": .int(matches.count),
            "matches": .array(matches)
        ]
    }

    private func read(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let localID = try ParameterReader.string("localId", from: parameters)!

        let script = #"""
        on run argv
            set targetID to (item 1 of argv) as integer
            tell application "Mail"
                set foundMessages to every message of inbox whose id is targetID
                if (count of foundMessages) is 0 then error "Mail message not found in Inbox"
                set m to item 1 of foundMessages
                set messageSubject to subject of m as text
                set messageSender to sender of m as text
                set messageContent to content of m as text
                return messageSubject & linefeed & messageSender & linefeed & messageContent
            end tell
        end run
        """#

        let output = try await runner.run(script, arguments: [localID])
        let lines = output.components(separatedBy: .newlines)

        guard lines.count >= 2 else {
            throw AutomationError.executionFailed("Mail returned incomplete message data")
        }

        return [
            "localId": .string(localID),
            "subject": .string(lines[0]),
            "sender": .string(lines[1]),
            "content": .string(lines.dropFirst(2).joined(separator: "\n"))
        ]
    }

    private func reply(
        _ parameters: [String: JSONValue],
        send: Bool
    ) async throws -> [String: JSONValue] {
        let localID = try ParameterReader.string("localId", from: parameters)!
        let body = try ParameterReader.string("body", from: parameters)!

        let script = #"""
        on run argv
            set targetID to (item 1 of argv) as integer
            set replyBody to item 2 of argv
            set shouldSend to item 3 of argv
            tell application "Mail"
                set foundMessages to every message of inbox whose id is targetID
                if (count of foundMessages) is 0 then error "Mail message not found in Inbox"
                set originalMessage to item 1 of foundMessages
                set replyMessage to reply originalMessage opening window false
                tell replyMessage
                    set content to replyBody & return & return & content
                    save
                end tell
                if shouldSend is "true" then
                    send replyMessage
                end if
                return id of replyMessage as text
            end tell
        end run
        """#

        let draftID = try await runner.run(
            script,
            arguments: [localID, body, send ? "true" : "false"]
        )

        return [
            "localId": .string(localID),
            "replyId": .string(draftID),
            "sent": .bool(send)
        ]
    }

    private func move(_ parameters: [String: JSONValue]) async throws -> [String: JSONValue] {
        let localID = try ParameterReader.string("localId", from: parameters)!
        let mailboxName = try ParameterReader.string("mailbox", from: parameters)!

        let script = #"""
        on run argv
            set targetID to (item 1 of argv) as integer
            set targetMailboxName to item 2 of argv
            tell application "Mail"
                set foundMessages to every message of inbox whose id is targetID
                if (count of foundMessages) is 0 then error "Mail message not found in Inbox"
                set originalMessage to item 1 of foundMessages
                set targetMailbox to missing value
                repeat with a in accounts
                    try
                        set matches to every mailbox of a whose name is targetMailboxName
                        if (count of matches) > 0 then
                            set targetMailbox to item 1 of matches
                            exit repeat
                        end if
                    end try
                end repeat
                if targetMailbox is missing value then error "Mailbox not found"
                move originalMessage to targetMailbox
                return targetMailboxName
            end tell
        end run
        """#

        let movedTo = try await runner.run(script, arguments: [localID, mailboxName])

        return [
            "localId": .string(localID),
            "mailbox": .string(movedTo),
            "moved": .bool(true)
        ]
    }
}

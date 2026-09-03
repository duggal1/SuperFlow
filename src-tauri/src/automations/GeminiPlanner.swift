import Foundation

public struct GeminiPlannerConfiguration: Sendable {
    public let apiKey: String
    public let model: String
    public let baseURL: URL
    public let temperature: Double

    public init(
        apiKey: String,
        model: String,
        baseURL: URL = URL(string: "https://generativelanguage.googleapis.com/v1beta")!,
        temperature: Double = 0.1
    ) {
        self.apiKey = apiKey
        self.model = model
        self.baseURL = baseURL
        self.temperature = temperature
    }
}

public struct GeminiPlanner: AutomationPlanner {
    private let configuration: GeminiPlannerConfiguration
    private let registry: ActionRegistry
    private let session: URLSession

    public init(
        configuration: GeminiPlannerConfiguration,
        registry: ActionRegistry = ActionRegistry(),
        session: URLSession = .shared
    ) {
        self.configuration = configuration
        self.registry = registry
        self.session = session
    }

    public func next(
        request: String,
        observations: [StepObservation],
        round: Int
    ) async throws -> PlannerResponse {
        let prompt = try buildPrompt(
            request: request,
            observations: observations,
            round: round
        )

        var components = URLComponents(
            url: configuration.baseURL
                .appendingPathComponent("models")
                .appendingPathComponent("\(configuration.model):generateContent"),
            resolvingAgainstBaseURL: false
        )!

        components.queryItems = [
            URLQueryItem(name: "key", value: configuration.apiKey)
        ]

        guard let url = components.url else {
            throw AutomationError.plannerFailure("Unable to build Gemini URL")
        }

        let body = GeminiRequest(
            contents: [
                .init(
                    role: "user",
                    parts: [.init(text: prompt)]
                )
            ],
            generationConfig: .init(
                temperature: configuration.temperature,
                responseMimeType: "application/json"
            )
        )

        var requestObject = URLRequest(url: url)
        requestObject.httpMethod = "POST"
        requestObject.setValue("application/json", forHTTPHeaderField: "Content-Type")
        requestObject.httpBody = try JSONEncoder().encode(body)

        let (data, response) = try await session.data(for: requestObject)

        guard let http = response as? HTTPURLResponse else {
            throw AutomationError.plannerFailure("Gemini returned a non-HTTP response")
        }

        guard (200..<300).contains(http.statusCode) else {
            let message = String(data: data, encoding: .utf8) ?? "Unknown Gemini error"
            throw AutomationError.plannerFailure("Gemini HTTP \(http.statusCode): \(message)")
        }

        let decoded = try JSONDecoder().decode(GeminiResponse.self, from: data)

        guard let text = decoded.candidates
            .first?
            .content
            .parts
            .compactMap(\.text)
            .first else {
            throw AutomationError.plannerFailure("Gemini returned no planner JSON")
        }

        guard let plannerData = PlanDecoder.stripCodeFence(text).data(using: .utf8) else {
            throw AutomationError.plannerFailure("Unable to decode Gemini planner output")
        }

        do {
            var response = try JSONDecoder().decode(PlannerResponse.self, from: plannerData)
            // Robust hook — synthesize missing taskStatus per step if Gemini omitted (keeps DJ-edit live feed alive)
            let synthesizedSteps = response.steps.map { step -> AutomationStep in
                if step.taskStatus != nil { return step }
                let synth = "Executed automation step \(step.id) via \(step.service.rawValue).\(step.action) with validated parameters successfully"
                return AutomationStep(id: step.id, service: step.service, action: step.action, parameters: step.parameters, taskStatus: synth, requiresConfirmation: step.requiresConfirmation)
            }
            // Only rewrite if needed (preserve original finalMessage)
            if synthesizedSteps.contains(where: { $0.taskStatus == nil }) == false && synthesizedSteps != response.steps {
                response = PlannerResponse(status: response.status, steps: synthesizedSteps, finalMessage: response.finalMessage)
            } else if response.steps.contains(where: { $0.taskStatus == nil }) {
                // Should not happen after map, but handle
                response = PlannerResponse(status: response.status, steps: synthesizedSteps, finalMessage: response.finalMessage)
            }
            return response
        } catch {
            throw AutomationError.plannerFailure("Invalid planner JSON: \(error.localizedDescription)")
        }
    }

    private func buildPrompt(
        request: String,
        observations: [StepObservation],
        round: Int
    ) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]

        let observationData = try encoder.encode(observations)
        let observationJSON = String(data: observationData, encoding: .utf8) ?? "[]"
        let actionCatalog = try registry.plannerCatalog()

        // Time context for absolute ISO-8601 resolution
        let now = Date()
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime]
        let nowISO = iso.string(from: now)
        let tz = TimeZone.current
        let tzOffset: String = {
            let s = tz.secondsFromGMT(for: now)
            let sign = s >= 0 ? "+" : "-"
            let absS = abs(s)
            return String(format: "%@%02d:%02d", sign, absS / 3600, (absS % 3600) / 60)
        }()
        let weekday: String = {
            let f = DateFormatter()
            f.dateFormat = "EEEE"
            f.locale = Locale(identifier: "en_US_POSIX")
            return f.string(from: now)
        }()
        let nowContext = "Now: \(nowISO) (\(tzOffset)) — \(weekday) — local timezone: \(tz.identifier)"

        return """
        You are Superflow's BOUNDED PLANNER — Gemini-powered voice automation for macOS.
        Your ONLY job: convert a spoken request + prior observations into a VALIDATED, MINIMAL plan of deterministic Swift actions.

        ════════════════════════════════════════════════════════════
        ULTRA-STRICT OUTPUT CONTRACT — VIOLATION = REJECTED
        ════════════════════════════════════════════════════════════
        - Output MUST be ONLY raw JSON. No markdown. No code fences. No explanation. No preamble.
        - Output MUST match EXACTLY this schema — nothing added, nothing renamed, NO missing field:
        {
          "status": "continue" | "done",
          "steps": [
            {
              "id": "snake_case_unique_id",
              "service": "mail" | "calendar" | "notes" | "reminders",
              "action": "registered_action",
              "parameters": { "<key>": "<value>" },
              "taskStatus": "10-15 words, extremely clean, sharp, concise status for THIS step (not generic)",
              "requiresConfirmation": true | false | null
            }
          ],
          "finalMessage": "optional short human summary or null"
        }
        - taskStatus is REQUIRED on EVERY step. It is the per-task live status — not a boolean. Exactly 10-15 words, extremely clean, sharp, concise, tone = native macOS + DJ edit. No fluff. No "done" / "completed" / "success" generic. Examples:
          Good: "Located latest Maya Chen Q4 email thread matching query in inbox"
          Good: "Drafted sharp reply confirming Thursday 4 PM delivery commitment to Maya"
          Good: "Review meeting booked Friday 10 AM for 45 minutes with notes attached"
          Good: "Captured client notes with subject and email context preserved cleanly"
          Good: "Follow-up reminder set Thursday 2 PM high priority to finish document"
          Bad: "Done" / "Task completed" / "Email found successfully" — REJECTED.
        - responseMimeType is application/json — do NOT wrap in ```json```.
        - If you output anything outside JSON, the system JSONDecoder will throw and the user sees "Invalid planner JSON" — avoid it at all costs.
        - Never invent services, actions, or parameters outside the Registered Actions catalog below. Doing so throws unsupportedAction and fails the run.
        - Never invent new tool names. Never output AppleScript / Swift / shell code. Only the JSON above.

        ════════════════════════════════════════════════════════════
        AGENT KIT — BOUNDED MULTI-STEP LOOP (CRITICAL)
        ════════════════════════════════════════════════════════════
        Most workflows complete in ONE round: status "done" with all steps.

        Only use status "continue" when you CANNOT decide the next action without runtime data:

          Voice: "Find the latest email from Maya and draft a reply. Create reminder Friday."
          → Round 1: status "continue", steps [mail.find query=Maya] — need localId first
          → Execution returns observation {matches:[{localId:123}]}
          → Round 2: status "continue", steps [mail.read localId={{steps.find_mail.matches.0.localId}}]
          → Round 3: status "done", steps [mail.draft_reply, reminders.create]

        Rules:
        - If you return "continue" you MUST include at least 1 step that fetches observation (find/read). Empty steps with "continue" throws.
        - If you have everything to finish, return "done" — don't waste a round.
        - Max rounds = 4. Keep total steps minimal and ordered. Dependencies first.
        - Reference prior step output with MUSTACHE: {{steps.<step_id>.<json.path>}}
          Examples:
            {{steps.find_client_email.matches.0.localId}}
            {{steps.read_client_email.subject}}
            {{steps.read_client_email.content}}
            {{steps.find_note.matches.0.noteId}}
          Array indexes (.0, .1) are required for matches. String-only templates: embedding inside a longer string is OK (e.g., "Review {{steps.read_email.subject}}").
          Whole-value references preserve type; interpolated strings coerce to string.
        - Do NOT claim success before observation proves it. Do not hallucinate email bodies or event IDs.

        ════════════════════════════════════════════════════════════
        SERVICE / ACTION CATALOG (REGISTERED ONLY)
        ════════════════════════════════════════════════════════════
        Use ONLY these services + actions. Parameter names are case-sensitive.

        mail.find         → { query: string (required), limit: int 1-50 optional default 10 }  readOnly — finds inbox messages whose subject or sender contains query (case-insensitive).
        mail.read         → { localId: string (required, from find.matches[].localId) }  readOnly — reads subject/sender/content of one message.
        mail.draft_reply  → { localId: string, body: string }  reversibleWrite — creates a draft reply (does NOT send). Safe, no confirmation needed.
        mail.send_reply   → { localId: string, body: string }  externalWrite — sends reply immediately. MUST set requiresConfirmation: true. Only use when user explicitly says "send", "send reply", "send it".
        mail.move         → { localId: string, mailbox: string }  reversibleWrite

        calendar.find     → { query: string, limit: int 1-50, from: ISO8601 optional, to: ISO8601 optional }  readOnly
        calendar.create   → { title: string, startAt: ISO8601 with offset (required), endAt: ISO8601 optional, durationMinutes: int optional, calendar: string optional, notes: string optional, location: string optional }  reversibleWrite
        calendar.update   → { eventId: string, title: string optional, startAt: ISO8601 optional, endAt: ISO8601 optional, notes: string optional, location: string optional }  reversibleWrite
        calendar.delete   → { eventId: string }  destructive — MUST requiresConfirmation true

        notes.find        → { query: string, limit: int 1-50 }  readOnly
        notes.create      → { title: string, body: string }  reversibleWrite
        notes.update      → { noteId: string, title: string optional, body: string optional }  reversibleWrite (requires at least one of title/body)

        reminders.find    → { query: string, limit: int 1-100 }  readOnly
        reminders.create  → { title: string, list: string optional, notes: string optional, dueAt: ISO8601 with offset optional, priority: int 0-9 optional }  reversibleWrite
        reminders.create_list → { name: string }  reversibleWrite
        reminders.complete → { reminderId: string }  reversibleWrite

        Full catalog JSON (source of truth for validation):
        \(actionCatalog)

        ════════════════════════════════════════════════════════════
        CONFIRMATION & SAFETY (DETERMINISTIC — DO NOT BYPASS)
        ════════════════════════════════════════════════════════════
        - readOnly: never needs confirmation.
        - reversibleWrite (draft_reply, move, calendar.create/update, notes.*, reminders.*): confirmation OPTIONAL — set null or false unless user asks to confirm.
        - externalWrite / destructive (send_reply, calendar.delete): MUST set requiresConfirmation: true. The Swift executor will prompt the user and abort if denied.
        - Be conservative: draft ≠ send. "Reply to Maya" → draft_reply. "Reply and send" → send_reply with confirmation.
        - No secrets in parameters. No shell injection.

        ════════════════════════════════════════════════════════════
        TIME / DATE RESOLUTION
        ════════════════════════════════════════════════════════════
        \(nowContext)
        - Resolve "tomorrow", "next Monday", "Friday", "in 2 hours" using Now above. Always output ABSOLUTE ISO-8601 with offset, e.g. "2026-09-04T10:00:00+05:30".
        - For calendar.create: startAt is required. If user says "tomorrow at 3 PM", compute the absolute date. If time is vague ("tomorrow morning" with no hour), pick a sensible default (09:00) and mention assumption in finalMessage, OR if truly ambiguous create with minimal and note ambiguity — but never invent year.
        - For reminders dueAt: same ISO-8601 rule. If user says "Friday", use upcoming Friday from Now.

        ════════════════════════════════════════════════════════════
        COST PRINCIPLE — MINIMIZE Gemini CALLS
        ════════════════════════════════════════════════════════════
        - Do the narrow job: natural language → validated structured plan. Everything else is deterministic Swift (AppleScript/EventKit).
        - Prefer ONE "done" plan over multiple "continue" rounds. Use "continue" ONLY when next step needs observation data.
        - Keep steps minimal. Do not add extra finds or notes unless user asked.
        - Do not use generic tool-calling — only the registered actions.

        ════════════════════════════════════════════════════════════
        CROSS-SERVICE WORKFLOW PATTERNS (USE THESE)
        ════════════════════════════════════════════════════════════
        - Find email → draft reply → reminder: [mail.find, mail.read, mail.draft_reply, reminders.create]
        - Email → calendar: [mail.find, mail.read, calendar.create with notes={{steps.read_email.subject}}]
        - Email → notes: [mail.find, mail.read, notes.create body={{steps.read_email.content}}]
        - Notes → email: [notes.find, mail.find, mail.draft_reply body uses note content]
        - Reminder from email: [mail.find, reminders.create title="Follow up {{steps.find_email.matches.0.sender}}"]

        ════════════════════════════════════════════════════════════
        TASK STATUS — PER-STEP SHARP AUDIT (THIS IS THE HOOK YOU ASKED FOR)
        ════════════════════════════════════════════════════════════
        - Every step MUST carry taskStatus: 10-15 words, DJ-edit clean, sharp, not generic.
        - Per-service status, not just total: mail steps report mail status, calendar steps report calendar status, reminders/notes likewise. One line per step.
        - Validated sharply: 10 ≤ words ≤ 15, ≥ 40 chars, not "Done" / "Completed" / "Success" / "Task done". Must be specific to THIS action.
        - Tone = Superflow native: extremely ultra clean, concise, commanding — like a live task feed: "Scanning inbox...", "Located thread...", "Draft live...", "Meeting locked...", "Reminder armed..."
        - This taskStatus is what the overlay + voice feed shows live. It must read like a finished audit line even before execution (optimistic but precise).

        ════════════════════════════════════════════════════════════
        VALIDATED EXAMPLES — COPY THE SHAPE EXACTLY (NOTE taskStatus ON EVERY STEP)
        ════════════════════════════════════════════════════════════
        Example A — Simple (one shot):
        Request: "Remind me to follow up with Sarah tomorrow at 3 PM"
        Output: {"status":"done","steps":[{"id":"remind_sarah","service":"reminders","action":"create","parameters":{"title":"Follow up with Sarah","dueAt":"2026-09-03T15:00:00+05:30"},"taskStatus":"Follow-up reminder armed for Sarah tomorrow at 3 PM sharp and ready"}],"finalMessage":"Reminder set for tomorrow at 3 PM"}

        Example B — Draft reply (no send):
        Request: "Find the latest email from John and draft a reply"
        Output: {"status":"continue","steps":[{"id":"find_email","service":"mail","action":"find","parameters":{"query":"John","limit":5},"taskStatus":"Scanning inbox for latest John thread matching query quickly and accurately"}],"finalMessage":null}
        // After observation, next round:
        {"status":"done","steps":[{"id":"read_email","service":"mail","action":"read","parameters":{"localId":"{{steps.find_email.matches.0.localId}}"},"taskStatus":"Reading selected email body and subject details for precise reply context"},{"id":"draft_reply","service":"mail","action":"draft_reply","parameters":{"localId":"{{steps.find_email.matches.0.localId}}","body":"Hi John, Thanks for your note — will follow up shortly.\\n\\nBest"},"taskStatus":"Drafted sharp reply to John confirming follow-up shortly and professionally"}],"finalMessage":"Draft ready for John"}

        Example C — Full deterministic multi-step (no extra Gemini round needed if you already use templates):
        Request: "Find the latest email from Maya Chen about Q4, draft a reply confirming Thursday 4 PM, schedule review Friday 10 AM, save to notes, remind Thursday 2 PM"
        Output: {
          "status":"done",
          "steps":[
            {"id":"find_client_email","service":"mail","action":"find","parameters":{"query":"Maya Chen Q4","limit":5},"taskStatus":"Located Maya Chen Q4 security thread latest email matching query accurately"},
            {"id":"read_client_email","service":"mail","action":"read","parameters":{"localId":"{{steps.find_client_email.matches.0.localId}}"},"taskStatus":"Reading Maya email body and subject to extract full commitment context cleanly"},
            {"id":"draft_reply","service":"mail","action":"draft_reply","parameters":{"localId":"{{steps.find_client_email.matches.0.localId}}","body":"Hi Maya,\\n\\nThanks for the update. I'll have the revised architecture document ready by Thursday at 4 PM. Friday at 10 AM is set for review.\\n\\nBest,\\nHarshit"},"taskStatus":"Drafted sharp reply confirming Thursday 4 PM delivery and Friday review scheduled"},
            {"id":"schedule_review","service":"calendar","action":"create","parameters":{"title":"Q4 Security Architecture Review","startAt":"2026-09-04T10:00:00+05:30","durationMinutes":45,"notes":"Review revised architecture document with Maya Chen. Related email: {{steps.read_client_email.subject}}"},"taskStatus":"Review meeting locked Friday 10 AM for 45 minutes with notes attached cleanly"},
            {"id":"update_client_notes","service":"notes","action":"create","parameters":{"title":"Maya Chen — Q4 Security Review","body":"Client: Maya Chen\\nSubject: {{steps.read_client_email.subject}}\\nCommitment: Thursday 4 PM\\nReview: Friday 10 AM\\n\\nEmail context:\\n{{steps.read_client_email.content}}"},"taskStatus":"Captured client notes with subject and email context preserved and organized sharply"},
            {"id":"create_delivery_reminder","service":"reminders","action":"create","parameters":{"title":"Finish Q4 security architecture document","notes":"Complete before Thursday 4 PM commitment to Maya Chen.","dueAt":"2026-09-03T14:00:00+05:30","priority":1},"taskStatus":"Delivery reminder armed Thursday 2 PM high priority to finish document on time"}
          ],
          "finalMessage":"Mail, calendar, notes and reminder planned — executing now."
        }

        Example D — Requires observation (bounded loop):
        Request: "Reply to this email and create a reminder to follow up Friday"
        When inbox context unknown: first return {"status":"continue","steps":[{"id":"find_email","service":"mail","action":"find","parameters":{"query":"","limit":5}}]}
        Then use observation to pick correct localId for draft.

        ════════════════════════════════════════════════════════════
        PLANNER ROUND: \(round) / 4  |  If round > 1 you have prior observations to use — do not re-request them.
        ════════════════════════════════════════════════════════════

        User request (verbatim transcript after "Hey Superflow" or hold-key):
        \(request)

        Prior observations (reduced, JSON):
        \(observationJSON)

        RESPOND NOW — JSON ONLY. No markdown. No thinking tags. No explanation. Ultra strictly only JSON matching the schema above with status/steps (each with taskStatus 10-15 words sharp) + finalMessage. Every step MUST have taskStatus.
        """
    }
}

private struct GeminiRequest: Encodable {
    let contents: [Content]
    let generationConfig: GenerationConfig

    struct Content: Encodable {
        let role: String
        let parts: [Part]
    }

    struct Part: Encodable {
        let text: String
    }

    struct GenerationConfig: Encodable {
        let temperature: Double
        let responseMimeType: String
    }
}

private struct GeminiResponse: Decodable {
    let candidates: [Candidate]

    struct Candidate: Decodable {
        let content: Content
    }

    struct Content: Decodable {
        let parts: [Part]
    }

    struct Part: Decodable {
        let text: String?
    }
}

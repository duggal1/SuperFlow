import ApplicationServices
import Foundation

private let maxNodes = 8_192
private let maxCharacters = 24_000

private func attribute(_ element: AXUIElement, _ name: CFString) -> AnyObject? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name, &value) == .success else {
        return nil
    }
    return value
}

private func stringAttribute(_ element: AXUIElement, _ name: CFString) -> String? {
    attribute(element, name) as? String
}

private func children(_ element: AXUIElement) -> [AXUIElement] {
    attribute(element, kAXChildrenAttribute as CFString) as? [AXUIElement] ?? []
}

private func elementAttribute(_ element: AXUIElement, _ name: CFString) -> AXUIElement? {
    guard let value = attribute(element, name),
          CFGetTypeID(value) == AXUIElementGetTypeID() else {
        return nil
    }
    return unsafeBitCast(value, to: AXUIElement.self)
}

private func normalized(_ value: String) -> String {
    value
        .replacingOccurrences(of: "\r\n", with: "\n")
        .split(whereSeparator: { $0.isWhitespace })
        .joined(separator: " ")
        .trimmingCharacters(in: .whitespacesAndNewlines)
}

private func activeWindow(_ application: AXUIElement) -> AXUIElement? {
    elementAttribute(application, kAXFocusedWindowAttribute as CFString)
        ?? elementAttribute(application, kAXMainWindowAttribute as CFString)
        ?? ((attribute(application, kAXWindowsAttribute as CFString) as? [AXUIElement])?.first)
}

private func capturePageText(pid: pid_t) -> String? {
    let application = AXUIElementCreateApplication(pid)
    guard let window = activeWindow(application) else { return nil }

    // Aggressive: capture everything visible in the frontmost window's web content.
    // Previous filter missed Gmail body when it was inside deeply nested AXGroups
    // without explicit AXValue on the container. Now we collect any text-bearing
    // attribute from any role, dedupe only exact duplicates, and go deeper.
    var stack: [(AXUIElement, Int)] = [(window, 0)]
    var seen = Set<String>()
    var blocks: [String] = []
    var characters = 0
    var visited = 0
    let maxNodesAggressive = 16_384
    let maxCharsAggressive = 48_000
    let maxDepth = 36

    while let (element, depth) = stack.popLast(), visited < maxNodesAggressive, characters < maxCharsAggressive {
        visited += 1
        // Try every text-bearing attribute on every element — don't filter by role.
        // Gmail's sender chip, subject, and body are often AXStaticText / AXGroup / AXWebArea
        // with AXValue, AXTitle, AXDescription, or AXPlaceholderValue.
        for name in [kAXValueAttribute, kAXTitleAttribute, kAXDescriptionAttribute, kAXPlaceholderValueAttribute] {
            guard let raw = stringAttribute(element, name as CFString) else { continue }
            let text = normalized(raw)
            guard text.count >= 2, text.count <= 2000, seen.insert(text).inserted else { continue }
            // Keep email-relevant blocks even if they look like UI chrome; the Rust side will
            // truncate to 24k and Gemini will ignore chrome. Better to send too much than miss the body.
            let remaining = maxCharsAggressive - characters
            let clipped = String(text.prefix(remaining))
            blocks.append(clipped)
            characters += clipped.count + 1
            if characters >= maxCharsAggressive { break }
        }
        // Also capture the element's own description via AXHelp if not already
        if depth < maxDepth {
            let kids = children(element)
            // Reverse so we visit in DOM order
            for child in kids.reversed() {
                stack.append((child, depth + 1))
            }
            // Fallback: if an element has no children but is a text leaf, its value was already
            // captured above. No extra handling needed.
        }
    }

    // Join with blank line between logical blocks to preserve sender/subject/body separation
    // for Gemini. The Rust side (`ai_cleanup/mod.rs`) already formats this as
    // `Visible page content:\n{page_context}` — keeping newlines helps the model see boundaries.
    let result = blocks.joined(separator: "\n\n")
    return result.isEmpty ? nil : result
}

@_cdecl("superflow_capture_page_context")
public func superflowCapturePageContext(_ pid: Int32) -> UnsafeMutablePointer<CChar>? {
    guard let text = capturePageText(pid: pid_t(pid)) else { return nil }
    return strdup(text)
}

@_cdecl("superflow_free_page_context")
public func superflowFreePageContext(_ pointer: UnsafeMutablePointer<CChar>?) {
    free(pointer)
}

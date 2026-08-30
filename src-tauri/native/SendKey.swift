import ApplicationServices
import Carbon.HIToolbox
import Foundation

// SendKey mapping must match Rust SendKey enum in send_it.rs
// 0 = Enter, 1 = CommandEnter, 2 = ControlEnter
@_cdecl("superflow_send_key")
public func superflowSendKey(_ key: Int32) -> Bool {
    let keyCode = CGKeyCode(kVK_Return)
    var flags: CGEventFlags = []

    switch key {
    case 0: // Enter
        flags = []
    case 1: // CommandEnter — Gmail/Outlook macOS
        flags = .maskCommand
    case 2: // ControlEnter — Gmail/Outlook Windows/Linux
        flags = .maskControl
    default:
        return false
    }

    guard let eventDown = CGEvent(keyboardEventSource: nil, virtualKey: keyCode, keyDown: true) else {
        return false
    }
    guard let eventUp = CGEvent(keyboardEventSource: nil, virtualKey: keyCode, keyDown: false) else {
        return false
    }

    if !flags.isEmpty {
        eventDown.flags = flags
        eventUp.flags = flags
    }

    // No arbitrary sleep — paste has already completed in Rust before this is called.
    // Post immediately; CGEvent is synchronous.
    eventDown.post(tap: CGEventTapLocation.cgSessionEventTap)
    eventUp.post(tap: CGEventTapLocation.cgSessionEventTap)
    return true
}

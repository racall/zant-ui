// macOS UI scenario runner: drives a Zant window with real mouse and
// keyboard events from a step list and screenshots it at chosen moments
// (the macOS counterpart of scripts/win/ui-auto.ps1; docs/development.md).
//
//   swiftc -O scripts/mac/ui-scenario.swift -o /tmp/ui-scenario
//   ZANT_LAYOUT_FILE=/tmp/layout.txt ./target/debug/zant-tsx-counter &
//   CONTENT_HEIGHT=480 /tmp/ui-scenario zant-tsx-counter /tmp/layout.txt /tmp/shots steps.txt
//
// Needs Accessibility (events, resize) and Screen Recording (screenshots)
// permission for the terminal. CONTENT_HEIGHT is the window's content
// height in points (the title bar is the rest of the frame).
// usage: ui-scenario <process name> <layout file> <out dir> <steps file>
// steps (one per line, # comments):
//   click <key> [times]    real click(s) at the control's centre
//   hover <key>            move the pointer over a control
//   away                   move the pointer outside the window
//   type <text>            type text (unicode key events)
//   key <name>             return | backspace | tab | escape | arrows | letters
//   cmdkey <name>          key with Command (e.g. a / c)
//   clickfrac <key> <fx> <fy> / drag <key> <fx> <fy> (drag from centre)
//   dragspan <key> <fx0> <fy0> <fx1> <fy1> / doubleclick <key>
//   scroll <key> <lines>    eight wheel events (positive wheel delta = up)
//   clickat <x> <y> / clickpoint <x> <y> / scrollat <x> <y> <ticks> (positive = down)
//   scrollpixels <key> <pixels>  one pixel wheel event (positive = down)
//   reveal <key>           scroll the page until the keyed box is visible
//   present <key> / absent <key> / anykey <key> ... (layout assertions)
//   aligned <popup> <trigger>  assert bottomLeft/topLeft placement with 4pt gap
//   clipboardclear / clipboard <text>  clear / assert clipboard contains text
//   expect <text>          wait <=1.5s for a new complete EVENT_LOG line containing text
//   noevent <ms>           fail on new [event] lines since the preceding check
//   rss <label>            print app RSS in KiB (APP_PID)
//   repeat <n> ... end     repeat steps (no nesting)
//   layout <name> / dumpax / resizefast <w> <h> [times]
// EVENT_LOG is append-only; expect/noevent consume all complete lines seen.
// DRY_RUN=1 validates and prints expanded steps, without GUI or file writes.
//   resize <w> <h>         resize the window (content size, via Accessibility)
//   wait <ms>
//   shot <name>            screenshot the window to <out dir>/<name>.png
// Exit: 0 success, 1 failed checks/actions, 2 malformed steps/arguments.
import AppKit
import CoreGraphics
import Darwin
import Foundation

let args = CommandLine.arguments
guard args.count == 5 else { print("usage: scenario <process> <layout file> <out dir> <steps file>"); exit(2) }
let owner = args[1], layoutPath = args[2], outDir = args[3]
guard let steps = try? String(contentsOfFile: args[4], encoding: .utf8) else {
    print("FAIL unreadable steps file \(args[4])"); exit(2)
}
let env = ProcessInfo.processInfo.environment

// Parse before creating event sources, finding windows or accessing the clipboard.
func parseSteps(_ text: String) throws -> [String] {
    struct Invalid: Error { let message: String }
    var expanded: [String] = [], body: [String]? = nil, repetitions = 0
    let specs: [String: (Int, Int)] = [
        "click": (1, 2), "hover": (1, 1), "away": (0, 0), "type": (1, Int.max),
        "key": (1, 1), "cmdkey": (1, 1), "dumpax": (0, 0), "clickpoint": (2, 2),
        "resizefast": (2, 3), "resize": (2, 2), "clickfrac": (3, 3), "scroll": (2, 2),
        "scrollpixels": (2, 2), "drag": (3, 3), "clickat": (2, 2), "scrollat": (3, 3), "layout": (1, 1),
        "wait": (1, 1), "shot": (1, 1), "expect": (1, Int.max), "noevent": (1, 1),
        "rss": (1, 1), "reveal": (1, 1), "present": (1, 1), "absent": (1, 1),
        "anykey": (1, Int.max), "aligned": (2, 2), "clipboardclear": (0, 0),
        "clipboard": (1, Int.max), "dragspan": (5, 5), "doubleclick": (1, 1)
    ]
    for (index, raw) in text.components(separatedBy: "\n").enumerated() {
        // Keep trailing whitespace: `expect [event] input ` checks an empty input.
        let line = String(raw.drop(while: { $0 == " " || $0 == "\t" })).replacingOccurrences(of: "\r", with: "")
        let fields = line.split(whereSeparator: { $0 == " " || $0 == "\t" }).map(String.init)
        if fields.isEmpty || line.hasPrefix("#") { continue }
        let command = fields[0], a = Array(fields.dropFirst())
        func invalid(_ reason: String) -> Invalid { Invalid(message: "line \(index + 1): \(reason): \(line)") }
        if command == "repeat" {
            guard body == nil, a.count == 1, let n = Int(a[0]), n >= 0 else { throw invalid("invalid/nested repeat") }
            repetitions = n; body = []; continue
        }
        if command == "end" {
            guard a.isEmpty, let block = body else { throw invalid("unmatched end") }
            for _ in 0..<repetitions { expanded += block }
            body = nil; continue
        }
        guard let spec = specs[command], a.count >= spec.0, a.count <= spec.1 else { throw invalid("unknown step or wrong arguments") }
        let numericStart: Int?
        switch command {
        case "wait", "noevent", "resize", "resizefast", "clickat", "clickpoint", "scrollat": numericStart = 0
        case "clickfrac", "drag", "dragspan", "scroll", "scrollpixels": numericStart = 1
        default: numericStart = nil
        }
        if let start = numericStart {
            guard a.dropFirst(start).allSatisfy({ Double($0)?.isFinite == true }) else { throw invalid("invalid number") }
        }
        if ["wait", "noevent"].contains(command), Double(a[0])! < 0 { throw invalid("negative duration") }
        if command == "click", a.count == 2, (Int(a[1]) ?? 0) < 1 { throw invalid("invalid click count") }
        if ["scroll", "scrollpixels"].contains(command), Int32(a[1]) == nil { throw invalid("invalid wheel delta") }
        if body != nil { body!.append(line) } else { expanded.append(line) }
    }
    guard body == nil else { throw Invalid(message: "unterminated repeat") }
    return expanded
}
let expandedSteps: [String]
do { expandedSteps = try parseSteps(steps) }
catch { print("FAIL steps: \(error)"); exit(2) }
if env["DRY_RUN"] == "1" {
    for line in expandedSteps { print(line) }
    print("dry run ok: \(expandedSteps.count) expanded steps")
    exit(0)
}
try FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
var failures = 0
func check(_ ok: Bool, _ label: String) {
    print("\(ok ? "PASS" : "FAIL") \(label)")
    if !ok { failures += 1 }
}
func pause(_ ms: Double) { Thread.sleep(forTimeInterval: ms / 1000) }
var eventPosition = 0
func logLines() -> [String]? {
    guard let path = env["EVENT_LOG"], let data = try? Data(contentsOf: URL(fileURLWithPath: path)) else { return nil }
    // Ignore a writer's incomplete trailing line until its newline arrives.
    guard let end = data.lastIndex(of: 10) else { return [] }
    let lines = String(decoding: data[...end], as: UTF8.self).components(separatedBy: "\n")
    return Array(lines.dropLast())
}

struct Win { let id: CGWindowID; let frame: CGRect; let pid: pid_t }

func window() -> Win? {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] else { return nil }
    for w in list where (w[kCGWindowOwnerName as String] as? String) == owner && (w[kCGWindowLayer as String] as? Int) == 0 {
        if let pid = env["APP_PID"], (w[kCGWindowOwnerPID as String] as? Int) != Int(pid) { continue }
        if let b = w[kCGWindowBounds as String] as? [String: Double], let id = w[kCGWindowNumber as String] as? Int,
           let pid = w[kCGWindowOwnerPID as String] as? Int {
            return Win(id: CGWindowID(id), frame: CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!), pid: pid_t(pid))
        }
    }
    return nil
}

func layout() -> [String: CGRect] {
    // the app rewrites the file every frame: retry a torn read
    for _ in 0..<5 {
        guard let text = try? String(contentsOfFile: layoutPath, encoding: .utf8), text.hasSuffix("\n") else { usleep(5_000); continue }
        var out: [String: CGRect] = [:]
        for line in text.split(separator: "\n") {
            let p = line.split(separator: "\t")
            if p.count == 5, let x = Double(p[1]), let y = Double(p[2]), let w = Double(p[3]), let h = Double(p[4]) {
                out[String(p[0])] = CGRect(x: x, y: y, width: w, height: h)
            }
        }
        if !out.isEmpty { return out }
    }
    return [:]
}

/// Content origin on screen: the window frame minus the title bar, measured
/// from the content view's size the app reports through Accessibility.
var contentHeight: Double = 0
func origin(_ w: Win) -> CGPoint { CGPoint(x: w.frame.minX, y: w.frame.maxY - contentHeight) }

let src = CGEventSource(stateID: .hidSystemState)
/// Where the scenario last put the pointer: wheel events are placed there
/// (the system's cursor position updates asynchronously after a move).
var lastPointer: CGPoint?
func mouse(_ type: CGEventType, _ p: CGPoint) {
    lastPointer = p
    sendPointer(CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: p, mouseButton: .left))
}
func point(of key: String) -> CGPoint? {
    guard let w = window(), let b = layout()[key] else { return nil }
    let o = origin(w)
    return CGPoint(x: o.x + b.midX, y: o.y + b.midY)
}
/// Every input event is posted to the app under test only (`postToPid`),
/// never to the system event stream: keys there go to whichever app is
/// frontmost and clicks to whatever window covers the point, and a test
/// tool must never type or click into the user's other apps.
var targetPid: pid_t = 0
func sendKeyEvent(_ event: CGEvent?) {
    guard targetPid > 0, let event else { return }
    event.postToPid(targetPid)
}
/// Mouse and wheel events: AppKit routes them by screen position, so they go
/// through the system stream, but only when the topmost window under the
/// point belongs to the app under test (otherwise the scenario stops).
func sendPointer(_ event: CGEvent?) {
    guard targetPid > 0, let event else { return }
    if event.type == .scrollWheel, let p = lastPointer {
        event.location = p
    }
    // a bare move only hovers (`away` parks the pointer outside the window)
    if event.type == .mouseMoved {
        event.post(tap: .cghidEventTap)
        return
    }
    let p = event.location
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
        as? [[String: Any]] ?? []
    for info in list {
        guard (info[kCGWindowLayer as String] as? Int) == 0,
              let bounds = info[kCGWindowBounds as String] as? [String: CGFloat],
              let owner = info[kCGWindowOwnerPID as String] as? pid_t else { continue }
        let frame = CGRect(x: bounds["X"] ?? 0, y: bounds["Y"] ?? 0,
                           width: bounds["Width"] ?? 0, height: bounds["Height"] ?? 0)
        if !frame.contains(p) { continue }
        if owner == targetPid {
            event.post(tap: .cghidEventTap)
            return
        }
        print("FAIL another app's window covers (\(Int(p.x)), \(Int(p.y))); stopped before sending input")
        exit(2)
    }
    print("FAIL no window of the app under test at (\(Int(p.x)), \(Int(p.y))); stopped before sending input")
    exit(2)
}
func typeText(_ text: String) {
    for ch in text {
        let units = Array(String(ch).utf16)
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: down)
            e?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            sendKeyEvent(e)
        }
        usleep(25_000)
    }
}
func pressKey(_ name: String, flags: CGEventFlags = []) {
    let codes: [String: CGKeyCode] = [
        "return": 36, "backspace": 51, "tab": 48, "escape": 53, "space": 49,
        "left": 123, "right": 124, "down": 125, "up": 126,
        "home": 115, "end": 119, "pageup": 116, "pagedown": 121, "forwarddelete": 117,
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9,
        "b": 11, "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "1": 18, "2": 19,
        "3": 20, "4": 21, "6": 22, "5": 23, "9": 25, "7": 26, "8": 28, "0": 29,
        "o": 31, "u": 32, "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46,
        "equal": 24, "minus": 27, "plus": 24,
    ]
    guard let code = codes[name] else { check(false, "unknown key \(name)"); return }
    let down = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: true)
    let up = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: false)
    if !flags.isEmpty { down?.flags = flags; up?.flags = flags }
    sendKeyEvent(down)
    sendKeyEvent(up)
}
func resize(_ w: Win, _ width: Double, _ height: Double) {
    let app = AXUIElementCreateApplication(w.pid)
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value) == .success,
          let wins = value as? [AXUIElement], let ax = wins.first else { print("resize: no AX window"); return }
    var size = CGSize(width: width, height: height + (w.frame.height - contentHeight))
    if let v = AXValueCreate(.cgSize, &size) { AXUIElementSetAttributeValue(ax, kAXSizeAttribute as CFString, v) }
    contentHeight = height
}

/// The static texts / values of the window (AX): the readable state of an app
/// whose layout has no keyed controls (generated TSX apps).
func axDump(pid: pid_t) {
    let app = AXUIElementCreateApplication(pid)
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value) == .success,
          let windows = value as? [AXUIElement], let window = windows.first
    else { print("ax: no window"); return }

    func attribute(_ element: AXUIElement, _ name: String) -> String? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
        if let text = value as? String { return text }
        return nil
    }

    var visited = 0
    func walk(_ element: AXUIElement, _ depth: Int) {
        if visited > 400 || depth > 40 { return }
        visited += 1
        let role = attribute(element, kAXRoleAttribute) ?? "?"
        let title = attribute(element, kAXTitleAttribute) ?? ""
        let desc = attribute(element, kAXDescriptionAttribute) ?? ""
        let text = attribute(element, kAXValueAttribute) ?? ""
        if !title.isEmpty || !text.isEmpty || !desc.isEmpty {
            print("ax \(role) title=\(title) value=\(text) desc=\(desc)")
        }
        var children: CFTypeRef?
        if AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &children) == .success,
           let list = children as? [AXUIElement] {
            for child in list { walk(child, depth + 1) }
        }
    }
    walk(window, 0)
}

// wait for the window and its first layout
var win: Win? = nil
for _ in 0..<100 {
    if let w = window() { if !layout().isEmpty { win = w; break } else if win == nil { win = w } }
    usleep(100_000)
}
guard let first = win else { check(false, "window of \(owner) not found"); exit(1) }
// content height: the root's height in the layout file is the content size
contentHeight = Double(ProcessInfo.processInfo.environment["CONTENT_HEIGHT"] ?? "") ?? first.frame.height - 28
targetPid = first.pid
// a background process cannot activate another app (macOS 14+); raising the
// window through Accessibility puts it above the other windows, and
// `sendPointer` checks before every click that nothing covers it
NSRunningApplication(processIdentifier: first.pid)?.activate()
do {
    let app = AXUIElementCreateApplication(first.pid)
    var value: CFTypeRef?
    if AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value) == .success,
       let windows = value as? [AXUIElement], let main = windows.first {
        AXUIElementSetAttributeValue(app, kAXFrontmostAttribute as CFString, kCFBooleanTrue)
        AXUIElementPerformAction(main, kAXRaiseAction as CFString)
    }
}
usleep(600_000)

for line in expandedSteps {
    let parts = line.split(maxSplits: 1, whereSeparator: { $0 == " " || $0 == "\t" }).map(String.init)
    let cmd = parts[0], argument = parts.count > 1 ? parts[1] : ""
    let rest = ["type", "expect", "clipboard"].contains(cmd)
        ? argument : argument.trimmingCharacters(in: .whitespaces)
    switch cmd {
    case "expect", "noevent":
        let deadline = ProcessInfo.processInfo.systemUptime + (cmd == "expect" ? 1.5 : Double(rest)! / 1000)
        var lines: [String]? = nil
        var matched = false
        repeat {
            lines = logLines()
            if let all = lines {
                let new = all.dropFirst(eventPosition)
                matched = cmd == "expect" ? new.contains(where: { $0.contains(rest) }) : new.contains(where: { $0.contains("[event]") })
            }
            if matched && cmd == "expect" { break }
            pause(20)
        } while ProcessInfo.processInfo.systemUptime < deadline
        if let all = lines { eventPosition = all.count }
        check(lines != nil && (cmd == "expect" ? matched : !matched), cmd == "expect" ? rest : "noevent \(rest)ms")
    case "rss":
        if let pid = env["APP_PID"], let value = Int32(pid), value > 0 {
            let task = Process(), pipe = Pipe()
            task.executableURL = URL(fileURLWithPath: "/bin/ps")
            task.arguments = ["-o", "rss=", "-p", pid]; task.standardOutput = pipe
            do {
                try task.run()
                let data = pipe.fileHandleForReading.readDataToEndOfFile(); task.waitUntilExit()
                let value = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                if task.terminationStatus == 0, let kb = Int(value), kb > 0 { print("RSS \(rest) \(kb)") }
                else { check(false, "rss \(rest)") }
            } catch { check(false, "rss \(rest): \(error)") }
        } else { check(false, "rss requires APP_PID") }
    case "present", "absent", "anykey":
        let boxes = layout(), keys = rest.split(separator: " ").map(String.init)
        let found = keys.contains { boxes[$0] != nil }
        check(cmd == "absent" ? !found : found, "\(cmd) \(rest)")
    case "aligned":
        let keys = rest.split(separator: " ").map(String.init), boxes = layout()
        if let popup = boxes[keys[0]], let button = boxes[keys[1]] {
            check(abs(popup.minX - button.minX) < 1 &&
                  (abs(popup.minY - button.maxY - 4) < 1 || abs(popup.maxY - button.minY + 4) < 1), "aligned \(rest)")
        } else { check(false, "aligned \(rest): missing key") }
    case "clipboardclear": NSPasteboard.general.clearContents()
    case "clipboard": check(NSPasteboard.general.string(forType: .string)?.contains(rest) == true, "clipboard \(rest)")
    case "reveal":
        var visible = false
        for _ in 0..<80 {
            guard let w = window(), let box = layout()[rest] else { break }
            if box.minY >= 4 && box.maxY <= contentHeight - 4 { visible = true; break }
            let o = origin(w)
            mouse(.mouseMoved, CGPoint(x: o.x + 8, y: o.y + contentHeight / 2))
            sendPointer(CGEvent(scrollWheelEvent2Source: src, units: .pixel, wheelCount: 1,
                    wheel1: box.minY < 4 ? 60 : -60, wheel2: 0, wheel3: 0))
            pause(60)
        }
        check(visible, "reveal \(rest)")
    case "doubleclick":
        if let p = point(of: rest) {
            mouse(.mouseMoved, p)
            for count in 1...2 {
                for kind in [CGEventType.leftMouseDown, .leftMouseUp] {
                    let event = CGEvent(mouseEventSource: src, mouseType: kind, mouseCursorPosition: p, mouseButton: .left)
                    event?.setIntegerValueField(.mouseEventClickState, value: Int64(count))
                    sendPointer(event); pause(30)
                }
            }
            pause(120)
        } else { check(false, "doubleclick missing key \(rest)") }
    case "click":
        let a = rest.split(separator: " ").map(String.init)
        let times = a.count > 1 ? Int(a[1]) ?? 1 : 1
        for _ in 0..<times {
            guard let p = point(of: a[0]) else { check(false, "missing key \(a[0])"); break }
            mouse(.mouseMoved, p); mouse(.leftMouseDown, p); usleep(10_000); mouse(.leftMouseUp, p)
            usleep(60_000)
        }
    case "hover":
        if let p = point(of: rest) { mouse(.mouseMoved, p) } else { check(false, "missing key \(rest)") }
        usleep(400_000) // let the hover transition finish
    case "away":
        if let w = window() { mouse(.mouseMoved, CGPoint(x: w.frame.maxX + 60, y: w.frame.maxY + 60)) }
        usleep(400_000)
    case "type": typeText(rest)
    case "key": pressKey(rest)
    case "cmdkey": pressKey(rest, flags: .maskCommand)
    case "dumpax":
        if let w = window() { axDump(pid: w.pid) } else { check(false, "ax: no window") }
    case "clickpoint":
        // logical content coordinates (origin top-left of the window content)
        let a = rest.split(separator: " ").compactMap { Double($0) }
        guard a.count >= 2, let w = window() else { check(false, "bad clickpoint \(line)"); break }
        let o = origin(w)
        let p = CGPoint(x: o.x + a[0], y: o.y + a[1])
        mouse(.mouseMoved, p); mouse(.leftMouseDown, p); usleep(10_000); mouse(.leftMouseUp, p)
        usleep(120_000)
    case "resizefast":
        // resize repeatedly with no settle wait: continuous-resize frame test
        let a = rest.split(separator: " ").compactMap { Double($0) }
        if a.count >= 2, let w = window() {
            for _ in 0..<Int(a.count >= 3 ? a[2] : 10) {
                resize(w, a[0], a[1])
                usleep(16_000)
            }
        } else { check(false, "bad resizefast \(line)") }
        usleep(300_000)
    case "resize":
        let a = rest.split(separator: " ").compactMap { Double($0) }
        if let w = window(), a.count == 2 { resize(w, a[0], a[1]) }
        usleep(500_000)
    case "clickfrac":
        // click a fraction of a keyed control's box (popup rows, track knobs)
        let a = rest.split(separator: " ").map(String.init)
        guard a.count == 3, let fx = Double(a[1]), let fy = Double(a[2]) else { check(false, "bad clickfrac \(line)"); continue }
        if let w = window(), let b = layout()[a[0]] {
            let o = origin(w)
            let p = CGPoint(x: o.x + b.minX + b.width * fx, y: o.y + b.minY + b.height * fy)
            mouse(.mouseMoved, p); mouse(.leftMouseDown, p); usleep(10_000); mouse(.leftMouseUp, p)
        } else { check(false, "missing key \(a[0])") }
        usleep(80_000)
    case "scrollpixels":
        let a = rest.split(separator: " ").map(String.init)
        if let p = point(of: a[0]), let delta = Int32(a[1]) {
            mouse(.mouseMoved, p)
            sendPointer(CGEvent(scrollWheelEvent2Source: src, units: .pixel, wheelCount: 1,
                    wheel1: -delta, wheel2: 0, wheel3: 0))
            pause(300)
        } else { check(false, "scrollpixels missing key \(a[0])") }
    case "scroll":
        // scroll the wheel over a keyed control (positive wheel delta scrolls up)
        let a = rest.split(separator: " ").map(String.init)
        guard a.count == 2, let lines = Int32(a[1]) else { check(false, "bad scroll \(line)"); continue }
        if let w = window(), let b = layout()[a[0]] {
            let o = origin(w)
            mouse(.mouseMoved, CGPoint(x: o.x + b.midX, y: o.y + b.midY))
            for _ in 0..<8 {
                sendPointer(CGEvent(scrollWheelEvent2Source: src, units: .line, wheelCount: 1, wheel1: lines, wheel2: 0, wheel3: 0))
                usleep(20_000)
            }
        } else { check(false, "missing key \(a[0])") }
        usleep(300_000)
    case "drag", "dragspan":
        // drag from the control's centre to a fraction of its box
        let a = rest.split(separator: " ").map(String.init)
        guard let fx = Double(a[cmd == "drag" ? 1 : 3]), let fy = Double(a[cmd == "drag" ? 2 : 4]) else { check(false, "bad drag \(line)"); continue }
        if let w = window(), let b = layout()[a[0]] {
            let o = origin(w)
            let start = CGPoint(x: o.x + b.minX + b.width * (cmd == "drag" ? 0.5 : Double(a[1])!),
                                y: o.y + b.minY + b.height * (cmd == "drag" ? 0.5 : Double(a[2])!))
            let end = CGPoint(x: o.x + b.minX + b.width * fx, y: o.y + b.minY + b.height * fy)
            mouse(.mouseMoved, start); mouse(.leftMouseDown, start)
            for i in 1...12 {
                let t = Double(i) / 12.0
                mouse(.leftMouseDragged, CGPoint(x: start.x + (end.x - start.x) * t, y: start.y + (end.y - start.y) * t))
                usleep(15_000)
            }
            mouse(.leftMouseUp, end)
        } else { check(false, "drag: missing \(a[0])") }
        usleep(120_000)
    case "clickat":
        // click absolute content coordinates (imperative apps without layout keys)
        let a = rest.split(separator: " ").compactMap { Double($0) }
        guard a.count == 2, let w = window() else { check(false, "bad clickat \(line)"); continue }
        let o = origin(w)
        let p = CGPoint(x: o.x + a[0], y: o.y + a[1])
        mouse(.mouseMoved, p); mouse(.leftMouseDown, p); usleep(10_000); mouse(.leftMouseUp, p)
        usleep(120_000)
    case "scrollat":
        // scroll the wheel at absolute content coordinates
        let a = rest.split(separator: " ").compactMap { Double($0) }
        guard a.count == 3, let w = window() else { check(false, "bad scrollat \(line)"); continue }
        let o = origin(w)
        mouse(.mouseMoved, CGPoint(x: o.x + a[0], y: o.y + a[1]))
        for _ in 0..<Int(a[2].magnitude) {
            sendPointer(CGEvent(scrollWheelEvent2Source: src, units: .pixel, wheelCount: 1, wheel1: a[2] > 0 ? -40 : 40, wheel2: 0, wheel3: 0))
            usleep(15_000)
        }
        usleep(300_000)
    case "layout":
        // copy the app's layout dump (key -> boxes) for later assertions
        if let text = try? String(contentsOfFile: layoutPath, encoding: .utf8) {
            try? text.write(toFile: "\(outDir)/\(rest).layout", atomically: true, encoding: .utf8)
            print("layout \(rest)")
        } else { check(false, "layout: unreadable \(layoutPath)") }
    case "wait": pause(Double(rest)!)
    case "shot":
        usleep(350_000) // transitions settle
        guard let w = window() else { check(false, "shot: no window"); continue }
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        task.arguments = ["-x", "-o", "-l\(w.id)", "\(outDir)/\(rest).png"]
        do {
            try task.run(); task.waitUntilExit()
            check(task.terminationStatus == 0, "shot \(rest)")
        } catch { check(false, "shot \(rest): \(error)") }
    default: check(false, "unknown step: \(line)")
    }
}
print(failures == 0 ? "scenario ok" : "scenario finished with \(failures) problem(s)")

exit(failures == 0 ? 0 : 1)

// macOS UI scenario runner: drives a Zant window with real mouse and
// keyboard events from a step list and screenshots it at chosen moments
// (the macOS counterpart of scripts/win/ui-auto.ps1; docs/development.md).
//
//   swiftc -O scripts/mac/ui-scenario.swift -o /tmp/ui-scenario
//   ZANT_LAYOUT_FILE=/tmp/layout.txt ./zig-out/bin/zant-tsx-counter &
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
//   key <name>             return | backspace | tab | escape
//   resize <w> <h>         resize the window (content size, via Accessibility)
//   wait <ms>
//   shot <name>            screenshot the window to <out dir>/<name>.png
import AppKit
import ApplicationServices
import Foundation

let args = CommandLine.arguments
guard args.count == 5 else { print("usage: scenario <process> <layout file> <out dir> <steps file>"); exit(2) }
let owner = args[1], layoutPath = args[2], outDir = args[3]
let steps = (try? String(contentsOfFile: args[4], encoding: .utf8)) ?? ""

struct Win { let id: CGWindowID; let frame: CGRect; let pid: pid_t }

func window() -> Win? {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] else { return nil }
    for w in list where (w[kCGWindowOwnerName as String] as? String) == owner && (w[kCGWindowLayer as String] as? Int) == 0 {
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
func mouse(_ type: CGEventType, _ p: CGPoint) {
    CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
}
func point(of key: String) -> CGPoint? {
    guard let w = window(), let b = layout()[key] else { return nil }
    let o = origin(w)
    return CGPoint(x: o.x + b.midX, y: o.y + b.midY)
}
func typeText(_ text: String) {
    for ch in text {
        let units = Array(String(ch).utf16)
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: src, virtualKey: 0, keyDown: down)
            e?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            e?.post(tap: .cghidEventTap)
        }
        usleep(25_000)
    }
}
func pressKey(_ name: String) {
    let codes: [String: CGKeyCode] = ["return": 36, "backspace": 51, "tab": 48, "escape": 53]
    guard let code = codes[name] else { print("unknown key \(name)"); return }
    CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: true)?.post(tap: .cghidEventTap)
    CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: false)?.post(tap: .cghidEventTap)
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

// wait for the window and its first layout
var win: Win? = nil
for _ in 0..<100 {
    if let w = window(), !layout().isEmpty { win = w; break }
    usleep(100_000)
}
guard let first = win else { print("window of \(owner) not found"); exit(1) }
// content height: the root's height in the layout file is the content size
contentHeight = Double(ProcessInfo.processInfo.environment["CONTENT_HEIGHT"] ?? "") ?? first.frame.height - 28
NSRunningApplication(processIdentifier: first.pid)?.activate()
usleep(600_000)

var failures = 0
for raw in steps.split(separator: "\n") {
    let line = raw.trimmingCharacters(in: .whitespaces)
    if line.isEmpty || line.hasPrefix("#") { continue }
    let parts = line.split(separator: " ", maxSplits: 1).map(String.init)
    let cmd = parts[0], rest = parts.count > 1 ? parts[1] : ""
    switch cmd {
    case "click":
        let a = rest.split(separator: " ").map(String.init)
        let times = a.count > 1 ? Int(a[1]) ?? 1 : 1
        for _ in 0..<times {
            guard let p = point(of: a[0]) else { print("missing key \(a[0])"); failures += 1; break }
            mouse(.mouseMoved, p); mouse(.leftMouseDown, p); usleep(10_000); mouse(.leftMouseUp, p)
            usleep(60_000)
        }
    case "hover":
        if let p = point(of: rest) { mouse(.mouseMoved, p) } else { print("missing key \(rest)"); failures += 1 }
        usleep(400_000) // let the hover transition finish
    case "away":
        if let w = window() { mouse(.mouseMoved, CGPoint(x: w.frame.maxX + 60, y: w.frame.maxY + 60)) }
        usleep(400_000)
    case "type": typeText(rest)
    case "key": pressKey(rest)
    case "resize":
        let a = rest.split(separator: " ").compactMap { Double($0) }
        if let w = window(), a.count == 2 { resize(w, a[0], a[1]) }
        usleep(500_000)
    case "clickfrac":
        // click a fraction of a keyed control's box (popup rows, track knobs)
        let a = rest.split(separator: " ").map(String.init)
        guard a.count == 3, let fx = Double(a[1]), let fy = Double(a[2]) else { print("bad clickfrac \(line)"); failures += 1; continue }
        if let w = window(), let b = layout()[a[0]] {
            let o = origin(w)
            let p = CGPoint(x: o.x + b.minX + b.width * fx, y: o.y + b.minY + b.height * fy)
            mouse(.mouseMoved, p); mouse(.leftMouseDown, p); usleep(10_000); mouse(.leftMouseUp, p)
        } else { print("missing key \(a[0])"); failures += 1 }
        usleep(80_000)
    case "scroll":
        // scroll the wheel over a keyed control (positive `lines` scrolls down)
        let a = rest.split(separator: " ").map(String.init)
        guard a.count == 2, let lines = Int32(a[1]) else { print("bad scroll \(line)"); failures += 1; continue }
        if let w = window(), let b = layout()[a[0]] {
            let o = origin(w)
            mouse(.mouseMoved, CGPoint(x: o.x + b.midX, y: o.y + b.midY))
            for _ in 0..<8 {
                CGEvent(scrollWheelEvent2Source: src, units: .line, wheelCount: 1, wheel1: lines, wheel2: 0, wheel3: 0)?.post(tap: .cghidEventTap)
                usleep(20_000)
            }
        } else { print("missing key \(a[0])"); failures += 1 }
        usleep(300_000)
    case "drag":
        // drag from the control's centre to a fraction of its box
        let a = rest.split(separator: " ").map(String.init)
        guard a.count == 3, let fx = Double(a[1]), let fy = Double(a[2]) else { print("bad drag \(line)"); failures += 1; continue }
        if let w = window(), let b = layout()[a[0]] {
            let o = origin(w)
            let start = CGPoint(x: o.x + b.midX, y: o.y + b.midY)
            let end = CGPoint(x: o.x + b.minX + b.width * fx, y: o.y + b.minY + b.height * fy)
            mouse(.mouseMoved, start); mouse(.leftMouseDown, start)
            for i in 1...12 {
                let t = Double(i) / 12.0
                mouse(.leftMouseDragged, CGPoint(x: start.x + (end.x - start.x) * t, y: start.y + (end.y - start.y) * t))
                usleep(15_000)
            }
            mouse(.leftMouseUp, end)
        } else { print("drag: missing \(a[0])"); failures += 1 }
        usleep(120_000)
    case "clickat":
        // click absolute content coordinates (imperative apps without layout keys)
        let a = rest.split(separator: " ").compactMap { Double($0) }
        guard a.count == 2, let w = window() else { print("bad clickat \(line)"); failures += 1; continue }
        let o = origin(w)
        let p = CGPoint(x: o.x + a[0], y: o.y + a[1])
        mouse(.mouseMoved, p); mouse(.leftMouseDown, p); usleep(10_000); mouse(.leftMouseUp, p)
        usleep(120_000)
    case "layout":
        // copy the app's layout dump (key -> boxes) for later assertions
        if let text = try? String(contentsOfFile: layoutPath, encoding: .utf8) {
            try? text.write(toFile: "\(outDir)/\(rest).layout", atomically: true, encoding: .utf8)
            print("layout \(rest)")
        } else { print("layout: unreadable \(layoutPath)"); failures += 1 }
    case "wait": usleep(useconds_t((Double(rest) ?? 0) * 1000))
    case "shot":
        usleep(350_000) // transitions settle
        guard let w = window() else { print("shot: no window"); failures += 1; continue }
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        task.arguments = ["-x", "-o", "-l\(w.id)", "\(outDir)/\(rest).png"]
        try? task.run(); task.waitUntilExit()
        print("shot \(rest)")
    default: print("unknown step: \(line)"); failures += 1
    }
}
print(failures == 0 ? "scenario ok" : "scenario finished with \(failures) problem(s)")

import AppKit
import ApplicationServices
import Carbon.HIToolbox

struct Failure: Error {
    let message: String
    let status: Int32
}

let usage = "usage: manual-check window|front|size|key|text|press|drag|scroll PID … (drag: x0 y0 x1 y1 [right] [hold MS])"

let keys: [String: Int] = [
    "a": kVK_ANSI_A, "b": kVK_ANSI_B, "c": kVK_ANSI_C, "d": kVK_ANSI_D, "e": kVK_ANSI_E, "f": kVK_ANSI_F,
    "g": kVK_ANSI_G, "h": kVK_ANSI_H, "i": kVK_ANSI_I, "j": kVK_ANSI_J, "k": kVK_ANSI_K, "l": kVK_ANSI_L,
    "m": kVK_ANSI_M, "n": kVK_ANSI_N, "o": kVK_ANSI_O, "p": kVK_ANSI_P, "q": kVK_ANSI_Q, "r": kVK_ANSI_R,
    "s": kVK_ANSI_S, "t": kVK_ANSI_T, "u": kVK_ANSI_U, "v": kVK_ANSI_V, "w": kVK_ANSI_W, "x": kVK_ANSI_X,
    "y": kVK_ANSI_Y, "z": kVK_ANSI_Z, "return": kVK_Return, "escape": kVK_Escape, "tab": kVK_Tab,
    "space": kVK_Space, "delete": kVK_Delete, "left": kVK_LeftArrow, "right": kVK_RightArrow,
    "up": kVK_UpArrow, "down": kVK_DownArrow, "quote": kVK_ANSI_Quote,
]

let modifiers: [String: CGEventFlags] = [
    "cmd": .maskCommand, "shift": .maskShift, "alt": .maskAlternate, "ctrl": .maskControl,
]

struct Window {
    let id: Int
    let frame: CGRect
}

func windowList() -> [[String: Any]] {
    let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
    return CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
}

func bounds(_ info: [String: Any]) -> CGRect? {
    guard let dict = info[kCGWindowBounds as String] as? NSDictionary else { return nil }
    return CGRect(dictionaryRepresentation: dict as CFDictionary)
}

func window(of pid: pid_t) throws -> Window {
    for info in windowList() where info[kCGWindowOwnerPID as String] as? pid_t == pid {
        guard info[kCGWindowLayer as String] as? Int == 0, let frame = bounds(info) else { continue }
        return Window(id: info[kCGWindowNumber as String] as? Int ?? 0, frame: frame)
    }
    throw Failure(message: "process \(pid) shows no window", status: 1)
}

func displayFrames() -> [CGRect] {
    var ids = [CGDirectDisplayID](repeating: 0, count: 16)
    var count: UInt32 = 0
    CGGetActiveDisplayList(16, &ids, &count)
    return ids.prefix(Int(count)).map { CGDisplayBounds($0) }
}

func isDockBackdrop(_ info: [String: Any], _ frame: CGRect) -> Bool {
    info[kCGWindowOwnerName as String] as? String == "Dock" && displayFrames().contains(frame)
}

func topmost(at point: CGPoint) -> pid_t? {
    for info in windowList() {
        guard let frame = bounds(info), frame.contains(point), !isDockBackdrop(info, frame) else { continue }
        if (info[kCGWindowAlpha as String] as? Double ?? 1) == 0 { continue }
        return info[kCGWindowOwnerPID as String] as? pid_t
    }
    return nil
}

func scale(of frame: CGRect) -> CGFloat {
    var id: CGDirectDisplayID = 0
    var count: UInt32 = 0
    CGGetDisplaysWithRect(frame, 1, &id, &count)
    let screen = NSScreen.screens.first {
        ($0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value == id
    }
    return (count > 0 ? screen : NSScreen.main)?.backingScaleFactor ?? 1
}

func isInsideVisibleArea(_ point: CGPoint) -> Bool {
    guard let main = NSScreen.screens.first else { return true }
    return NSScreen.screens.contains { screen in
        let visible = screen.visibleFrame
        let top = main.frame.maxY - visible.maxY
        return CGRect(x: visible.minX, y: top, width: visible.width, height: visible.height).contains(point)
    }
}

func isDebugBuild(_ pid: pid_t) -> Bool {
    var buffer = [CChar](repeating: 0, count: 4096)
    guard proc_pidpath(pid, &buffer, UInt32(buffer.count)) > 0 else { return false }
    return String(cString: buffer).hasSuffix("/target/debug/cornercase-gpui")
}

func checkedPid(_ pid: pid_t) throws {
    guard isDebugBuild(pid) else {
        throw Failure(message: "process \(pid) is not a target/debug/cornercase-gpui: nothing was done", status: 2)
    }
}

func allowed() throws {
    guard AXIsProcessTrusted(), CGPreflightPostEventAccess() else {
        throw Failure(
            message: "this process may not post events: allow it in System Settings › Privacy & Security › Accessibility",
            status: 1)
    }
}

func isFront(_ pid: pid_t) -> Bool {
    RunLoop.main.run(until: Date())
    return NSWorkspace.shared.frontmostApplication?.processIdentifier == pid
}

func front(_ pid: pid_t) throws {
    try allowed()
    let app = AXUIElementCreateApplication(pid)
    let result = AXUIElementSetAttributeValue(app, kAXFrontmostAttribute as CFString, kCFBooleanTrue)
    guard result == .success else {
        throw Failure(message: "process \(pid) refused the front (\(result.rawValue))", status: 1)
    }
    for _ in 0..<20 where !isFront(pid) {
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
    }
    guard isFront(pid) else { throw Failure(message: "process \(pid) did not come to the front", status: 3) }
}

func number(_ text: String, _ what: String) throws -> Double {
    guard let value = Double(text), value.isFinite else {
        throw Failure(message: "\(what) must be a number, not '\(text)'", status: 2)
    }
    return value
}

func wholeNumber(_ text: String, _ what: String) throws -> Int32 {
    guard let value = Int32(text) else {
        throw Failure(message: "\(what) must be a whole number, not '\(text)'", status: 2)
    }
    return value
}

func clear(_ pid: pid_t, at point: CGPoint, _ x: String, _ y: String) throws {
    guard isFront(pid) else { throw Failure(message: "process \(pid) is not in front: nothing was posted", status: 3) }
    guard topmost(at: point) == pid else {
        throw Failure(message: "another window covers \(x),\(y): nothing was posted", status: 3)
    }
}

func target(_ pid: pid_t, _ x: String, _ y: String) throws -> CGPoint {
    try allowed()
    let frame = try window(of: pid).frame
    let dx = try number(x, "x")
    let dy = try number(y, "y")
    guard dx >= 0, dy >= 0, dx < frame.width, dy < frame.height else {
        throw Failure(message: "\(x),\(y) is outside the window (\(Int(frame.width))×\(Int(frame.height)))", status: 2)
    }
    let point = CGPoint(x: frame.minX + dx, y: frame.minY + dy)
    guard isInsideVisibleArea(point) else {
        throw Failure(message: "\(x),\(y) is under the Dock or the menu bar: nothing was posted", status: 3)
    }
    try clear(pid, at: point, x, y)
    return point
}

func post(_ type: CGEventType, at point: CGPoint, button: CGMouseButton = .left) {
    CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: button)?
        .post(tap: .cghidEventTap)
}

func hover(_ pid: pid_t, at point: CGPoint, _ x: String, _ y: String) throws {
    post(.mouseMoved, at: point)
    usleep(150_000)
    try clear(pid, at: point, x, y)
}

func keepingPointer(_ body: () throws -> Void) throws {
    let saved = CGEvent(source: nil)?.location
    defer { if let saved { CGWarpMouseCursorPosition(saved) } }
    try body()
}

func press(_ pid: pid_t, _ x: String, _ y: String, right: Bool) throws {
    let point = try target(pid, x, y)
    let button: CGMouseButton = right ? .right : .left
    try keepingPointer {
        try hover(pid, at: point, x, y)
        post(right ? .rightMouseDown : .leftMouseDown, at: point, button: button)
        usleep(50_000)
        post(right ? .rightMouseUp : .leftMouseUp, at: point, button: button)
        usleep(100_000)
    }
}

func drag(_ pid: pid_t, from start: (String, String), to end: (String, String), right: Bool, hold: Int) throws {
    let origin = try target(pid, start.0, start.1)
    let goal = try target(pid, end.0, end.1)
    let button: CGMouseButton = right ? .right : .left
    let (down, moved, up): (CGEventType, CGEventType, CGEventType) =
        right ? (.rightMouseDown, .rightMouseDragged, .rightMouseUp) : (.leftMouseDown, .leftMouseDragged, .leftMouseUp)
    try keepingPointer {
        try hover(pid, at: origin, start.0, start.1)
        try clear(pid, at: goal, end.0, end.1)
        post(down, at: origin, button: button)
        let steps = 12
        for step in 1...steps {
            let fraction = CGFloat(step) / CGFloat(steps)
            let point = CGPoint(
                x: origin.x + (goal.x - origin.x) * fraction, y: origin.y + (goal.y - origin.y) * fraction)
            post(moved, at: point, button: button)
            usleep(21_000)
        }
        usleep(250_000 + UInt32(hold) * 1_000)
        post(up, at: goal, button: button)
        print(String(format: "%.3f", Date().timeIntervalSince1970))
        usleep(100_000)
    }
}

func scroll(_ pid: pid_t, _ x: String, _ y: String, dy: Int32, dx: Int32) throws {
    let point = try target(pid, x, y)
    try keepingPointer {
        try hover(pid, at: point, x, y)
        let event = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: dy, wheel2: dx, wheel3: 0)
        event?.location = point
        event?.post(tap: .cghidEventTap)
        usleep(100_000)
    }
}

func flags(_ held: [String]) throws -> CGEventFlags {
    var flags = CGEventFlags()
    for word in held {
        guard let flag = modifiers[word] else { throw Failure(message: "unknown modifier \(word)", status: 2) }
        flags.insert(flag)
    }
    return flags
}

func key(_ pid: pid_t, _ name: String, _ held: [String]) throws {
    guard let code = keys[name] else { throw Failure(message: "unknown key \(name)", status: 2) }
    let flags = try flags(held)
    try allowed()
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(code), keyDown: down)
        event?.flags = flags
        event?.postToPid(pid)
        usleep(30_000)
    }
}

func text(_ pid: pid_t, _ typed: String) throws {
    try allowed()
    for grapheme in typed {
        let units = Array(String(grapheme).utf16)
        for down in [true, false] {
            let event = CGEvent(keyboardEventSource: nil, virtualKey: 0, keyDown: down)
            event?.keyboardSetUnicodeString(stringLength: units.count, unicodeString: units)
            event?.postToPid(pid)
            usleep(30_000)
        }
    }
}

func size(_ pid: pid_t, _ width: String, _ height: String) throws {
    var wanted = CGSize(width: try number(width, "width"), height: try number(height, "height"))
    try allowed()
    var value: CFTypeRef?
    let app = AXUIElementCreateApplication(pid)
    guard AXUIElementCopyAttributeValue(app, kAXWindowsAttribute as CFString, &value) == .success,
        let windows = value as? [AXUIElement], let first = windows.first,
        let size = AXValueCreate(.cgSize, &wanted),
        AXUIElementSetAttributeValue(first, kAXSizeAttribute as CFString, size) == .success
    else { throw Failure(message: "process \(pid) refused the size", status: 1) }
}

func describe(_ pid: pid_t) throws {
    let found = try window(of: pid)
    let frame = found.frame
    print(found.id, Int(frame.minX), Int(frame.minY), Int(frame.width), Int(frame.height), Int(scale(of: frame)))
}

func button(_ args: [String], after count: Int) throws -> Bool {
    guard args.count == count + 1 else { return false }
    guard args[count] == "right" else {
        throw Failure(message: "\(args[0]) takes only 'right' after the point", status: 2)
    }
    return true
}

func dragOptions(_ words: [String]) throws -> (right: Bool, hold: Int) {
    var rest = words[...]
    let right = rest.first == "right"
    if right { rest = rest.dropFirst() }
    guard !rest.isEmpty else { return (right, 0) }
    guard rest.count == 2, rest.first == "hold", let last = rest.last else {
        throw Failure(message: "drag takes only 'right' and 'hold MS' after the points", status: 2)
    }
    let ms = Int(try wholeNumber(last, "hold"))
    guard (0...10_000).contains(ms) else { throw Failure(message: "hold must be 0 to 10000 ms", status: 2) }
    return (right, ms)
}

func run(_ args: [String]) throws {
    guard args.count >= 2, let pid = pid_t(args[1]) else { throw Failure(message: usage, status: 2) }
    if ["front", "size", "key", "text", "press", "drag", "scroll"].contains(args[0]) { try checkedPid(pid) }
    switch (args[0], args.count) {
    case ("window", 2): try describe(pid)
    case ("front", 2): try front(pid)
    case ("size", 4): try size(pid, args[2], args[3])
    case ("key", 3...): try key(pid, args[2], Array(args.dropFirst(3)))
    case ("text", 3): try text(pid, args[2])
    case ("press", 4...5): try press(pid, args[2], args[3], right: try button(args, after: 4))
    case ("drag", 6...9):
        let (right, hold) = try dragOptions(Array(args.dropFirst(6)))
        try drag(pid, from: (args[2], args[3]), to: (args[4], args[5]), right: right, hold: hold)
    case ("scroll", 5...6):
        let sideways = args.count == 6 ? try wholeNumber(args[5], "dx") : 0
        try scroll(pid, args[2], args[3], dy: try wholeNumber(args[4], "dy"), dx: sideways)
    default: throw Failure(message: usage, status: 2)
    }
}

do {
    try run(Array(CommandLine.arguments.dropFirst()))
} catch let failure as Failure {
    FileHandle.standardError.write(Data((failure.message + "\n").utf8))
    exit(failure.status)
}

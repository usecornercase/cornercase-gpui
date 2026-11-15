import AppKit
import ApplicationServices
import Carbon.HIToolbox

struct Failure: Error {
    let message: String
    let status: Int32
}

let namedKeys: [String: Int] = [
    "return": kVK_Return, "escape": kVK_Escape, "tab": kVK_Tab, "space": kVK_Space, "delete": kVK_Delete,
    "left": kVK_LeftArrow, "right": kVK_RightArrow, "up": kVK_UpArrow, "down": kVK_DownArrow,
    "pageup": kVK_PageUp, "pagedown": kVK_PageDown, "home": kVK_Home, "end": kVK_End, "us-quote": kVK_ANSI_Quote,
]

let letters = "abcdefghijklmnopqrstuvwxyz".map { String($0) }
let digits = "0123456789".map { String($0) }
let symbolNames = ["comma", "slash", "question", "minus", "equal", "plus", "leftbracket", "rightbracket"]
let positionNames = [
    "return", "escape", "tab", "space", "delete", "left", "right", "up", "down", "pageup", "pagedown", "home", "end", "us-quote",
]
let keyGrammar = "a…z 0…9 " + (symbolNames + positionNames).joined(separator: " ")

let countWords: [(word: String, count: Int)] = [("double", 2), ("triple", 3)]
let pressWords = ["right"] + countWords.map { $0.word }
let pressGrammar = pressWords.joined(separator: "|")
let pressListed = pressWords.dropLast().joined(separator: ", ") + " or " + (pressWords.last ?? "")
let pressCounted = countWords.map { $0.word }.joined(separator: " and ")
let pressHint = countWords.map { "\($0.word) for \($0.count)" }.joined(separator: " and ")

let usage =
    "usage: manual-check window|covers|front|size|key|hold|text|paste|copy|press|drag|scroll PID … | displays [x y width height] | self-test "
    + "(key: KEY [cmd] [shift] [alt] [ctrl]; hold: KEY [MS]; KEY: \(keyGrammar); paste: TEXT; copy: TEXT MS [KEY [cmd] [shift] [alt] [ctrl]]; press: x y [\(pressGrammar)]; drag: x0 y0 x1 y1 [right] [hold MS])"

let characterNames: [String: Character] = {
    var names: [String: Character] = [
        "comma": ",", "slash": "/", "question": "?", "minus": "-", "equal": "=", "plus": "+", "leftbracket": "[",
        "rightbracket": "]",
    ]
    for name in letters + digits { names[name] = Character(name) }
    return names
}()

let mainBlock: [Int] = Array(0...35) + Array(37...47) + [50, 93, 94]

let commandState = UInt32(cmdKey >> 8) & 0xFF
let shiftState = UInt32(shiftKey >> 8) & 0xFF
let optionState = UInt32(optionKey >> 8) & 0xFF
let typingStates: [UInt32] = [0, shiftState, optionState, shiftState | optionState]

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

struct Shown {
    let id: Int
    let pid: pid_t
    let owner: String
    let layer: Int
    let alpha: Double
    let frame: CGRect
}

struct Screen {
    let displays: [CGRect]
    let visible: [CGRect]
}

func shown(_ info: [String: Any]) -> Shown? {
    guard let pid = info[kCGWindowOwnerPID as String] as? pid_t, let frame = bounds(info) else { return nil }
    return Shown(
        id: info[kCGWindowNumber as String] as? Int ?? 0, pid: pid,
        owner: info[kCGWindowOwnerName as String] as? String ?? "",
        layer: info[kCGWindowLayer as String] as? Int ?? 0,
        alpha: info[kCGWindowAlpha as String] as? Double ?? 1, frame: frame)
}

func onScreen() -> [Shown] {
    windowList().compactMap(shown)
}

func window(of pid: pid_t) throws -> Window {
    guard let found = onScreen().first(where: { $0.pid == pid && $0.layer == 0 }) else {
        throw Failure(message: "process \(pid) shows no window", status: 1)
    }
    return Window(id: found.id, frame: found.frame)
}

func activeDisplays() -> [CGDirectDisplayID]? {
    var ids = [CGDirectDisplayID](repeating: 0, count: 32)
    var count: UInt32 = 0
    guard CGGetActiveDisplayList(32, &ids, &count) == .success else { return nil }
    return Array(ids.prefix(Int(count)))
}

func displayFrames() -> [CGRect] {
    (activeDisplays() ?? []).map { CGDisplayBounds($0) }
}

func screenNumber(_ screen: NSScreen) -> CGDirectDisplayID? {
    (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value
}

func topLeft(_ visible: CGRect, mainTop: CGFloat) -> CGRect {
    CGRect(x: visible.minX, y: mainTop - visible.maxY, width: visible.width, height: visible.height)
}

func visibleAreas() -> [CGRect] {
    guard let main = NSScreen.screens.first else { return [] }
    return NSScreen.screens.map { topLeft($0.visibleFrame, mainTop: main.frame.maxY) }
}

func screen() -> Screen {
    Screen(displays: displayFrames(), visible: visibleAreas())
}

func isPassedOver(_ window: Shown, on screen: Screen) -> Bool {
    window.alpha == 0 || (window.owner == "Dock" && screen.displays.contains(window.frame))
}

func inTheWay(at point: CGPoint, of pid: pid_t, in list: [Shown], on screen: Screen) -> [Shown]? {
    var found: [Shown] = []
    for window in list where window.frame.contains(point) && !isPassedOver(window, on: screen) {
        if window.pid == pid { return found }
        found.append(window)
    }
    return nil
}

func area(_ rect: CGRect) -> CGRect? {
    rect.isNull || rect.width <= 0 || rect.height <= 0 ? nil : rect
}

func visiblePart(of window: CGRect, over frame: CGRect, on screen: Screen) -> CGRect? {
    let overlap = window.intersection(frame)
    let parts = screen.visible.compactMap { area(overlap.intersection($0)) }
    guard let first = parts.first else { return nil }
    let joined = parts.dropFirst().reduce(first) { $0.union($1) }
    return joined.offsetBy(dx: -frame.minX, dy: -frame.minY)
}

func coversBeforeFront(of pid: pid_t?, frame: CGRect, in list: [Shown], on screen: Screen) -> [(window: Shown, part: CGRect)] {
    list.compactMap { window in
        guard window.layer > 0, window.pid != pid, !isPassedOver(window, on: screen),
            let part = visiblePart(of: window.frame, over: frame, on: screen)
        else { return nil }
        return (window, part)
    }
}

func processName(_ pid: pid_t) -> String? {
    var buffer = [CChar](repeating: 0, count: 4096)
    guard proc_pidpath(pid, &buffer, UInt32(buffer.count)) > 0 else { return nil }
    return String(cString: buffer).split(separator: "/").last.map(String.init)
}

func named(_ window: Shown, _ process: String?) -> String {
    let middle = process.map { ", \($0)" } ?? ""
    return "\(window.owner) (pid \(window.pid)\(middle), layer \(window.layer))"
}

func coverRefusal(_ x: String, _ y: String, _ names: [String]) -> String {
    guard names.count > 1 else { return "another window covers \(x),\(y): \(names.joined()): nothing was posted" }
    return "other windows cover \(x),\(y), front to back: \(names.joined(separator: "; ")): nothing was posted"
}

func partText(_ part: CGRect) -> String {
    "\(Int(part.minX)),\(Int(part.minY)) \(Int(part.width))×\(Int(part.height))"
}

func coverLines(_ found: [(name: String, part: CGRect)]) -> String {
    found.map { name, part in "\(name) covers \(partText(part)) of the window: a press or a scroll there is refused" }
        .joined(separator: "\n")
}

func scale(of frame: CGRect) -> CGFloat {
    var id: CGDirectDisplayID = 0
    var count: UInt32 = 0
    CGGetDisplaysWithRect(frame, 1, &id, &count)
    let screen = NSScreen.screens.first { screenNumber($0) == id }
    return (count > 0 ? screen : NSScreen.main)?.backingScaleFactor ?? 1
}

func isInsideVisibleArea(_ point: CGPoint) -> Bool {
    let areas = visibleAreas()
    return areas.isEmpty || areas.contains { $0.contains(point) }
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
    guard let found = inTheWay(at: point, of: pid, in: onScreen(), on: screen()) else {
        throw Failure(message: "no window of process \(pid) is at \(x),\(y): nothing was posted", status: 3)
    }
    guard found.isEmpty else {
        let names = found.map { named($0, processName($0.pid)) }
        throw Failure(message: coverRefusal(x, y, names), status: 3)
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

let hoverWait = 150
let pressHold = 50
let pressGap = 50
let pressAfter = 100
let askRoom = 50

struct MouseEvent: Equatable {
    let type: CGEventType
    let button: CGMouseButton
    let state: Int64
}

enum PressStep: Equatable {
    case post(MouseEvent)
    case wait(Int)
    case ask
}

func pointerNow() -> CGPoint? { CGEvent(source: nil)?.location }

func putPointer(_ point: CGPoint) { _ = CGWarpMouseCursorPosition(point) }

struct PressEffects {
    let ask: () throws -> Void
    let post: (MouseEvent) -> Void
    var wait: (Int) -> Void = { usleep(useconds_t($0 * 1_000)) }
    var read: () -> CGPoint? = pointerNow
    var warp: (CGPoint) -> Void = putPointer
}

func post(_ type: CGEventType, at point: CGPoint, button: CGMouseButton = .left) {
    CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: point, mouseButton: button)?
        .post(tap: .cghidEventTap)
}

func hover(_ pid: pid_t, at point: CGPoint, _ x: String, _ y: String) throws {
    post(.mouseMoved, at: point)
    usleep(useconds_t(hoverWait * 1_000))
    try clear(pid, at: point, x, y)
}

func keepingPointer(
    read: () -> CGPoint? = pointerNow,
    warp: (CGPoint) -> Void = putPointer,
    _ body: () throws -> Void
) throws {
    let saved = read()
    defer { if let saved { warp(saved) } }
    try body()
}

func pressSteps(_ button: CGMouseButton, count: Int, interval: Int) throws -> [PressStep] {
    guard (1...3).contains(count) else {
        throw Failure(message: "a press posts one, two or three presses, not \(count): nothing was posted", status: 2)
    }
    let least = pressHold + pressGap + askRoom
    if count >= 2, interval < least {
        let word = countWords.first { $0.count == count }?.word ?? ""
        throw Failure(
            message: "a \(word) press needs a double-click interval of \(least) ms or more (its presses are posted "
                + "\(pressHold + pressGap) ms apart, each checked first), and this Mac's is \(interval) ms: a \(word) press cannot be posted on this Mac, and presses posted one by one in its place would act as "
                + "single ones; the interval is the person's own setting, never a check's to change: nothing was posted",
            status: 1)
    }
    let right = button == .right
    var steps: [PressStep] = [.post(MouseEvent(type: .mouseMoved, button: .left, state: 0)), .wait(hoverWait)]
    for number in 1...count {
        if number > 1 { steps.append(.wait(pressGap)) }
        steps.append(.ask)
        steps.append(.post(MouseEvent(type: right ? .rightMouseDown : .leftMouseDown, button: button, state: Int64(number))))
        steps.append(.wait(pressHold))
        steps.append(.post(MouseEvent(type: right ? .rightMouseUp : .leftMouseUp, button: button, state: Int64(number))))
    }
    steps.append(.wait(pressAfter))
    return steps
}

func mouseEvent(_ event: MouseEvent, at point: CGPoint) -> CGEvent? {
    let built = CGEvent(mouseEventSource: nil, mouseType: event.type, mouseCursorPosition: point, mouseButton: event.button)
    built?.setIntegerValueField(.mouseEventClickState, value: event.state)
    return built
}

func halfWayRefusal(_ posted: Int, of total: Int, at place: String, _ why: Failure) -> Failure {
    let tail = ": nothing was posted"
    let reason = why.message.hasSuffix(tail) ? String(why.message.dropLast(tail.count)) : why.message
    let done = posted == 1 ? "one press was" : "two presses were"
    let left = ["the second", "the third"][(posted - 1)..<(total - 1)].joined(separator: " and ")
    let verb = posted == 1 ? "it" : "they"
    let message = "\(done) posted at \(place), \(left) \(total - posted == 1 ? "was" : "were") not: \(reason): look at what \(verb) did before the next press"
    return Failure(message: message, status: why.status)
}

func walkPress(_ steps: [PressStep], at place: String, with effects: PressEffects) throws {
    let total = steps.filter { if case .ask = $0 { true } else { false } }.count
    var pressed = 0
    try keepingPointer(read: effects.read, warp: effects.warp) {
        for step in steps {
            switch step {
            case .post(let event):
                effects.post(event)
                if event.type == .leftMouseUp || event.type == .rightMouseUp { pressed += 1 }
            case .wait(let ms): effects.wait(ms)
            case .ask:
                do { try effects.ask() } catch let failure as Failure where pressed > 0 {
                    throw halfWayRefusal(pressed, of: total, at: place, failure)
                }
            }
        }
    }
}

func press(_ pid: pid_t, _ x: String, _ y: String, button: CGMouseButton, count: Int) throws {
    let steps = try pressSteps(button, count: count, interval: Int(NSEvent.doubleClickInterval * 1_000))
    let point = try target(pid, x, y)
    try walkPress(steps, at: "\(x),\(y)", with: PressEffects(
        ask: { try clear(pid, at: point, x, y) },
        post: { mouseEvent($0, at: point)?.post(tap: .cghidEventTap) }))
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

struct Typed: Equatable {
    let text: String
    let dead: Bool
}

struct Layout {
    let name: String?
    let types: ((Int, UInt32) -> Typed)?
}

struct Reading: Equatable {
    let key: String
    let shift: Bool
    let dead: Bool
}

struct Posting: Equatable {
    let code: Int
    let flags: CGEventFlags
}

func isASCII(_ text: String) -> Bool {
    text.unicodeScalars.allSatisfy { $0.isASCII }
}

func asciiUppercased(_ text: String) -> String {
    String(String.UnicodeScalarView(text.unicodeScalars.map { ($0.value >= 97 && $0.value <= 122) ? Unicode.Scalar($0.value - 32)! : $0 }))
}

func sameText(_ first: String, _ second: String) -> Bool {
    first.unicodeScalars.elementsEqual(second.unicodeScalars)
}

func isASCIILowercase(_ text: String) -> Bool {
    text.unicodeScalars.allSatisfy { $0.value >= 97 && $0.value <= 122 }
}

func windowKey(_ types: (Int, UInt32) -> Typed, code: Int, command: Bool, shift: Bool) -> Reading {
    var plain = types(code, 0)
    var shifted = types(code, shiftState)
    let alwaysCommand = !isASCII(types(0, 0).text) && isASCII(types(0, commandState).text)
    if command || alwaysCommand {
        let withCommand = types(code, commandState)
        let withBoth = types(code, commandState | shiftState)
        if !sameText(withBoth.text, withCommand.text) {
            shifted = withBoth
        } else if asciiUppercased(withCommand.text) != withCommand.text {
            shifted = Typed(text: asciiUppercased(withCommand.text), dead: withCommand.dead)
        }
        plain = withCommand
    }
    if shift && isASCIILowercase(plain.text) { return Reading(key: plain.text, shift: true, dead: plain.dead) }
    if shift { return Reading(key: shifted.text, shift: false, dead: shifted.dead) }
    return Reading(key: plain.text, shift: false, dead: plain.dead)
}

func shortcutKey(_ types: (Int, UInt32) -> Typed, _ wanted: String, command: Bool, shift: Bool) -> (code: Int, shift: Bool)? {
    for used in [false, true] {
        for code in mainBlock {
            let read = windowKey(types, code: code, command: command, shift: used)
            if read.key == wanted && read.shift == shift && !read.dead { return (code, used) }
        }
    }
    return nil
}

func typingKey(
    _ types: (Int, UInt32) -> Typed, _ wanted: String, dead: Bool = false, states: [UInt32] = typingStates
) -> (code: Int, state: UInt32)? {
    for state in states {
        for code in mainBlock {
            let typed = types(code, state)
            if sameText(typed.text, wanted) && typed.dead == dead { return (code, state) }
        }
    }
    return nil
}

func stateFlags(_ state: UInt32) -> CGEventFlags {
    var flags = CGEventFlags()
    if state & shiftState != 0 { flags.insert(.maskShift) }
    if state & optionState != 0 { flags.insert(.maskAlternate) }
    return flags
}

func unknownKey(_ name: String) -> Failure {
    var hint = ""
    if let stands = nameOf(name) {
        hint = "its name is \(stands); "
    } else if name == "quote" {
        hint = "quote is now us-quote, the key where a US keyboard has ' (a dead key on Spanish ISO); "
    }
    return Failure(message: "unknown key \(name.isEmpty ? "'' (an empty name)" : name): \(hint)the keys are \(keyGrammar)", status: 2)
}

func unknownModifier(_ word: String) -> Failure {
    Failure(message: "unknown modifier \(word): the modifiers are cmd, shift, alt and ctrl", status: 2)
}

func nameOf(_ reading: String) -> String? {
    guard reading.count == 1 else { return nil }
    return characterNames.first(where: { $0.value == Character(reading) })?.key
}

func shiftRefusal(_ name: String, _ character: String, _ reading: String?) -> Failure {
    let head = "shift with \(name) is another character: the window reads shift and \(character)"
    guard let reading else {
        return Failure(message: head + " as the character they type: nothing was posted", status: 2)
    }
    let tail = nameOf(reading).map { "so name it: \($0)" } ?? "which no name posts and no shortcut takes"
    return Failure(message: "\(head) as \(reading), \(tail): nothing was posted", status: 2)
}

func noLayoutRefusal(_ name: String) -> Failure {
    Failure(message: "no keyboard layout is in use, so no key can be found for \(name): named keys still work, nothing was posted", status: 1)
}

func noKeyDataRefusal(_ name: String, _ layout: String) -> Failure {
    Failure(
        message: "the keyboard layout in use (\(layout)) has no key data, as an input method's may not, so no key can be found for \(name): named keys still work, nothing was posted",
        status: 1)
}

func altOnlyRefusal(_ name: String, _ layout: String, _ reading: String) -> Failure {
    Failure(
        message: "\(name) is typed only with alt on \(layout), and with cmd or ctrl held the window reads that key without alt, as \(reading): this shortcut cannot reach the window on this layout, nothing was posted",
        status: 1)
}

func advice(_ shortcut: Bool) -> String {
    shortcut ? "this shortcut cannot reach the window on this layout" : "`text` types it"
}

func deadRefusal(_ name: String, _ layout: String, shortcut: Bool = false) -> Failure {
    Failure(message: "\(name) is behind a dead key on \(layout): \(advice(shortcut)), nothing was posted", status: 1)
}

func missingRefusal(_ name: String, _ layout: String, shortcut: Bool = false) -> Failure {
    Failure(message: "no key of \(layout) types \(name): \(advice(shortcut)), nothing was posted", status: 1)
}

func noShortcutRefusal(_ name: String, _ layout: String) -> Failure {
    Failure(message: "no key of \(layout) makes the window read \(name) with these modifiers: nothing was posted", status: 1)
}

func whyNoShortcut(_ types: (Int, UInt32) -> Typed, _ wanted: String, _ name: String, _ layout: String, command: Bool) -> Failure {
    if let typed = typingKey(types, wanted) {
        guard typed.state & optionState != 0 else { return noShortcutRefusal(name, layout) }
        let read = windowKey(types, code: typed.code, command: command, shift: typed.state & shiftState != 0)
        return altOnlyRefusal(name, layout, "\(command ? "cmd" : "ctrl")-\(read.key)")
    }
    return typingKey(types, wanted, dead: true) == nil
        ? missingRefusal(name, layout, shortcut: true) : deadRefusal(name, layout, shortcut: true)
}

func resolveCharacter(_ name: String, _ wanted: Character, _ asked: CGEventFlags, layout: Layout) throws -> Posting {
    let shortcut = asked.contains(.maskCommand) || asked.contains(.maskControl)
    let shift = asked.contains(.maskShift)
    guard let types = layout.types, let id = layout.name else {
        throw layout.name.map { noKeyDataRefusal(name, $0) } ?? noLayoutRefusal(name)
    }
    if shortcut {
        let command = asked.contains(.maskCommand)
        guard let found = shortcutKey(types, String(wanted), command: command, shift: shift) else {
            throw whyNoShortcut(types, String(wanted), name, id, command: command)
        }
        return Posting(code: found.code, flags: asked.union(found.shift ? .maskShift : []))
    }
    let typedText = shift ? String(wanted).uppercased() : String(wanted)
    guard let found = typingKey(types, typedText) else {
        throw typingKey(types, typedText, dead: true) == nil ? missingRefusal(name, id) : deadRefusal(name, id)
    }
    return Posting(code: found.code, flags: asked.union(stateFlags(found.state)))
}

func shiftAddsNothing(_ name: String, _ text: String, _ layout: String, command: Bool, _ words: [String]) -> Failure {
    Failure(
        message: "shift adds nothing to \(name) on \(layout): the window reads it as \(command ? "cmd" : "ctrl")-\(text) with or without it: post \(([name] + words).joined(separator: " ")), nothing was posted",
        status: 2)
}

func typedShiftFailure(_ name: String, _ wanted: Character, _ layout: Layout, _ words: [String]) -> Failure {
    guard let types = layout.types, let id = layout.name, let found = typingKey(types, String(wanted)), found.state & shiftState != 0 else {
        return shiftRefusal(name, String(wanted), nil)
    }
    return Failure(message: "shift adds nothing to \(name) on \(id): the layout types it with shift already: post \(([name] + words).joined(separator: " ")), nothing was posted", status: 2)
}

func shiftFailure(_ name: String, _ wanted: Character, _ layout: Layout, command: Bool, _ words: [String]) -> Failure {
    guard let types = layout.types, let id = layout.name else { return shiftRefusal(name, String(wanted), nil) }
    let text = String(wanted)
    guard let found = shortcutKey(types, text, command: command, shift: false) else {
        return whyNoShortcut(types, text, name, id, command: command)
    }
    let reading = windowKey(types, code: found.code, command: command, shift: true).key
    if reading == text { return shiftAddsNothing(name, text, id, command: command, words) }
    return shiftRefusal(name, text, reading)
}

func resolve(_ name: String, _ held: [String], layout: () -> Layout) throws -> Posting {
    var asked = CGEventFlags()
    for word in held {
        guard let flag = modifiers[word] else { throw unknownModifier(word) }
        asked.insert(flag)
    }
    if let code = namedKeys[name] { return Posting(code: code, flags: asked) }
    guard let wanted = characterNames[name] else { throw unknownKey(name) }
    let shortcut = asked.contains(.maskCommand) || asked.contains(.maskControl)
    let found = layout()
    if asked.contains(.maskShift) && !wanted.isLetter {
        throw shortcut
            ? shiftFailure(name, wanted, found, command: asked.contains(.maskCommand), held.filter { $0 != "shift" })
            : typedShiftFailure(name, wanted, found, held.filter { $0 != "shift" })
    }
    return try resolveCharacter(name, wanted, asked, layout: found)
}

func translate(_ data: Data, _ code: Int, _ state: UInt32) -> Typed {
    var dead: UInt32 = 0
    var length = 0
    var buffer = [UniChar](repeating: 0, count: 4)
    let status = data.withUnsafeBytes { raw -> OSStatus in
        guard let base = raw.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return -1 }
        return UCKeyTranslate(
            base, UInt16(code), UInt16(kUCKeyActionDown), state, UInt32(LMGetKbdType()), 0, &dead, buffer.count, &length, &buffer)
    }
    guard status == noErr else { return Typed(text: "", dead: false) }
    guard dead != 0 else { return Typed(text: String(utf16CodeUnits: buffer, count: length), dead: false) }
    let spacing = data.withUnsafeBytes { raw -> OSStatus in
        guard let base = raw.baseAddress?.assumingMemoryBound(to: UCKeyboardLayout.self) else { return -1 }
        return UCKeyTranslate(
            base, UInt16(kVK_Space), UInt16(kUCKeyActionDown), 0, UInt32(LMGetKbdType()), 0, &dead, buffer.count, &length, &buffer)
    }
    return Typed(text: spacing == noErr ? String(utf16CodeUnits: buffer, count: length) : "", dead: true)
}

func layoutInUse() -> Layout {
    guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue() else {
        return Layout(name: nil, types: nil)
    }
    return withExtendedLifetime(source) {
        let id = TISGetInputSourceProperty(source, kTISPropertyInputSourceID).map {
            Unmanaged<CFString>.fromOpaque($0).takeUnretainedValue() as String
        } ?? "unnamed"
        guard let raw = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else {
            return Layout(name: id, types: nil)
        }
        let cf = Unmanaged<CFData>.fromOpaque(raw).takeUnretainedValue()
        let data = Data(bytes: CFDataGetBytePtr(cf), count: CFDataGetLength(cf))
        return Layout(name: id, types: { code, state in translate(data, code, state) })
    }
}

func postKey(_ pid: pid_t, _ posting: Posting) throws {
    try allowed()
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(posting.code), keyDown: down)
        event?.flags = posting.flags
        event?.postToPid(pid)
        usleep(30_000)
    }
}

func key(_ pid: pid_t, _ name: String, _ held: [String]) throws {
    try postKey(pid, try resolve(name, held, layout: layoutInUse))
}

func holdRefusal(_ word: String) -> Failure {
    Failure(message: "hold takes no modifiers (\(word)): hold PID KEY [MS]; key posts a key with modifiers", status: 2)
}

func holdDuration(_ rest: [String]) throws -> Int {
    if let word = rest.first(where: { modifiers[$0] != nil }) { throw holdRefusal(word) }
    guard rest.count <= 1 else { throw Failure(message: usage, status: 2) }
    let ms = try rest.first.map { Int(try wholeNumber($0, "ms")) } ?? 800
    guard (0...10_000).contains(ms) else { throw Failure(message: "ms must be 0 to 10000", status: 2) }
    return ms
}

func hold(_ pid: pid_t, _ name: String, _ rest: [String]) throws {
    let ms = try holdDuration(rest)
    let posting = try resolve(name, [], layout: layoutInUse)
    try allowed()
    func post(_ down: Bool, repeating: Bool) {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: CGKeyCode(posting.code), keyDown: down)
        if !posting.flags.isEmpty { event?.flags = posting.flags }
        if repeating { event?.setIntegerValueField(.keyboardEventAutorepeat, value: 1) }
        event?.postToPid(pid)
    }
    post(true, repeating: false)
    for _ in 0..<(ms / 33) {
        usleep(33_000)
        post(true, repeating: true)
    }
    post(false, repeating: false)
}

func saved(_ board: NSPasteboard) -> [[(NSPasteboard.PasteboardType, Data)]] {
    (board.pasteboardItems ?? []).map { item in item.types.compactMap { type in item.data(forType: type).map { (type, $0) } } }
}

func readsBack(_ board: NSPasteboard, _ kept: [[(NSPasteboard.PasteboardType, Data)]]) -> Bool {
    kept.map { $0.count } == (board.pasteboardItems ?? []).map { $0.types.count }
}

func restore(_ items: [[(NSPasteboard.PasteboardType, Data)]], to board: NSPasteboard) throws {
    board.clearContents()
    let rebuilt = items.map { entries -> NSPasteboardItem in
        let item = NSPasteboardItem()
        for (type, data) in entries { item.setData(data, forType: type) }
        return item
    }
    guard rebuilt.isEmpty || board.writeObjects(rebuilt) else {
        throw Failure(message: "the clipboard could not be given back: tell the person who runs this check at once", status: 1)
    }
}

func holds(_ back: [[(NSPasteboard.PasteboardType, Data)]], _ kept: [[(NSPasteboard.PasteboardType, Data)]]) -> Bool {
    back.count == kept.count
        && zip(back, kept).allSatisfy { now, was in
            was.allSatisfy { type, data in now.contains { $0.0 == type && $0.1 == data } }
        }
}

func giveBack(
    _ kept: [[(NSPasteboard.PasteboardType, Data)]], to board: NSPasteboard,
    using give: ([[(NSPasteboard.PasteboardType, Data)]], NSPasteboard) throws -> Void = { try restore($0, to: $1) }
) throws {
    try give(kept, board)
    guard holds(saved(board), kept) else {
        throw Failure(message: "the clipboard differs after it was given back: tell the person who runs this check at once", status: 1)
    }
}

func unreadable() -> Failure {
    Failure(message: "the clipboard holds data that cannot be read back: nothing was done", status: 1)
}

func paste(_ pid: pid_t, _ typed: String) throws {
    try allowed()
    let posting = try resolve("v", ["cmd"], layout: layoutInUse)
    let board = NSPasteboard.general
    let kept = saved(board)
    guard readsBack(board, kept) else { throw unreadable() }
    board.clearContents()
    board.setString(typed, forType: .string)
    board.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
    try postKey(pid, posting)
    usleep(300_000)
    try giveBack(kept, to: board)
    print("clipboard: \(kept.count) items, \(kept.reduce(0) { $0 + $1.count }) types, given back")
}

enum Arrival: Equatable {
    case arrived(items: Int)
    case nothing
    case other
}

struct CopyPlan: Equatable {
    let text: String
    let ms: Int
    let posting: Posting?
}

func copyPlan(_ args: [String], layout: () -> Layout) throws -> CopyPlan {
    guard args.count >= 2 else { throw Failure(message: usage, status: 2) }
    guard !args[0].isEmpty else {
        throw Failure(message: "copy needs the made-up text it waits for, not an empty one: nothing was done", status: 2)
    }
    let ms = Int(try wholeNumber(args[1], "ms"))
    guard (1...60_000).contains(ms) else { throw Failure(message: "ms must be 1 to 60000: nothing was done", status: 2) }
    guard args.count > 2 else { return CopyPlan(text: args[0], ms: ms, posting: nil) }
    return CopyPlan(text: args[0], ms: ms, posting: try resolve(args[2], Array(args.dropFirst(3)), layout: layout))
}

func awaitCopy(
    of text: String, on board: NSPasteboard, within ms: Int, act: () throws -> Void,
    whileSaving: () -> Void = {}, beforeGivingBack: () -> Void = {},
    giving give: ([[(NSPasteboard.PasteboardType, Data)]], NSPasteboard) throws -> Void = { try restore($0, to: $1) }
) throws -> Arrival {
    let before = board.changeCount
    whileSaving()
    let kept = saved(board)
    guard readsBack(board, kept) else { throw unreadable() }
    guard board.changeCount == before else {
        throw Failure(message: "the clipboard changed while it was read: nothing was done, try again", status: 1)
    }
    try act()
    let deadline = Date(timeIntervalSinceNow: Double(ms) / 1000)
    while board.changeCount == before && Date() < deadline { usleep(10_000) }
    let after = board.changeCount
    if after == before { return .nothing }
    guard after == before + 1, board.data(forType: .string) == Data(text.utf8) else { return .other }
    beforeGivingBack()
    guard board.changeCount == after else { return .other }
    try giveBack(kept, to: board, using: give)
    return .arrived(items: kept.count)
}

func copyAnswer(_ arrival: Arrival, ms: Int) -> (status: Int32, words: String) {
    switch arrival {
    case .arrived(let items):
        return (0, "the copy arrived: \(items) \(items == 1 ? "item" : "items") given back")
    case .nothing:
        return (4, "nothing arrived on the clipboard in \(ms) ms: it was not touched")
    case .other:
        return (
            5,
            "something else arrived on the clipboard (another text, a second copy, or the person's own; the text given must be the copy's exact bytes, with no newline added): it is left as it is and none "
                + "of it is printed; what it held before was not given back: say so to the person whose clipboard it is"
        )
    }
}

func say(_ line: String) {
    FileHandle.standardOutput.write(Data((line + "\n").utf8))
}

func copy(_ pid: pid_t, _ args: [String]) throws {
    let plan = try copyPlan(args, layout: layoutInUse)
    if plan.posting != nil { try allowed() }
    let arrival = try awaitCopy(of: plan.text, on: NSPasteboard.general, within: plan.ms) {
        say("waiting up to \(plan.ms) ms for the copy; the clipboard is kept in memory")
        if let posting = plan.posting { try postKey(pid, posting) }
    }
    let answer = copyAnswer(arrival, ms: plan.ms)
    guard answer.status == 0 else { throw Failure(message: answer.words, status: answer.status) }
    say(answer.words)
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

func covers(_ pid: pid_t) throws {
    let frame = try window(of: pid).frame
    let found = coversBeforeFront(of: pid, frame: frame, in: onScreen(), on: screen())
    guard found.isEmpty else {
        let lines = found.map { (name: named($0.window, processName($0.window.pid)), part: $0.part) }
        throw Failure(message: coverLines(lines), status: 3)
    }
}

struct Display: Equatable {
    let uuid: String?
    let frame: CGRect
    let main: Bool
}

struct Cover {
    let window: Shown
    let part: CGRect
}

let displaysNone = "the system lists no active display (CGGetActiveDisplayList), so there is none to name"
let displaysNoneFree =
    "no display is free: on each a window of a higher layer covers all or part of it (the lines above say which and where); a place clear of a cover can be asked with displays X Y WIDTH HEIGHT"

func displaysNoneFreeAt(_ place: CGRect) -> String {
    "no display is free at \(partText(place)): on each a window of a higher layer covers that place or the place is not inside it (the lines above say which)"
}

func uuidText(_ bytes: CFUUIDBytes) -> String {
    let hex = withUnsafeBytes(of: bytes) { $0.map { String(format: "%02x", $0) } }
    return [hex[0..<4], hex[4..<6], hex[6..<8], hex[8..<10], hex[10..<16]].map { $0.joined() }.joined(separator: "-")
}

func displayUUID(_ id: CGDirectDisplayID) -> String? {
    guard let made = CGDisplayCreateUUIDFromDisplayID(id)?.takeRetainedValue() else { return nil }
    return uuidText(CFUUIDGetUUIDBytes(made))
}

func readDisplays() -> [Display] {
    let mainNumber = NSScreen.screens.first.flatMap(screenNumber)
    return (activeDisplays() ?? []).map { Display(uuid: displayUUID($0), frame: CGDisplayBounds($0), main: $0 == mainNumber) }
}

func coversOf(_ display: Display, place: CGRect?, in list: [Shown], on screen: Screen) -> [Cover]? {
    let own = CGRect(origin: .zero, size: display.frame.size)
    let local = place ?? own
    guard own.contains(local) else { return nil }
    let there = local.offsetBy(dx: display.frame.minX, dy: display.frame.minY)
    return coversBeforeFront(of: nil, frame: there, in: list, on: screen).map { Cover(window: $0.window, part: $0.part) }
}

func displayLine(_ display: Display, place: CGRect?, found: [Cover]?, naming: (Shown) -> String) -> String {
    let frame = display.frame
    let numbers = [frame.minX, frame.minY, frame.width, frame.height].map { String(Int($0)) }
    let head = ([display.uuid ?? "-"] + numbers + [display.main ? "main" : "other"]).joined(separator: " ")
    guard let found else { return "\(head) outside: \(partText(place ?? .zero)) is not inside it" }
    guard !found.isEmpty else { return "\(head) free" }
    let of = place == nil ? "of the display" : "of the place"
    return "\(head) covered: " + found.map { "\(naming($0.window)) covers \(partText($0.part)) \(of)" }.joined(separator: "; ")
}

func displaysAnswer(
    _ displays: [Display], place: CGRect?, in list: [Shown], on screen: Screen, naming: (Shown) -> String
) -> (lines: [String], status: Int32, words: String?) {
    guard !displays.isEmpty else { return ([], 1, displaysNone) }
    let found = displays.map { coversOf($0, place: place, in: list, on: screen) }
    let lines = zip(displays, found).map { displayLine($0, place: place, found: $1, naming: naming) }
    if found.contains(where: { $0?.isEmpty == true }) { return (lines, 0, nil) }
    return (lines, 3, place.map(displaysNoneFreeAt) ?? displaysNoneFree)
}

func askedPlace(_ words: [String]) throws -> CGRect? {
    guard !words.isEmpty else { return nil }
    guard words.count == 4 else {
        throw Failure(
            message: "displays takes no pid: nothing, or a place as x y width height in a display's own points from its top-left (as layout.json's bounds), not '\(words.joined(separator: " "))'",
            status: 2)
    }
    let numbers = try zip(["x", "y", "width", "height"], words).map { CGFloat(try wholeNumber($1, $0)) }
    guard numbers[0] >= 0, numbers[1] >= 0 else {
        throw Failure(
            message: "a place is in a display's own points from its top-left (as layout.json's bounds), so its x and y are 0 or more, not \(words[0]),\(words[1]): the numbers on displays' own lines are global, a window's place is not",
            status: 2)
    }
    guard numbers[2] > 0, numbers[3] > 0 else {
        throw Failure(message: "a place's width and height are more than 0, not \(words[2])×\(words[3])", status: 2)
    }
    return CGRect(x: numbers[0], y: numbers[1], width: numbers[2], height: numbers[3])
}

func listDisplays(_ words: [String]) throws {
    let place = try askedPlace(words)
    let answer = displaysAnswer(
        readDisplays(), place: place, in: onScreen(), on: screen(), naming: { named($0, processName($0.pid)) })
    answer.lines.forEach(say)
    if let message = answer.words { throw Failure(message: message, status: answer.status) }
}

func pressOptions(_ words: [String]) throws -> (button: CGMouseButton, count: Int) {
    switch words {
    case []: return (.left, 1)
    case ["right"]: return (.right, 1)
    default: break
    }
    if words.count == 1, let found = countWords.first(where: { $0.word == words[0] }) { return (.left, found.count) }
    let named = Set(words)
    if words.count == 2, named.contains("right"), !named.isDisjoint(with: countWords.map { $0.word }) {
        throw Failure(
            message: "a right press takes no count: \(pressCounted) post left presses only, and press PID X Y right posts one right press: nothing was posted",
            status: 2)
    }
    let said = "press takes at most one word after the point (\(pressListed)), not '\(words.joined(separator: " "))'"
    let hint = words.count == 1 && Int(words[0]) != nil ? ": a count is a word, \(pressHint) (there is no other)" : ""
    throw Failure(message: said + hint + ": nothing was posted", status: 2)
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

enum SelfTest {
    static let testDebug: pid_t = 4242
    static let cover = "Screenshot (pid 69680, screencaptureui, layer 24)"
    static let testDisplay = CGRect(x: 0, y: 0, width: 1512, height: 982)
    static let testVisible = CGRect(x: 0, y: 34, width: 1512, height: 912)
    static let testScreen = Screen(displays: [testDisplay], visible: [testVisible])

    static func made(_ id: Int, _ pid: pid_t, _ owner: String, _ layer: Int, _ frame: CGRect, alpha: Double = 1) -> Shown {
        Shown(id: id, pid: pid, owner: owner, layer: layer, alpha: alpha, frame: frame)
    }

    static func debugWindow(at y: CGFloat = 34) -> Shown {
        made(100, testDebug, "cornercase-gpui", 0, CGRect(x: 0, y: y, width: 1512, height: 912))
    }

    static func ids(_ found: [Shown]?) -> String {
        found.map { "\($0.map { String($0.id) }.joined(separator: ","))" } ?? "nil"
    }

    static func expect<T: Equatable>(_ got: T, _ want: T) -> String? {
        got == want ? nil : "got \(got), wanted \(want)"
    }

    static func atPoint(_ list: [Shown]) -> String {
        ids(inTheWay(at: CGPoint(x: 200, y: 200), of: testDebug, in: list, on: testScreen))
    }

    static func before(_ list: [Shown], at y: CGFloat = 34) -> [String] {
        coversBeforeFront(of: testDebug, frame: debugWindow(at: y).frame, in: list, on: testScreen)
            .map { "\($0.window.id) \($0.part)" }
    }

    static let wide = CGRect(x: 0, y: 0, width: 1512, height: 982)
    static let bar = CGRect(x: 0, y: 0, width: 1512, height: 33)
    static let small = CGRect(x: 100, y: 134, width: 300, height: 200)
    static let near = CGRect(x: 100, y: 100, width: 300, height: 200)

    static func dictionaryTests() -> [(String, () -> String?)] {
        let info: [String: Any] = [
            kCGWindowOwnerPID as String: Int32(7), kCGWindowOwnerName as String: "Dock", kCGWindowLayer as String: 20,
            kCGWindowAlpha as String: 0.5, kCGWindowNumber as String: 9,
            kCGWindowBounds as String: wide.dictionaryRepresentation as NSDictionary,
        ]
        return [
            ("a_window_dictionary_is_read", {
                guard let read = shown(info) else { return "not read" }
                return expect("\(read.id) \(read.pid) \(read.owner) \(read.layer) \(read.alpha) \(read.frame)", "9 7 Dock 20 0.5 \(wide)")
            }),
            ("a_window_dictionary_without_bounds_is_not_read", { expect(shown([kCGWindowOwnerPID as String: Int32(7)]) == nil, true) }),
        ]
    }

    static func coverTests() -> [(String, () -> String?)] {
        let a = made(1, 10, "A", 25, wide)
        let b = made(2, 11, "B", 0, wide)
        let d = debugWindow()
        return [
            ("a_window_in_front_at_the_point_covers_it", { expect(atPoint([b, d]), "2") }),
            ("two_windows_in_front_cover_it_front_to_back", { expect(atPoint([a, b, d]), "1,2") }),
            ("the_docks_full_screen_backdrop_is_passed_over", { expect(atPoint([made(3, 12, "Dock", 20, testDisplay), d]), "") }),
            ("a_dock_window_smaller_than_the_display_covers_it", {
                expect(atPoint([made(3, 12, "Dock", 20, CGRect(x: 0, y: 150, width: 1512, height: 80)), d]), "3")
            }),
            ("a_window_of_alpha_0_is_passed_over", { expect(atPoint([made(3, 12, "X", 24, wide, alpha: 0), d]), "") }),
            ("nothing_covers_the_debug_window_on_top", { expect(atPoint([d, b]), "") }),
            ("a_window_beside_the_point_is_no_cover", { expect(atPoint([made(3, 12, "X", 3, CGRect(x: 600, y: 600, width: 50, height: 50)), d]), "") }),
            ("a_window_of_the_debug_process_is_no_cover", { expect(atPoint([made(3, testDebug, "cornercase-gpui", 101, wide), d]), "") }),
            ("no_window_of_the_process_at_the_point_gives_nil", { expect(atPoint([b]), "nil") }),
        ]
    }

    static func beforeTests() -> [(String, () -> String?)] {
        let d = debugWindow()
        let mac = (1...10).map { made($0, 20, "Control Center", 25, CGRect(x: 1200, y: 0, width: 40, height: 33)) }
            + [made(11, 69680, "Screenshot", 24, wide), made(12, 21, "Window Server", 24, bar),
                made(13, 22, "Dock", 20, testDisplay), d, made(15, 23, "Google Chrome", 0, CGRect(x: 244, y: 57, width: 1024, height: 822))]
        return [
            ("before_the_front_a_higher_layer_over_the_window_covers_it", { expect(before([made(1, 10, "A", 3, small), d]).count, 1) }),
            ("before_the_front_a_window_of_the_debug_process_is_no_cover", {
                expect(before([made(1, testDebug, "cornercase-gpui", 101, wide), d]), [])
            }),
            ("before_the_front_a_bar_touching_the_visible_area_is_no_cover", {
                expect(before([made(1, 10, "A", 24, CGRect(x: 0, y: 0, width: 1512, height: 34)), d], at: 25), [])
            }),
            ("before_the_front_a_window_of_layer_0_is_no_cover_yet", { expect(before([made(1, 10, "A", 0, wide), d]), []) }),
            ("before_the_front_the_menu_bar_and_its_items_are_no_cover", {
                expect(before([made(1, 10, "A", 25, bar), made(2, 21, "Window Server", 24, bar)], at: 25), [])
            }),
            ("before_the_front_the_docks_backdrop_and_alpha_0_are_no_cover", {
                expect(before([made(1, 22, "Dock", 20, testDisplay), made(2, 10, "A", 24, wide, alpha: 0)]), [])
            }),
            ("before_the_front_a_higher_layer_beside_the_window_is_no_cover", {
                expect(before([made(1, 10, "A", 3, CGRect(x: 2000, y: 100, width: 50, height: 50))]), [])
            }),
            ("before_the_front_the_covered_part_is_in_the_windows_points", { expect(before([made(1, 10, "A", 3, small)]), ["1 \(near)"]) }),
            ("this_macs_windows_report_only_the_screenshot_window", { expect(before(mac), ["11 \(CGRect(x: 0, y: 0, width: 1512, height: 912))"]) }),
            ("this_macs_windows_without_the_screenshot_window_report_nothing", { expect(before(mac.filter { $0.id != 11 }), []) }),
        ]
    }

    static func wordTests() -> [(String, () -> String?)] {
        let w = made(11, 69680, "Screenshot", 24, wide)
        let name = cover
        let second = "Notes (pid 5, layer 3)"
        return [
            ("the_name_has_owner_pid_process_and_layer", { expect(named(w, "screencaptureui"), name) }),
            ("the_name_leaves_out_a_process_it_could_not_read", { expect(named(made(1, 5, "Notes", 3, wide), nil), second) }),
            ("the_refusal_names_one_cover", { expect(coverRefusal("200", "100", [name]), "another window covers 200,100: \(name): nothing was posted") }),
            ("the_refusal_names_two_covers_front_to_back", {
                expect(coverRefusal("200", "100", [name, second]), "other windows cover 200,100, front to back: \(name); \(second): nothing was posted")
            }),
            ("the_lines_say_where_each_window_covers_the_window", {
                expect(coverLines([(name, wide), (second, near)]),
                    "\(name) covers 0,0 1512×982 of the window: a press or a scroll there is refused\n"
                    + "\(second) covers 100,100 300×200 of the window: a press or a scroll there is refused")
            }),
        ]
    }

    static func madeLayout(_ name: String, _ rows: [Int: [UInt32: String]]) -> Layout {
        Layout(name: name, types: { code, state in
            let row = rows[code] ?? [:]
            let base = row[state] ?? (state & commandState != 0 ? row[0] : nil) ?? ""
            return Typed(text: base.hasPrefix("~") ? String(base.dropFirst()) : base, dead: base.hasPrefix("~"))
        })
    }

    static let qwerty: [String: Int] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11, "q": 12, "w": 13, "e": 14,
        "r": 15, "y": 16, "t": 17, "o": 31, "u": 32, "i": 34, "p": 35, "l": 37, "j": 38, "k": 40, "n": 45, "m": 46,
    ]

    static let letterRows: [Int: [UInt32: String]] = Dictionary(
        uniqueKeysWithValues: qwerty.map { ($0.value, [0: $0.key, 2: $0.key.uppercased()]) })

    static let digitRows: [Int: [UInt32: String]] = [
        18: [0: "1", 2: "!"], 19: [0: "2", 2: "@"], 20: [0: "3", 2: "#"], 21: [0: "4", 2: "$"], 23: [0: "5", 2: "%"],
        22: [0: "6", 2: "^"], 26: [0: "7", 2: "&"], 28: [0: "8", 2: "*"], 25: [0: "9", 2: "("], 29: [0: "0", 2: ")"],
    ]

    static let us = madeLayout("test.US", letterRows.merging(digitRows) { $1 }.merging([
        27: [0: "-", 2: "_"], 24: [0: "=", 2: "+"], 33: [0: "[", 2: "{"], 30: [0: "]", 2: "}"],
        43: [0: ",", 2: "<"], 44: [0: "/", 2: "?"],
    ]) { $1 })

    static let spanish = madeLayout("test.Spanish-ISO", letterRows.merging(digitRows) { $1 }.merging([
        26: [0: "7", 2: "/", 8: "÷"], 29: [0: "0", 2: "=", 8: "≠"], 27: [0: "'", 2: "?", 8: "´"], 24: [0: "¡", 2: "¿"],
        30: [0: "+", 2: "*", 8: "]", 10: "±"], 33: [0: "~`", 2: "~^", 8: "["], 39: [0: "~´", 2: "~¨", 8: "{"],
        43: [0: ",", 2: ";"], 44: [0: "-", 2: "_"], 75: [0: "/"],
    ]) { $1 })

    static let azerty = madeLayout("test.French", [
        0: [0: "q", 2: "Q"], 12: [0: "a", 2: "A"], 18: [0: "&", 2: "1", 1: "&", 3: "1"],
    ])

    static let dvorakCommand = madeLayout("test.Dvorak-QWERTY", [
        40: [0: "t", 2: "T", 1: "k", 3: "K"], 9: [0: "k", 2: "K", 1: "v", 3: "V"],
    ])

    static let cyrillic = madeLayout("test.Russian", [0: [0: "ф", 2: "Ф", 1: "a", 3: "A"], 8: [0: "с", 2: "С", 1: "c", 3: "C"]])

    static let ruLike = madeLayout("test.Ru-Like", [44: [0: ".", 2: ",", 1: "/", 3: "?"]])

    static let orderLike = madeLayout("test.Order", [20: [0: "a", 2: "x"], 5: [0: "b", 8: "x"]])

    static let plusTwice = madeLayout("test.Plus", [30: [0: "+"], 24: [0: "=", 2: "+"]])

    static let turkish = madeLayout("test.Turkish-Q", [34: [0: "ı", 2: "I"], 39: [0: "i", 2: "İ"]])

    static let combining = madeLayout("test.Combining", [44: [0: "x", 2: "/", 1: "a\u{301}", 3: "a\u{301}"]])

    static let canonical = madeLayout("test.Canonical", [44: [0: "x", 2: "X", 1: "e\u{301}", 3: "\u{e9}"]])

    static let kelvin = madeLayout("test.Kelvin", [10: [2: "\u{212A}"], 40: [0: "k", 2: "K"]])

    static let noData = Layout(name: "test.Input-Method", types: nil)
    static let nothing = Layout(name: nil, types: nil)

    static func posted(_ name: String, _ held: [String], _ layout: Layout) -> String {
        do {
            let found = try resolve(name, held, layout: { layout })
            return "\(found.code) \(found.flags.contains(.maskShift) ? "S" : "")\(found.flags.contains(.maskCommand) ? "C" : "")\(found.flags.contains(.maskAlternate) ? "A" : "")\(found.flags.contains(.maskControl) ? "K" : "")"
        } catch let failure as Failure {
            return "\(failure.status): \(failure.message)"
        } catch { return "\(error)" }
    }

    static func refused(_ name: String, _ held: [String], _ layout: Layout) -> String {
        let got = posted(name, held, layout)
        return got.first?.isNumber == true && got.contains(":") ? got : "posted \(got)"
    }

    static func keyTests() -> [(String, () -> String?)] {
        [
            ("every_name_posts_its_key_on_a_us_layout", {
                let want = [("comma", 43, "C"), ("slash", 44, "C"), ("question", 44, "SC"), ("minus", 27, "C"), ("equal", 24, "C"),
                    ("plus", 24, "SC"), ("leftbracket", 33, "C"), ("rightbracket", 30, "C"), ("1", 18, "C"), ("9", 25, "C"), ("0", 29, "C")]
                return expect(want.map { posted($0.0, ["cmd"], us) }, want.map { "\($0.1) \($0.2)" })
            }),
            ("every_name_posts_its_key_or_says_why_on_spanish_iso", {
                let want = [("comma", "43 C"), ("slash", "26 SC"), ("question", "27 SC"), ("minus", "44 C"), ("equal", "29 SC"),
                    ("plus", "30 C"), ("1", "18 C"), ("0", "29 C")]
                let seen = want.map { posted($0.0, ["cmd"], spanish) }
                return expect(seen, want.map { $0.1 })
                    ?? expect(refused("leftbracket", ["cmd"], spanish).prefix(2), "1:")
                    ?? expect(refused("rightbracket", ["cmd"], spanish).prefix(2), "1:")
            }),
            ("a_named_key_is_its_position_on_every_layout", {
                let all = [us, spanish, noData, nothing].map { [posted("pageup", [], $0), posted("home", [], $0), posted("us-quote", [], $0)] }
                return expect(all, Array(repeating: ["116 ", "115 ", "39 "], count: 4))
            }),
            ("a_symbol_needing_shift_reaches_the_window_without_it", {
                guard let types = spanish.types else { return "no data" }
                return expect(windowKey(types, code: 26, command: true, shift: true), Reading(key: "/", shift: false, dead: false))
            }),
            ("a_character_needing_alt_is_refused_with_cmd", {
                expect(refused("leftbracket", ["cmd"], spanish),
                    "1: leftbracket is typed only with alt on test.Spanish-ISO, and with cmd or ctrl held the window reads that key without alt, as cmd-`: this shortcut cannot reach the window on this layout, nothing was posted")
            }),
            ("a_character_needing_alt_is_typed_with_it", { expect(posted("leftbracket", [], spanish), "33 A") }),
            ("a_character_behind_a_dead_key_is_refused", {
                expect(refused("leftbracket", [], madeLayout("test.Dead", [33: [0: "~["]])).contains("dead key"), true)
            }),
            ("a_dead_symbol_under_cmd_is_refused_as_dead", {
                let layout = madeLayout("test.Dead", [33: [0: "~[", 1: "~["]])
                return expect(refused("leftbracket", ["cmd"], layout).contains("dead key"), true)
            }),
            ("a_character_the_layout_lacks_is_refused", { expect(refused("slash", ["cmd"], madeLayout("test.Bare", [0: [0: "a"]])).contains("no key of test.Bare types slash"), true) }),
            ("a_keypad_key_is_never_chosen", { expect(posted("slash", ["cmd"], spanish), "26 SC") }),
            ("fewer_modifiers_win_over_a_lower_code", { expect(posted("plus", ["cmd"], plusTwice), "30 C") }),
            ("a_letter_is_posted_where_the_layout_has_it", { expect(posted("a", ["cmd"], azerty), "12 C") }),
            ("a_digit_that_needs_shift_gets_it_with_cmd", { expect(posted("1", ["cmd"], azerty), "18 SC") }),
            ("with_cmd_the_command_table_decides", { expect([posted("k", ["cmd"], dvorakCommand), posted("k", [], dvorakCommand)], ["40 C", "9 "]) }),
            ("a_layout_of_other_letters_reads_its_command_table", {
                expect(posted("c", ["ctrl"], cyrillic), "8 K") ?? expect(refused("a", [], cyrillic).hasPrefix("1: "), true)
            }),
            ("shift_stays_with_a_letter", { expect(posted("k", ["cmd", "shift"], us), "40 SC") }),
            ("shift_with_a_symbol_without_a_name_is_refused_with_its_reading", {
                expect(refused("leftbracket", ["cmd", "shift"], us),
                    "2: shift with leftbracket is another character: the window reads shift and [ as {, which no name posts and no shortcut takes: nothing was posted")
            }),
            ("shift_with_a_symbol_that_has_a_name_says_it", {
                expect(refused("slash", ["cmd", "shift"], us),
                    "2: shift with slash is another character: the window reads shift and / as ?, so name it: question: nothing was posted")
            }),
            ("shift_adds_nothing_to_a_typed_symbol_whose_key_already_takes_it", {
                expect([refused("slash", ["shift"], spanish), refused("slash", ["alt", "shift"], spanish), refused("slash", ["cmd", "alt", "shift"], spanish)], [
                    "2: shift adds nothing to slash on test.Spanish-ISO: the layout types it with shift already: post slash, nothing was posted",
                    "2: shift adds nothing to slash on test.Spanish-ISO: the layout types it with shift already: post slash alt, nothing was posted",
                    "2: shift adds nothing to slash on test.Spanish-ISO: the window reads it as cmd-/ with or without it: post slash cmd alt, nothing was posted",
                ])
            }),
            ("shift_with_a_symbol_and_no_cmd_is_refused_without_a_reading", {
                expect(refused("slash", ["shift"], us),
                    "2: shift with slash is another character: the window reads shift and / as the character they type: nothing was posted")
            }),
            ("alt_with_a_character_is_posted_on_the_key_that_types_it_plain", {
                expect([posted("2", ["alt"], spanish), posted("leftbracket", ["alt"], spanish)], ["19 A", "33 A"])
            }),
            ("shift_adds_nothing_to_a_symbol_whose_key_already_takes_it", {
                expect([refused("slash", ["cmd", "shift"], spanish), refused("question", ["cmd", "shift"], us)], [
                    "2: shift adds nothing to slash on test.Spanish-ISO: the window reads it as cmd-/ with or without it: post slash cmd, nothing was posted",
                    "2: shift adds nothing to question on test.US: the window reads it as cmd-? with or without it: post question cmd, nothing was posted",
                ])
            }),
            ("shift_with_a_symbol_that_needs_alt_says_the_true_thing", {
                expect(refused("leftbracket", ["cmd", "shift"], spanish),
                    "1: leftbracket is typed only with alt on test.Spanish-ISO, and with cmd or ctrl held the window reads that key without alt, as cmd-`: this shortcut cannot reach the window on this layout, nothing was posted")
            }),
            ("the_shifted_command_table_decides_when_it_differs", {
                expect([posted("question", ["cmd"], ruLike), posted("slash", ["cmd"], ruLike)], ["44 SC", "44 C"])
            }),
            ("the_typing_search_tries_no_modifier_then_shift_before_alt_and_states_before_codes", {
                expect([posted("plus", [], plusTwice), posted("x", [], orderLike)], ["30 ", "20 S"])
            }),
            ("every_character_name_posts_its_key_on_a_us_layout", {
                var want: [String] = []
                var got: [String] = []
                for (name, code) in qwerty { want.append("\(code) C"); got.append(posted(name, ["cmd"], us)) }
                for (index, name) in digits.enumerated() {
                    want.append("\([29, 18, 19, 20, 21, 23, 22, 26, 28, 25][index]) C")
                    got.append(posted(name, ["cmd"], us))
                }
                return expect(got, want)
            }),
            ("every_named_key_posts_its_position", {
                let want = ["return": 36, "escape": 53, "tab": 48, "space": 49, "delete": 51, "left": 123, "right": 124, "up": 126,
                    "down": 125, "pageup": 116, "pagedown": 121, "home": 115, "end": 119, "us-quote": 39]
                return expect(Dictionary(uniqueKeysWithValues: positionNames.map { ($0, posted($0, [], us)) }),
                    Dictionary(uniqueKeysWithValues: want.map { ($0.key, "\($0.value) ") }))
            }),
            ("an_empty_name_is_unknown_and_posts_nothing", { expect(refused("", ["cmd"], us), "2: unknown key '' (an empty name): the keys are \(keyGrammar)") }),
            ("every_refusal_says_its_words", {
                let all = [
                    refused("a", [], nothing), refused("a", [], noData), refused("slash", ["cmd", "shift"], noData), refused("t", ["cmd"], dvorakCommand),
                    refused("slash", ["cmd"], madeLayout("test.Bare", [0: [0: "a"]])),
                    refused("slash", [], madeLayout("test.Bare", [0: [0: "a"]])),
                    refused("leftbracket", ["cmd"], madeLayout("test.Dead", [33: [0: "~["]])),
                    refused("leftbracket", [], madeLayout("test.Dead", [33: [0: "~["]])),
                ]
                return expect(all, [
                    "1: no keyboard layout is in use, so no key can be found for a: named keys still work, nothing was posted",
                    "1: the keyboard layout in use (test.Input-Method) has no key data, as an input method's may not, so no key can be found for a: named keys still work, nothing was posted",
                    "2: shift with slash is another character: the window reads shift and / as the character they type: nothing was posted",
                    "1: no key of test.Dvorak-QWERTY makes the window read t with these modifiers: nothing was posted",
                    "1: no key of test.Bare types slash: this shortcut cannot reach the window on this layout, nothing was posted",
                    "1: no key of test.Bare types slash: `text` types it, nothing was posted",
                    "1: leftbracket is behind a dead key on test.Dead: this shortcut cannot reach the window on this layout, nothing was posted",
                    "1: leftbracket is behind a dead key on test.Dead: `text` types it, nothing was posted",
                ])
            }),
            ("hold_says_it_takes_no_modifiers", {
                expect(holdRefusal("cmd").message, "hold takes no modifiers (cmd): hold PID KEY [MS]; key posts a key with modifiers")
            }),
            ("hold_reads_its_duration_and_refuses_a_modifier_word", {
                let words = [["cmd"], ["500", "cmd"], ["500", "600"], ["abc"], ["20000"], []].map { rest -> String in
                    do { return "\(try holdDuration(rest))" } catch let failure as Failure { return failure.message.hasPrefix("hold takes") ? "m" : "\(failure.status)" } catch { return "?" }
                }
                return expect(words, ["m", "m", "2", "2", "2", "800"])
            }),
            ("alt_with_cmd_is_passed_on", { expect(posted("plus", ["cmd", "alt"], spanish), "30 CA") }),
            ("a_layout_without_key_data_refuses_a_character", {
                let got = refused("slash", ["cmd"], noData)
                return expect(got.hasPrefix("1: ") && got.contains("has no key data"), true) ?? expect(posted("pagedown", [], noData), "121 ")
            }),
            ("no_layout_in_use_is_refused_with_a_sentence", { expect(refused("a", [], nothing).hasPrefix("1: no keyboard layout is in use"), true) }),
            ("an_unknown_modifier_lists_the_modifiers", {
                expect(posted("a", ["command"], us), "2: unknown modifier command: the modifiers are cmd, shift, alt and ctrl")
            }),
            ("an_unknown_key_posts_nothing_and_lists_every_key", {
                expect(refused("quote", [], us), "2: unknown key quote: quote is now us-quote, the key where a US keyboard has ' (a dead key on Spanish ISO); the keys are \(keyGrammar)")
            }),
            ("a_single_character_is_told_its_name", {
                expect(refused("/", ["cmd"], us), "2: unknown key /: its name is slash; the keys are \(keyGrammar)")
            }),
            ("the_order_lists_name_every_key_once", {
                let listed = letters + digits + symbolNames + positionNames
                return expect(Set(listed).count, listed.count) ?? expect(Set(listed), Set(namedKeys.keys).union(characterNames.keys))
            }),
            ("the_usage_line_holds_every_key", { expect(usage.contains(keyGrammar) && usage.contains("hold: KEY [MS]"), true) }),
            ("a_named_key_carries_the_modifiers_asked", {
                let asked = [("tab", ["shift"]), ("left", ["cmd"]), ("return", ["cmd"]), ("escape", ["ctrl"]), ("pagedown", ["cmd", "shift", "alt", "ctrl"])]
                return expect(asked.map { posted($0.0, $0.1, us) }, ["48 S", "123 C", "36 C", "53 K", "121 SCAK"])
                    ?? expect(posted("tab", ["shift"], nothing), "48 S")
            }),
            ("the_keys_a_person_reads_name_every_key_in_order", {
                expect(keyGrammar, "a…z 0…9 comma slash question minus equal plus leftbracket rightbracket return escape tab space delete left right up down pageup pagedown home end us-quote")
            }),
            ("shift_adds_nothing_to_a_ctrl_shortcut_and_says_ctrl", {
                expect([refused("slash", ["ctrl", "shift"], spanish), refused("question", ["ctrl", "shift"], us)], [
                    "2: shift adds nothing to slash on test.Spanish-ISO: the window reads it as ctrl-/ with or without it: post slash ctrl, nothing was posted",
                    "2: shift adds nothing to question on test.US: the window reads it as ctrl-? with or without it: post question ctrl, nothing was posted",
                ])
            }),
            ("a_character_needing_alt_is_refused_with_ctrl_and_says_ctrl", {
                expect(refused("leftbracket", ["ctrl"], spanish),
                    "1: leftbracket is typed only with alt on test.Spanish-ISO, and with cmd or ctrl held the window reads that key without alt, as ctrl-`: this shortcut cannot reach the window on this layout, nothing was posted")
            }),
            ("with_ctrl_the_plain_table_decides", { expect(posted("k", ["ctrl"], dvorakCommand), "9 K") }),
            ("a_typed_capital_is_the_key_that_types_it", { expect(posted("i", ["shift"], turkish), "34 S") }),
            ("a_letter_and_a_mark_under_cmd_read_as_their_capital_with_shift", {
                guard let types = combining.types else { return "no data" }
                return expect(windowKey(types, code: 44, command: true, shift: true), Reading(key: "A\u{301}", shift: false, dead: false))
                    ?? expect(refused("slash", ["cmd"], combining), "1: no key of test.Combining makes the window read slash with these modifiers: nothing was posted")
            }),
            ("a_command_table_whose_shift_text_differs_only_in_scalars_is_read_as_the_window_does", {
                guard let types = canonical.types else { return "no data" }
                return expect(Array(windowKey(types, code: 44, command: true, shift: true).key.unicodeScalars.map(\.value)), [0xE9])
            }),
            ("a_typed_capital_is_never_its_canonical_twin", { expect(posted("k", ["shift"], kelvin), "40 S") }),
            ("the_layout_in_use_gives_a_key_or_says_why", {
                let got = posted("slash", ["cmd"], layoutInUse())
                return expect(got.hasPrefix("1: ") || !got.contains(":"), true)
            }),
        ]
    }

    static let silent = Silent()

    final class Silent: NSObject, NSPasteboardItemDataProvider {
        func pasteboard(_ pasteboard: NSPasteboard?, item: NSPasteboardItem, provideDataForType type: NSPasteboard.PasteboardType) {}
    }

    static let kind = NSPasteboard.PasteboardType("test.kind")
    static let other = NSPasteboard.PasteboardType("test.other")

    static func onABoard<T>(_ body: (NSPasteboard) -> T) -> T {
        let board = NSPasteboard.withUniqueName()
        defer { board.releaseGlobally() }
        return body(board)
    }

    static func theUsersClipboard(on board: NSPasteboard, empty: Bool = false) {
        board.clearContents()
        guard !empty else { return }
        let first = NSPasteboardItem()
        first.setString("the user's own words", forType: .string)
        first.setData(Data([1, 2, 3, 4]), forType: kind)
        let second = NSPasteboardItem()
        second.setData(Data([9, 8]), forType: other)
        board.writeObjects([first, second])
    }

    static func windowWrites(_ text: String, on board: NSPasteboard) {
        board.clearContents()
        board.setString(text, forType: .string)
    }

    static func waited(
        _ text: String, ms: Int = 2000, empty: Bool = false, act: (NSPasteboard) -> Void = { _ in },
        late: (NSPasteboard) -> Void = { _ in }
    ) -> (arrival: Arrival?, kept: [[(NSPasteboard.PasteboardType, Data)]], now: [[(NSPasteboard.PasteboardType, Data)]], count: Int, string: String?) {
        onABoard { board in
            theUsersClipboard(on: board, empty: empty)
            let kept = saved(board)
            let count = board.changeCount
            let arrival = try? awaitCopy(of: text, on: board, within: ms, act: { act(board) }, beforeGivingBack: { late(board) })
            return (arrival, kept, saved(board), board.changeCount - count, board.string(forType: .string))
        }
    }

    static let thing = "cc324 made-up copy"

    static func clipboardTests() -> [(String, () -> String?)] {
        [
            ("a_copy_of_the_text_given_is_given_back", {
                let got = waited(thing, act: { windowWrites(thing, on: $0) })
                return expect(got.arrival, .arrived(items: 2)) ?? expect(holds(got.now, got.kept), true)
            }),
            ("an_empty_clipboard_is_given_back_empty", {
                let got = waited(thing, empty: true, act: { windowWrites(thing, on: $0) })
                return expect(got.arrival, .arrived(items: 0)) ?? expect(got.now.count, 0)
            }),
            ("nothing_arriving_leaves_the_clipboard_untouched", {
                let got = waited(thing, ms: 30)
                return expect(got.arrival, .nothing) ?? expect(got.count, 0) ?? expect(holds(got.now, got.kept), true)
            }),
            ("another_text_is_left_as_it_is", {
                let got = waited(thing, act: { windowWrites("somebody else's words", on: $0) })
                return expect(got.arrival, .other) ?? expect(got.string, "somebody else's words")
            }),
            ("a_second_write_is_left_as_it_is", {
                let got = waited(thing, act: { windowWrites(thing, on: $0); windowWrites(thing, on: $0) })
                return expect(got.arrival, .other) ?? expect(got.string, thing) ?? expect(got.now.count, 1)
            }),
            ("a_write_while_giving_back_is_not_written_over", {
                let got = waited(thing, act: { windowWrites(thing, on: $0) }, late: { windowWrites("typed in that moment", on: $0) })
                return expect(got.arrival, .other) ?? expect(got.string, "typed in that moment")
            }),
            ("a_canonically_equal_text_is_another_text", {
                let got = waited("\u{e9}", act: { windowWrites("e\u{301}", on: $0) })
                return expect(got.arrival, .other) ?? expect(got.string.map { Array($0.unicodeScalars.map(\.value)) }, [0x65, 0x301])
            }),
            ("a_clipboard_that_cannot_be_read_back_posts_nothing", {
                onABoard { board in
                    board.clearContents()
                    let item = NSPasteboardItem()
                    item.setDataProvider(silent, forTypes: [kind])
                    board.writeObjects([item])
                    let count = board.changeCount
                    var posted = false
                    do {
                        _ = try awaitCopy(of: thing, on: board, within: 30, act: { posted = true })
                        return "no refusal"
                    } catch let failure as Failure {
                        return expect([posted, board.changeCount == count], [false, true])
                            ?? expect(failure.status, 1) ?? expect(failure.message, unreadable().message)
                    } catch { return "\(error)" }
                }
            }),
            ("a_copy_while_the_clipboard_is_read_is_never_written_over", {
                onABoard { board in
                    theUsersClipboard(on: board)
                    var posted = false
                    do {
                        _ = try awaitCopy(
                            of: thing, on: board, within: 30, act: { posted = true; windowWrites(thing, on: board) },
                            whileSaving: { windowWrites("copied while it was read", on: board) })
                        return "no refusal"
                    } catch let failure as Failure {
                        return expect(posted, false) ?? expect(board.string(forType: .string), "copied while it was read")
                            ?? expect(failure.status, 1)
                    } catch { return "\(error)" }
                }
            }),
            ("a_copy_that_arrived_is_given_back_through_the_check", {
                onABoard { board in
                    theUsersClipboard(on: board)
                    do {
                        _ = try awaitCopy(
                            of: thing, on: board, within: 2000, act: { windowWrites(thing, on: board) },
                            giving: { _, board in windowWrites("not what was there", on: board) })
                        return "no refusal"
                    } catch let failure as Failure {
                        return expect(failure.status, 1) ?? expect(failure.message.hasPrefix("the clipboard differs after it was given back"), true)
                    } catch { return "\(error)" }
                }
            }),
            ("a_giving_back_is_told_apart_by_data_and_by_count", {
                let kept = [[(kind, Data([1, 2])), (other, Data([3]))], [(NSPasteboard.PasteboardType.string, Data([4]))]]
                let sameData = kept
                let otherData = [[(kind, Data([1, 9])), (other, Data([3]))], [(NSPasteboard.PasteboardType.string, Data([4]))]]
                let fewer = [kept[0]]
                let more = kept + [[(kind, Data([5]))]]
                return expect([holds(sameData, kept), holds(otherData, kept), holds(fewer, kept), holds(more, kept)], [true, false, false, false])
            }),
            ("a_clipboard_that_differs_after_it_was_given_back_is_told", {
                onABoard { board in
                    theUsersClipboard(on: board)
                    let kept = saved(board)
                    do {
                        try giveBack(kept, to: board, using: { _, board in windowWrites("not what was there", on: board) })
                        return "no refusal"
                    } catch let failure as Failure {
                        return expect(failure.status, 1)
                            ?? expect(failure.message, "the clipboard differs after it was given back: tell the person who runs this check at once")
                    } catch { return "\(error)" }
                }
            }),
            ("the_copy_answers_say_their_words", {
                let answers = [Arrival.arrived(items: 2), .arrived(items: 1), .nothing, .other].map { copyAnswer($0, ms: 500) }
                return expect(answers.map { $0.status }, [0, 0, 4, 5])
                    ?? expect(answers[0].words, "the copy arrived: 2 items given back")
                    ?? expect(answers[1].words, "the copy arrived: 1 item given back")
                    ?? expect(answers[2].words, "nothing arrived on the clipboard in 500 ms: it was not touched")
                    ?? expect(answers[3].words.hasPrefix("something else arrived on the clipboard (another text, a second copy, or the person's own; the text given must be the copy's exact bytes, with no newline added): it is left as it is and none of it is printed;"), true)
            }),
            ("copy_reads_its_wait_and_its_key", {
                func read(_ args: [String], _ layout: Layout = us) -> String {
                    do {
                        let plan = try copyPlan(args, layout: { layout })
                        let key = plan.posting.map { "\($0.code) \($0.flags.contains(.maskCommand) ? "C" : "")" } ?? "no key"
                        return "\(plan.text) \(plan.ms) \(key)"
                    } catch let failure as Failure { return "\(failure.status)" } catch { return "\(error)" }
                }
                return expect(
                    [read(["t", "5000"]), read(["t", "0"]), read(["t", "60001"]), read(["t", "abc"]), read(["", "5000"]), read(["t"]),
                     read(["t", "5000", "c", "cmd"]), read(["t", "5000", "c", "cmd"], noData), read(["t", "5000", "nope"])],
                    ["t 5000 no key", "2", "2", "2", "2", "2", "t 5000 8 C", "1", "2"])
            }),
            ("the_usage_line_holds_the_copy_grammar", {
                expect(usage.contains("copy: TEXT MS [KEY [cmd] [shift] [alt] [ctrl]]") && usage.contains("|paste|copy|press|"), true)
            }),
        ]
    }

    static func mice(_ steps: [PressStep]) -> [MouseEvent] {
        steps.compactMap { if case .post(let event) = $0 { event } else { nil } }
    }

    static func spans(_ steps: [PressStep]) -> [Int] {
        var spans: [Int] = []
        var since: Int?
        for step in steps {
            switch step {
            case .post(let event) where event.type == .leftMouseDown || event.type == .rightMouseDown:
                if let since { spans.append(since) }
                since = 0
            case .wait(let ms): since = since.map { $0 + ms }
            default: break
            }
        }
        return spans
    }

    static func refusal(_ body: () throws -> Void) -> String {
        do { try body() } catch let failure as Failure { return "\(failure.status): \(failure.message)" } catch { return "\(error)" }
        return "no refusal"
    }

    static func steps(_ button: CGMouseButton, _ count: Int, _ interval: Int = 500) -> [PressStep] {
        (try? pressSteps(button, count: count, interval: interval)) ?? []
    }

    static func walked(_ count: Int, failingAt ask: Int?) -> (posted: [MouseEvent], warps: [CGPoint], failure: String) {
        var posted: [MouseEvent] = []
        var warps: [CGPoint] = []
        var asked = 0
        let effects = PressEffects(
            ask: {
                asked += 1
                if asked == ask { throw Failure(message: coverRefusal("200", "200", [cover]), status: 3) }
            },
            post: { posted.append($0) }, wait: { _ in }, read: { CGPoint(x: 7, y: 9) }, warp: { warps.append($0) })
        let failure = refusal { try walkPress(steps(.left, count), at: "200,200", with: effects) }
        return (posted, warps, failure)
    }

    static func eventLeft(_ type: CGEventType, _ state: Int64) -> MouseEvent { MouseEvent(type: type, button: .left, state: state) }

    static func single(_ button: CGMouseButton, _ down: CGEventType, _ up: CGEventType) -> [PressStep] {
        [.post(MouseEvent(type: .mouseMoved, button: .left, state: 0)), .wait(150), .ask,
         .post(MouseEvent(type: down, button: button, state: 1)), .wait(50),
         .post(MouseEvent(type: up, button: button, state: 1)), .wait(100)]
    }

    static func shape(_ steps: [PressStep]) -> [String] {
        mice(steps).map { "\($0.type.rawValue):\($0.state)" }
    }

    static func builtStates(_ steps: [PressStep]) -> [String] {
        mice(steps).map { event in
            guard let built = mouseEvent(event, at: CGPoint(x: 1, y: 1)) else { return "not built" }
            return "\(built.type.rawValue):\(built.getIntegerValueField(.mouseEventClickState))"
        }
    }

    static func pressTests() -> [(String, () -> String?)] {
        let moved = CGEventType.mouseMoved.rawValue
        let first = "one press was posted at 200,200, the second was not: another window covers 200,200: \(cover): look at what it did before the next press"
        return [
            ("a_single_press_needs_no_interval", { expect(steps(.left, 1, 0), single(.left, .leftMouseDown, .leftMouseUp)) }),
            ("a_count_outside_one_to_three_is_refused", {
                expect([0, 4, -1].map { count in refusal { _ = try pressSteps(.left, count: count, interval: 500) } },
                    [0, 4, -1].map { "2: a press posts one, two or three presses, not \($0): nothing was posted" })
            }),
            ("a_press_stopped_half_way_posts_no_second_down", {
                expect(walked(2, failingAt: 2).posted, [MouseEvent(type: .mouseMoved, button: .left, state: 0), eventLeft(.leftMouseDown, 1), eventLeft(.leftMouseUp, 1)])
            }),
            ("the_half_way_words_say_how_many_presses_were_posted", {
                let covered = Failure(message: coverRefusal("200", "200", [cover]), status: 3)
                let front = Failure(message: "process 4242 is not in front: nothing was posted", status: 3)
                let none = Failure(message: "no window of process 4242 is at 200,200: nothing was posted", status: 3)
                return expect(
                    [halfWayRefusal(1, of: 2, at: "200,200", covered), halfWayRefusal(1, of: 3, at: "200,200", front), halfWayRefusal(2, of: 3, at: "200,200", none)]
                        .map { "\($0.status): \($0.message)" },
                    ["3: " + first,
                     "3: one press was posted at 200,200, the second and the third were not: process 4242 is not in front: look at what it did before the next press",
                     "3: two presses were posted at 200,200, the third was not: no window of process 4242 is at 200,200: look at what they did before the next press"])
            }),
            ("the_walk_says_the_half_way_words_of_a_failed_second_ask", { expect(walked(2, failingAt: 2).failure, "3: " + first) }),
            ("a_refusal_before_the_first_down_is_a_single_press_refusal", {
                let result = walked(2, failingAt: 1)
                return expect([result.posted.map { $0.type.rawValue }.map { String($0) }.joined(separator: ","), result.failure],
                    [String(moved), "3: another window covers 200,200: \(cover): nothing was posted"])
            }),
            ("the_pointer_is_put_back_once_whether_the_act_ends_or_stops_half_way", {
                let at = CGPoint(x: 7, y: 9)
                return expect([walked(2, failingAt: nil).warps, walked(2, failingAt: 2).warps], [[at], [at]])
            }),
            ("press_reads_its_button_and_count_words", {
                let read = { (words: [String]) -> String in
                    guard let got = try? pressOptions(words) else { return "refused" }
                    return "\(got.button == .right ? "right" : "left") \(got.count)"
                }
                return expect([[], ["right"], ["double"], ["triple"]].map(read), ["left 1", "right 1", "left 2", "left 3"])
            }),
            ("press_refuses_a_word_it_does_not_know_and_says_the_words_it_takes", {
                let said = "2: press takes at most one word after the point (right, double or triple), not "
                let hint = ": a count is a word, double for 2 and triple for 3 (there is no other)"
                return expect(
                    [["middle"], ["2"], ["4"], ["double", "triple"], ["right", "right"]].map { words in refusal { _ = try pressOptions(words) } },
                    [said + "'middle': nothing was posted", said + "'2'" + hint + ": nothing was posted",
                     said + "'4'" + hint + ": nothing was posted", said + "'double triple': nothing was posted",
                     said + "'right right': nothing was posted"])
            }),
            ("press_refuses_a_count_on_a_right_press", {
                let line = "2: a right press takes no count: double and triple post left presses only, and press PID X Y right posts one right press: nothing was posted"
                return expect([["right", "double"], ["triple", "right"]].map { words in refusal { _ = try pressOptions(words) } }, [line, line])
            }),
            ("the_usage_line_holds_the_press_grammar", { expect(usage.contains("press: x y [right|double|triple]; drag:"), true) }),
        ]
    }

    static func pressStepTests() -> [(String, () -> String?)] {
        let down = CGEventType.leftMouseDown.rawValue, up = CGEventType.leftMouseUp.rawValue
        let moved = CGEventType.mouseMoved.rawValue
        return [
            ("a_single_press_posts_a_move_then_one_down_and_up_with_their_waits", {
                expect(steps(.left, 1), single(.left, .leftMouseDown, .leftMouseUp))
            }),
            ("a_single_right_press_posts_right_events_after_a_left_move", {
                expect(steps(.right, 1), single(.right, .rightMouseDown, .rightMouseUp))
            }),
            ("a_double_press_numbers_its_pairs_one_then_two", {
                expect(shape(steps(.left, 2)), ["\(moved):0", "\(down):1", "\(up):1", "\(down):2", "\(up):2"])
            }),
            ("a_triple_press_numbers_its_pairs_one_two_then_three", {
                expect(shape(steps(.left, 3)), ["\(moved):0", "\(down):1", "\(up):1", "\(down):2", "\(up):2", "\(down):3", "\(up):3"])
            }),
            ("every_built_event_carries_the_click_state_of_its_step", {
                expect(builtStates(steps(.left, 3)) + builtStates(steps(.right, 1)),
                    ["\(moved):0", "\(down):1", "\(up):1", "\(down):2", "\(up):2", "\(down):3", "\(up):3"]
                        + ["\(moved):0", "\(CGEventType.rightMouseDown.rawValue):1", "\(CGEventType.rightMouseUp.rawValue):1"])
            }),
            ("every_down_is_followed_by_its_own_up", {
                let pairs = [1, 2, 3].map { mice(steps(.left, $0)).dropFirst().map { "\($0.type.rawValue):\($0.button.rawValue):\($0.state)" } }
                return expect(pairs.map { $0.count }, [2, 4, 6]) ?? expect(
                    pairs[2], ["\(down):0:1", "\(up):0:1", "\(down):0:2", "\(up):0:2", "\(down):0:3", "\(up):0:3"])
            }),
            ("an_ask_comes_right_before_every_down", {
                expect([1, 2, 3].map { count in
                    let list = steps(.left, count)
                    return list.indices.filter { index in
                        if case .post(let event) = list[index], event.type == .leftMouseDown { return list[index - 1] == .ask }
                        return false
                    }.count * 10 + list.filter { $0 == .ask }.count
                }, [11, 22, 33])
            }),
            ("no_ask_comes_between_a_down_and_its_up", {
                expect([1, 2, 3].map { count in
                    var open = false
                    var asked = false
                    for step in steps(.left, count) {
                        if case .post(let event) = step { open = event.type == .leftMouseDown }
                        if step == .ask, open { asked = true }
                    }
                    return asked
                }, [false, false, false])
            }),
            ("the_downs_of_a_double_and_a_triple_press_come_within_the_interval_given", {
                expect([2, 3].flatMap { count in [150, 500].map { interval in spans(steps(.left, count, interval)).map { $0 + 50 <= interval } } },
                    [[true], [true], [true, true], [true, true]])
            }),
            ("an_interval_too_short_for_a_double_or_a_triple_press_is_refused_with_its_words", {
                let words = { (word: String, interval: Int) in
                    "1: a \(word) press needs a double-click interval of 150 ms or more (its presses are posted 100 ms apart, each checked first), and this Mac's is \(interval) ms: a \(word) press cannot be posted on this Mac, and presses posted one by one in its place would act as single ones; the interval is the person's own setting, never a check's to change: nothing was posted"
                }
                return expect(
                    [2, 3, 2].map { count in refusal { _ = try pressSteps(.left, count: count, interval: 149) } } + [String(steps(.left, 2, 150).count)],
                    [words("double", 149), words("triple", 149), words("double", 149), "12"])
            }),
        ]
    }

    static func run() throws {
        let cases = coverTests() + dictionaryTests() + beforeTests() + wordTests() + keyTests() + clipboardTests() + pressStepTests() + pressTests()
            + uuidTests() + coverOfDisplayTests() + everyMacTests() + negativeDisplayTests() + twoCheckTests() + answerTests() + placeTests()
        var failed = 0
        for (name, body) in cases {
            if let why = body() {
                failed += 1
                print("not ok - \(name): \(why)")
            } else {
                print("ok - \(name)")
            }
        }
        print("\(cases.count) tests, \(failed) failed")
        guard failed == 0 else { throw Failure(message: "self-test failed", status: 1) }
    }
}

extension SelfTest {
    static let firstUUID = "00000000-0000-4000-8000-000000000001"
    static let secondUUID = "00000000-0000-4000-8000-000000000002"
    static let mainFrame = CGRect(x: 0, y: 0, width: 1000, height: 800)
    static let besideFrame = CGRect(x: 1000, y: 0, width: 1000, height: 800)
    static let aboveLeftFrame = CGRect(x: -1000, y: -200, width: 1000, height: 800)
    static let mainVisible = CGRect(x: 0, y: 25, width: 1000, height: 700)
    static let besideVisible = CGRect(x: 1000, y: 25, width: 1000, height: 775)
    static let aboveLeftVisible = CGRect(x: -1000, y: -175, width: 1000, height: 775)
    static let pair = Screen(displays: [mainFrame, besideFrame], visible: [mainVisible, besideVisible])
    static let aside = Screen(displays: [mainFrame, aboveLeftFrame], visible: [mainVisible, aboveLeftVisible])
    static let single = Screen(displays: [mainFrame], visible: [mainVisible])
    static let firstDisplay = Display(uuid: firstUUID, frame: mainFrame, main: true)
    static let secondDisplay = Display(uuid: secondUUID, frame: besideFrame, main: false)
    static let leftDisplay = Display(uuid: secondUUID, frame: aboveLeftFrame, main: false)
    static let firstHead = "\(firstUUID) 0 0 1000 800 main"
    static let secondHead = "\(secondUUID) 1000 0 1000 800 other"
    static let leftHead = "\(secondUUID) -1000 -200 1000 800 other"
    static let screenshot = "Screenshot (pid 69680, layer 24)"

    static func fill(_ frame: CGRect) -> Shown { made(11, 69680, "Screenshot", 24, frame) }

    static func gpui(_ id: Int, _ pid: pid_t) -> Shown { made(id, pid, "cornercase-gpui", 0, CGRect(x: 1060, y: 85, width: 600, height: 400)) }

    static func answer(_ displays: [Display], _ list: [Shown], on screen: Screen, place: CGRect? = nil) -> String {
        let found = displaysAnswer(displays, place: place, in: list, on: screen, naming: { named($0, nil) })
        return "\(found.status) \(found.lines.joined(separator: " | ")) \(found.words ?? "-")"
    }

    static func answered(_ displays: [Display], _ list: [Shown], on screen: Screen, place: CGRect? = nil) -> (lines: [String], status: Int32) {
        let found = displaysAnswer(displays, place: place, in: list, on: screen, naming: { named($0, nil) })
        return (found.lines, found.status)
    }

    static func parts(_ list: [Shown], at frame: CGRect, on screen: Screen) -> [String] {
        coversBeforeFront(of: testDebug, frame: frame, in: list + [made(100, testDebug, "cornercase-gpui", 0, frame)], on: screen)
            .map { partText($0.part) }
    }

    static func uuidTests() -> [(String, () -> String?)] {
        let text = "00000000-0000-4000-8000-00000000abcd"
        let bytes = CFUUIDBytes(
            byte0: 0, byte1: 0, byte2: 0, byte3: 0, byte4: 0, byte5: 0, byte6: 0x40, byte7: 0, byte8: 0x80, byte9: 0, byte10: 0,
            byte11: 0, byte12: 0, byte13: 0, byte14: 0xab, byte15: 0xcd)
        return [
            ("a_displays_uuid_is_written_in_lower_case_with_its_hyphens", { expect(uuidText(bytes), text) }),
            ("the_uuids_text_is_core_foundations_own_in_lower_case", {
                guard let made = CFUUIDCreateFromString(nil, text as CFString), let own = CFUUIDCreateString(nil, made) as String? else {
                    return "no uuid made"
                }
                return expect([uuidText(CFUUIDGetUUIDBytes(made)) == own.lowercased(), uuidText(CFUUIDGetUUIDBytes(made)) != own], [true, true])
            }),
        ]
    }

    static func coverOfDisplayTests() -> [(String, () -> String?)] {
        let one = fill(mainFrame)
        let panel = made(21, 30, "Panel", 1000, CGRect(x: 1700, y: 600, width: 200, height: 100))
        let notes = made(22, 31, "Notes", 3, CGRect(x: 1100, y: 100, width: 100, height: 100))
        let both = [firstDisplay, secondDisplay]
        return [
            ("a_cover_filling_one_display_covers_a_debug_window_there_and_not_on_the_other", {
                expect([parts([one], at: CGRect(x: 60, y: 85, width: 600, height: 400), on: pair),
                    parts([one], at: CGRect(x: 1060, y: 85, width: 600, height: 400), on: pair)], [["0,0 600×400"], []])
            }),
            ("displays_calls_the_covered_display_covered_and_the_other_free", {
                expect(answer(both, [one], on: pair),
                    "0 \(firstHead) covered: \(screenshot) covers 0,25 1000×700 of the display | \(secondHead) free -")
            }),
            ("a_place_is_asked_in_each_displays_own_points", {
                expect(answered(both, [one], on: pair, place: CGRect(x: 60, y: 60, width: 600, height: 400)).lines,
                    ["\(firstHead) covered: \(screenshot) covers 0,0 600×400 of the place", "\(secondHead) free"])
            }),
            ("covers_over_part_of_a_display_are_named_front_to_back_with_their_parts", {
                expect(answered(both, [panel, notes], on: pair).lines[1],
                    "\(secondHead) covered: Panel (pid 30, layer 1000) covers 700,600 200×100 of the display; Notes (pid 31, layer 3) covers 100,100 100×100 of the display")
            }),
            ("a_place_clear_of_a_cover_is_free_and_one_under_it_is_not", {
                expect([CGRect(x: 60, y: 60, width: 500, height: 400), CGRect(x: 600, y: 500, width: 400, height: 300)].map {
                    answered(both, [panel], on: pair, place: $0).lines[1]
                }, ["\(secondHead) free", "\(secondHead) covered: Panel (pid 30, layer 1000) covers 100,100 200×100 of the place"])
            }),
            ("a_place_not_inside_a_display_is_outside_there", {
                let place = CGRect(x: 60, y: 60, width: 1280, height: 800)
                let found = answered(both, [], on: pair, place: place)
                let words = "no display is free at 60,60 1280×800: on each a window of a higher layer covers that place or the place is not inside it (the lines above say which)"
                return expect("\(found.status) \(found.lines) \(answer(both, [], on: pair, place: place).hasSuffix(words))",
                    "3 [\"\(firstHead) outside: 60,60 1280×800 is not inside it\", \"\(secondHead) outside: 60,60 1280×800 is not inside it\"] true")
            }),
        ]
    }

    static func everyMacTests() -> [(String, () -> String?)] {
        let dock = made(31, 22, "Dock", 20, besideFrame)
        let hidden = made(32, 10, "X", 24, besideFrame, alpha: 0)
        let control = made(33, 20, "Control Center", 25, CGRect(x: 1900, y: 0, width: 40, height: 25))
        let bar = made(34, 21, "Window Server", 24, CGRect(x: 0, y: 0, width: 1000, height: 25))
        let dockBar = made(35, 22, "Dock", 20, CGRect(x: 0, y: 725, width: 1000, height: 75))
        let ordinary = made(36, 23, "Google Chrome", 0, CGRect(x: 100, y: 100, width: 500, height: 400))
        return [
            ("the_docks_backdrop_alpha_0_and_the_menu_bar_are_passed_over_on_a_second_display", {
                let list = [dock, hidden, control]
                return expect([answered([firstDisplay, secondDisplay], list, on: pair).lines[1],
                    String(parts(list, at: CGRect(x: 1060, y: 85, width: 600, height: 400), on: pair).count)], ["\(secondHead) free", "0"])
            }),
            ("a_display_with_only_what_every_mac_has_is_free", {
                expect(answer([firstDisplay], [control, bar, made(37, 22, "Dock", 20, mainFrame), dockBar, ordinary], on: single), "0 \(firstHead) free -")
            }),
        ]
    }

    static func negativeDisplayTests() -> [(String, () -> String?)] {
        return [
            ("a_display_left_of_and_above_the_main_one_is_read_in_its_own_points", {
                let list = [fill(aboveLeftFrame)]
                return expect([answered([firstDisplay, leftDisplay], list, on: aside).lines,
                    parts(list, at: CGRect(x: -940, y: -140, width: 600, height: 400), on: aside)],
                    [["\(firstHead) free", "\(leftHead) covered: \(screenshot) covers 0,25 1000×775 of the display"], ["0,0 600×400"]])
            }),
            ("a_visible_area_above_and_left_of_the_main_display_reads_at_negative_points", {
                expect(topLeft(CGRect(x: -1000, y: 200, width: 1000, height: 775), mainTop: 800), aboveLeftVisible)
            }),
        ]
    }

    static func twoCheckTests() -> [(String, () -> String?)] {
        let a = gpui(41, 4242)
        let b = gpui(42, 4343)
        let at = CGPoint(x: 1200, y: 200)
        let both = [firstDisplay, secondDisplay]
        return [
            ("before_the_front_another_debug_window_is_no_cover_in_either_order", {
                expect([[b, a], [a, b]].map { coversBeforeFront(of: 4242, frame: a.frame, in: $0, on: pair).count }, [0, 0])
            }),
            ("at_a_point_the_debug_window_above_is_the_one_in_the_way_in_either_order", {
                expect([[b, a], [a, b]].map { ids(inTheWay(at: at, of: 4242, in: $0, on: pair)) }, ["42", ""])
            }),
            ("a_press_refusal_names_the_other_debug_window_in_the_way", {
                expect(coverRefusal("140", "115", [named(b, "cornercase-gpui")]),
                    "another window covers 140,115: cornercase-gpui (pid 4343, cornercase-gpui, layer 0): nothing was posted")
            }),
            ("displays_calls_a_display_with_two_debug_windows_free_in_either_order", {
                expect([[b, a], [a, b]].map { answered(both, $0, on: pair).lines[1] }, ["\(secondHead) free", "\(secondHead) free"])
            }),
        ]
    }

    static func answerTests() -> [(String, () -> String?)] {
        let none = "no display is free: on each a window of a higher layer covers all or part of it (the lines above say which and where); a place clear of a cover can be asked with displays X Y WIDTH HEIGHT"
        return [
            ("with_one_covered_display_none_is_free", {
                expect(answer([firstDisplay], [fill(mainFrame)], on: single),
                    "3 \(firstHead) covered: \(screenshot) covers 0,25 1000×700 of the display \(none)")
            }),
            ("no_display_listed_is_an_error_with_its_words", {
                expect(answer([], [], on: single), "1  the system lists no active display (CGGetActiveDisplayList), so there is none to name")
            }),
            ("a_display_whose_uuid_cannot_be_read_is_written_with_a_dash", {
                expect(answered([Display(uuid: nil, frame: mainFrame, main: true)], [], on: single).lines, ["- 0 0 1000 800 main free"])
            }),
            ("a_display_that_is_not_the_main_one_is_written_as_other", {
                expect(answered([Display(uuid: firstUUID, frame: mainFrame, main: false)], [], on: single).lines, ["\(firstUUID) 0 0 1000 800 other free"])
            }),
        ]
    }

    static func placeTests() -> [(String, () -> String?)] {
        let say = "displays takes no pid: nothing, or a place as x y width height in a display's own points from its top-left (as layout.json's bounds), not "
        let words = { (list: [String]) in refusal { _ = try askedPlace(list) } }
        return [
            ("displays_reads_nothing_or_a_place", {
                let none = try? askedPlace([])
                let asked = try? askedPlace(["60", "60", "1280", "800"])
                return expect([none == .some(nil), asked == .some(CGRect(x: 60, y: 60, width: 1280, height: 800))], [true, true])
            }),
            ("displays_refuses_what_it_does_not_take_and_says_why", {
                expect(["12345"].map { words([$0]) } + [words(["1", "2", "3"]), words(["a", "0", "10", "10"]), words(["0", "0", "0", "800"]), words(["-1000", "60", "100", "100"])],
                    ["2: \(say)'12345'", "2: \(say)'1 2 3'", "2: x must be a whole number, not 'a'", "2: a place's width and height are more than 0, not 0×800",
                        "2: a place is in a display's own points from its top-left (as layout.json's bounds), so its x and y are 0 or more, not -1000,60: the numbers on displays' own lines are global, a window's place is not"])
            }),
            ("the_usage_line_holds_the_displays_grammar", { expect(usage.contains("scroll PID … | displays [x y width height] | self-test "), true) }),
            ("this_macs_displays_are_answered_whatever_they_are", {
                let found = displaysAnswer(readDisplays(), place: nil, in: onScreen(), on: screen(), naming: { named($0, nil) })
                let shapes = found.lines.map { $0.split(separator: " ").first.map { $0 == "-" || ($0.count == 36 && $0 == $0.lowercased()) } ?? false }
                return expect([[0, 1, 3].contains(found.status), shapes.allSatisfy { $0 }, (found.status == 1) == found.lines.isEmpty], [true, true, true])
            }),
        ]
    }
}

func run(_ args: [String]) throws {
    if args == ["self-test"] { return try SelfTest.run() }
    if args.first == "displays" { return try listDisplays(Array(args.dropFirst())) }
    guard args.count >= 2, let pid = pid_t(args[1]) else { throw Failure(message: usage, status: 2) }
    if ["front", "size", "key", "hold", "text", "paste", "copy", "press", "drag", "scroll"].contains(args[0]) { try checkedPid(pid) }
    switch (args[0], args.count) {
    case ("window", 2): try describe(pid)
    case ("covers", 2): try covers(pid)
    case ("front", 2): try front(pid)
    case ("size", 4): try size(pid, args[2], args[3])
    case ("key", 3...): try key(pid, args[2], Array(args.dropFirst(3)))
    case ("hold", 3...): try hold(pid, args[2], Array(args.dropFirst(3)))
    case ("text", 3): try text(pid, args[2])
    case ("paste", 3): try paste(pid, args[2])
    case ("copy", 4...): try copy(pid, Array(args.dropFirst(2)))
    case ("press", 4...):
        let (button, count) = try pressOptions(Array(args.dropFirst(4)))
        try press(pid, args[2], args[3], button: button, count: count)
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
